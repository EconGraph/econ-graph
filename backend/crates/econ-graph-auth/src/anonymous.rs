//! Shared anonymous catalog grants, obtained with server-side client credentials.
//! No service-account identity or non-catalog capability is attached to visitors.

use crate::{OidcVerifier, Role, VerifyError};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Server-only client configuration. Deliberately does not implement Debug.
pub struct AnonymousConfig {
    /// Internal token endpoint, which may differ from the public issuer URL.
    pub token_url: String,
    /// Dedicated client holding the anonymous catalog grants.
    pub client_id: String,
    /// Confidential client credential, never sent to visitors.
    pub client_secret: String,
    /// `ANONYMOUS_GRANTS_STALE_MAX_SECS`: how long past their expiry the last fetched grants
    /// may still be served while renewal is failing. Zero (the default) fails closed.
    pub stale_max: Duration,
}

/// Upper bound for `ANONYMOUS_GRANTS_STALE_MAX_SECS`: one day.
pub const MAX_STALE_SECS: u64 = 86_400;

impl AnonymousConfig {
    /// Read configuration. Missing credentials grant no anonymous catalog access.
    /// Partial or malformed configuration is a startup error.
    pub fn from_env() -> Result<Option<Self>, String> {
        Self::from_vars(|key| std::env::var(key).ok())
    }

    /// Injectable environment reader for configuration tests.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Option<Self>, String> {
        let set = |key| var(key).filter(|value| !value.trim().is_empty());
        let url = set("OIDC_ANONYMOUS_TOKEN_URL");
        let secret = set("OIDC_ANONYMOUS_CLIENT_SECRET");
        if url.is_none() && secret.is_none() && set("OIDC_ANONYMOUS_CLIENT_ID").is_none() {
            return Ok(None);
        }
        let token_url = url
            .filter(|s| !s.trim().is_empty())
            .ok_or("OIDC_ANONYMOUS_TOKEN_URL is required")?;
        let parsed = reqwest::Url::parse(&token_url).map_err(|_| "Invalid anonymous token URL")?;
        let loopback = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        let private_http = parsed.scheme() == "http"
            && (loopback || set("OIDC_ANONYMOUS_ALLOW_INSECURE_HTTP").as_deref() == Some("true"));
        if !(parsed.scheme() == "https" || private_http)
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err("Invalid anonymous token URL".into());
        }
        let client_secret = secret
            .filter(|s| !s.trim().is_empty())
            .ok_or("OIDC_ANONYMOUS_CLIENT_SECRET is required")?;
        let client_id =
            var("OIDC_ANONYMOUS_CLIENT_ID").unwrap_or_else(|| "econgraph-anonymous".into());
        if client_id.trim().is_empty() {
            return Err("Anonymous client ID is empty".into());
        }
        let stale_max = match set("ANONYMOUS_GRANTS_STALE_MAX_SECS") {
            None => Duration::ZERO,
            Some(value) => match value.trim().parse::<u64>() {
                Ok(secs) if secs <= MAX_STALE_SECS => Duration::from_secs(secs),
                _ => {
                    return Err(format!(
                        "ANONYMOUS_GRANTS_STALE_MAX_SECS must be a whole number of seconds from 0 to {MAX_STALE_SECS}"
                    ))
                }
            },
        };
        Ok(Some(Self {
            token_url,
            client_id,
            client_secret,
            stale_max,
        }))
    }
}

pub(crate) struct AnonymousPermissions {
    config: AnonymousConfig,
    client: reqwest::Client,
    /// Readers hold this lock only to copy the grant set; renewal never holds it across a
    /// request to Keycloak.
    state: RwLock<Grants>,
    stats: RenewalStats,
}

/// Renewal counters for metrics, updated by every attempt.
#[derive(Default)]
struct RenewalStats {
    successes: AtomicU64,
    outages: AtomicU64,
    refusals: AtomicU64,
    consecutive_failures: AtomicU64,
    task_running: AtomicBool,
}

/// A snapshot of anonymous grant renewal, for metrics and alerts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousRenewalStats {
    /// Renewals that fetched a grant set.
    pub successes: u64,
    /// Renewals that failed because Keycloak could not be asked or answered unusably.
    pub outages: u64,
    /// Renewals Keycloak refused, or whose token failed verification.
    pub refusals: u64,
    /// Failed renewals since the last success.
    pub consecutive_failures: u64,
    /// Catalog roles anonymous callers hold right now.
    pub current_grants: u64,
    /// Whether expired grants are being served (only while renewal fails, within the window).
    pub serving_stale: bool,
    /// Whether the background refresh task is running.
    pub task_running: bool,
}

/// The current grant set. Every successful renewal replaces it wholesale.
#[derive(Default)]
struct Grants {
    roles: BTreeSet<Role>,
    /// When the grant set's token expires (at most [`MAX_LIFETIME`] after it was fetched).
    valid_until: Option<Instant>,
    /// Set by a renewal that failed for an outage, cleared by a successful one. Only while it
    /// is set may expired grants be served, for at most `AnonymousConfig::stale_max`.
    renewal_failing: bool,
}

/// Grants are renewed at least this often, whatever the token's own lifetime.
const MAX_LIFETIME: Duration = Duration::from_secs(300);
/// Delay between renewal attempts while Keycloak is failing.
const RETRY_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

/// Why a renewal produced no grant set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenewalError {
    /// Keycloak could not be reached or answered unusably: an outage, during which expired
    /// grants may be served for `stale_max`.
    Outage,
    /// Keycloak definitively refused (client disabled, secret rotated, token rejected): the
    /// grants are dropped at once, as for a revocation.
    Refused,
}

/// What one renewal attempt did, for the refresh loop's schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renewal {
    /// New grants, valid for this long from the start of the attempt.
    Renewed(Duration),
    /// No new grants.
    Failed,
}

impl OidcVerifier {
    /// Enable anonymous grants without fetching a token. [`OidcVerifier::spawn_anonymous_refresh`]
    /// fetches them; until a fetch succeeds, anonymous callers have no catalog access.
    pub fn with_anonymous(mut self, config: AnonymousConfig) -> Self {
        self.anonymous = Some(AnonymousPermissions {
            config,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("HTTP client with default TLS settings"),
            state: RwLock::new(Grants::default()),
            stats: RenewalStats::default(),
        });
        self
    }

    /// The anonymous caller's catalog roles, from memory: this never waits for Keycloak.
    ///
    /// Unexpired grants are returned as fetched. Expired grants are returned only while
    /// renewal is failing for an outage, and for at most `stale_max` past their expiry;
    /// otherwise anonymous callers get no catalog access.
    pub fn anonymous_catalog_roles(&self) -> BTreeSet<Role> {
        match &self.anonymous {
            Some(provider) => provider.current(Instant::now()),
            None => BTreeSet::new(),
        }
    }

    /// Renewal counters and the current grant count; `None` without anonymous grants.
    pub fn anonymous_renewal_stats(&self) -> Option<AnonymousRenewalStats> {
        let provider = self.anonymous.as_ref()?;
        let now = Instant::now();
        let current_grants = provider.current(now).len() as u64;
        let serving_stale = {
            let state = provider.state.read().unwrap_or_else(|e| e.into_inner());
            state.valid_until.is_some_and(|expiry| expiry <= now) && current_grants > 0
        };
        let stats = &provider.stats;
        Some(AnonymousRenewalStats {
            successes: stats.successes.load(Ordering::Relaxed),
            outages: stats.outages.load(Ordering::Relaxed),
            refusals: stats.refusals.load(Ordering::Relaxed),
            consecutive_failures: stats.consecutive_failures.load(Ordering::Relaxed),
            current_grants,
            serving_stale,
            task_running: stats.task_running.load(Ordering::Relaxed),
        })
    }

    /// Start the one background task that renews anonymous grants: at once, then when three
    /// quarters of each grant set's lifetime has passed, and every 10 seconds while renewal
    /// fails. Requests never wait for it. `None` when anonymous grants are not configured.
    ///
    /// The task never ends on its own. If it does end (a panic, or an abort), it logs an error
    /// and `task_running` in [`OidcVerifier::anonymous_renewal_stats`] turns false; anonymous
    /// grants then lapse at their expiry.
    pub fn spawn_anonymous_refresh(self: &Arc<Self>) -> Option<tokio::task::JoinHandle<()>> {
        let provider = self.anonymous.as_ref()?;
        provider.stats.task_running.store(true, Ordering::Relaxed);
        let verifier = Arc::clone(self);
        // Moved into the task, so it is dropped however the task ends, even before it first runs.
        let running = TaskRunning(Arc::clone(self));
        Some(tokio::spawn(async move {
            let _running = running;
            loop {
                let wait = match verifier.renew_anonymous_grants().await {
                    Renewal::Renewed(valid_for) => (valid_for * 3 / 4).max(Duration::from_secs(1)),
                    Renewal::Failed => RETRY_INTERVAL,
                };
                tokio::time::sleep(wait).await;
            }
        }))
    }

    /// One renewal attempt. A success replaces the grant set at once, so a revocation takes
    /// effect immediately; an outage keeps the old set only within the stale window; a
    /// refusal clears it.
    pub async fn renew_anonymous_grants(&self) -> Renewal {
        let Some(provider) = &self.anonymous else {
            return Renewal::Failed;
        };
        let started = Instant::now();
        let result = provider.fetch(self).await;
        let mut state = provider.state.write().unwrap_or_else(|e| e.into_inner());
        let stats = &provider.stats;
        match &result {
            Ok(_) => {
                stats.successes.fetch_add(1, Ordering::Relaxed);
                stats.consecutive_failures.store(0, Ordering::Relaxed);
            }
            Err(error) => {
                match error {
                    RenewalError::Outage => &stats.outages,
                    RenewalError::Refused => &stats.refusals,
                }
                .fetch_add(1, Ordering::Relaxed);
                stats.consecutive_failures.fetch_add(1, Ordering::Relaxed);
            }
        }
        match result {
            Ok((roles, valid_for)) => {
                // Measured from BEFORE the HTTP requests, never extending the token lifetime.
                *state = Grants {
                    roles,
                    valid_until: started.checked_add(valid_for),
                    renewal_failing: false,
                };
                Renewal::Renewed(valid_for)
            }
            Err(RenewalError::Refused) => {
                *state = Grants::default();
                tracing::warn!(
                    "Keycloak refused the anonymous catalog client; denying anonymous catalog access"
                );
                Renewal::Failed
            }
            Err(RenewalError::Outage) => {
                state.renewal_failing = true;
                let now = Instant::now();
                match state.valid_until {
                    Some(expiry) if now < expiry => tracing::warn!(
                        "Anonymous catalog grant renewal failed; current grants expire in {}s",
                        (expiry - now).as_secs()
                    ),
                    Some(expiry) if now < expiry + provider.config.stale_max => tracing::warn!(
                        "Anonymous catalog grant renewal failed; serving grants that expired {}s ago (ANONYMOUS_GRANTS_STALE_MAX_SECS={})",
                        (now - expiry).as_secs(),
                        provider.config.stale_max.as_secs()
                    ),
                    _ => tracing::warn!(
                        "Anonymous catalog grants unavailable; denying anonymous catalog access"
                    ),
                }
                Renewal::Failed
            }
        }
    }
}

/// Marks the refresh task stopped, loudly, whenever it ends.
struct TaskRunning(Arc<OidcVerifier>);

impl Drop for TaskRunning {
    fn drop(&mut self) {
        if let Some(provider) = &self.0.anonymous {
            provider.stats.task_running.store(false, Ordering::Relaxed);
        }
        tracing::error!(
            "The anonymous catalog grant refresh task stopped; anonymous grants will lapse at \
             their expiry. Restart the backend."
        );
    }
}

impl AnonymousPermissions {
    fn current(&self, now: Instant) -> BTreeSet<Role> {
        let state = self.state.read().unwrap_or_else(|e| e.into_inner());
        match state.valid_until {
            Some(expiry) if now < expiry => state.roles.clone(),
            Some(expiry) if state.renewal_failing && now < expiry + self.config.stale_max => {
                state.roles.clone()
            }
            _ => BTreeSet::new(),
        }
    }

    async fn fetch(
        &self,
        verifier: &OidcVerifier,
    ) -> Result<(BTreeSet<Role>, Duration), RenewalError> {
        let response = self
            .client
            .post(&self.config.token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.as_str()),
            ])
            .send()
            .await
            .map_err(|_| RenewalError::Outage)?;
        // Keycloak answers 400/401 for a disabled client or a wrong secret.
        if matches!(response.status().as_u16(), 400 | 401 | 403) {
            return Err(RenewalError::Refused);
        }
        let response = response
            .error_for_status()
            .map_err(|_| RenewalError::Outage)?
            .json::<TokenResponse>()
            .await
            .map_err(|_| RenewalError::Outage)?;
        let token = verifier
            .verify_catalog_token(&response.access_token)
            .await
            .map_err(|e| match e {
                VerifyError::Unavailable(_) => RenewalError::Outage,
                VerifyError::Invalid(_) => RenewalError::Refused,
            })?;
        let now: u64 = chrono::Utc::now()
            .timestamp()
            .try_into()
            .map_err(|_| RenewalError::Outage)?;
        let remaining = token
            .expires_at
            .checked_sub(now)
            .filter(|seconds| *seconds > 0)
            .ok_or(RenewalError::Refused)?;
        let roles = token
            .principal()
            .roles
            .into_iter()
            .filter(|role| role.is_catalog())
            .collect();
        let seconds = remaining
            .min(response.expires_in)
            .min(MAX_LIFETIME.as_secs());
        if seconds == 0 {
            return Err(RenewalError::Refused);
        }
        Ok((roles, Duration::from_secs(seconds)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::TestIssuer;
    use serde_json::{json, Value};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    };
    use uuid::Uuid;
    use warp::Filter;

    #[test]
    fn absent_configuration_denies_and_partial_configuration_fails_without_secrets() {
        assert!(AnonymousConfig::from_vars(|_| None).unwrap().is_none());
        assert!(AnonymousConfig::from_vars(|_| Some("  ".into()))
            .unwrap()
            .is_none());
        for (url, secret) in [
            (None, Some("secret")),
            (Some("https://idp.test/token"), None),
            (Some("https://idp.test/token"), Some("")),
            (Some("file:///token"), Some("secret")),
            (Some("https://user:secret@idp.test/token"), Some("secret")),
            (
                Some("https://idp.test/token?secret=credential"),
                Some("secret"),
            ),
        ] {
            let result = AnonymousConfig::from_vars(|key| match key {
                "OIDC_ANONYMOUS_TOKEN_URL" => url.map(str::to_owned),
                "OIDC_ANONYMOUS_CLIENT_SECRET" => secret.map(str::to_owned),
                _ => None,
            });
            let error = result.err().expect("invalid configuration");
            assert!(!error.contains("secret@") && !error.contains("credential"));
        }
    }

    /// Credentials require HTTPS unless a local loopback or private-network exception applies.
    #[test]
    fn token_url_requires_secure_transport_or_explicit_private_http() {
        for (url, allow_http, valid) in [
            ("https://idp.test/token", None, true),
            ("http://idp.test/token", None, false),
            ("http://idp.test/token", Some("false"), false),
            ("http://idp.test/token", Some("true"), true),
            ("http://localhost/token", None, true),
            ("http://127.0.0.1/token", None, true),
            ("http://[::1]/token", None, true),
            ("http://localhost.other.test/token", None, false),
            ("http://user:secret@idp.test/token", Some("true"), false),
            (
                "http://idp.test/token?secret=credential",
                Some("true"),
                false,
            ),
            ("http://idp.test/token#secret", Some("true"), false),
        ] {
            let result = AnonymousConfig::from_vars(|key| match key {
                "OIDC_ANONYMOUS_TOKEN_URL" => Some(url.into()),
                "OIDC_ANONYMOUS_CLIENT_SECRET" => Some("test-only".into()),
                "OIDC_ANONYMOUS_ALLOW_INSECURE_HTTP" => allow_http.map(str::to_owned),
                _ => None,
            });
            assert_eq!(result.is_ok(), valid, "{url}");
        }
    }

    /// Serve arbitrary OAuth token responses while the real verifier trusts TestIssuer's keys.
    /// A response object with a `status` member is sent with that HTTP status instead.
    async fn provider(
        response: Value,
        issuer: &TestIssuer,
        stale_max: Duration,
    ) -> (
        Arc<OidcVerifier>,
        Arc<RwLock<Value>>,
        Arc<AtomicUsize>,
        tokio::task::JoinHandle<()>,
    ) {
        let response = Arc::new(RwLock::new(response));
        let calls = Arc::new(AtomicUsize::new(0));
        let body = response.clone();
        let count = calls.clone();
        let route = warp::post()
            .and(warp::body::form::<std::collections::HashMap<String, String>>())
            .map(move |form: std::collections::HashMap<String, String>| {
                assert_eq!(
                    form.get("grant_type").map(String::as_str),
                    Some("client_credentials")
                );
                assert_eq!(
                    form.get("client_id").map(String::as_str),
                    Some("econgraph-anonymous")
                );
                assert_eq!(
                    form.get("client_secret").map(String::as_str),
                    Some("test-only")
                );
                count.fetch_add(1, Ordering::SeqCst);
                let body = body.read().unwrap().clone();
                let status = body
                    .get("status")
                    .and_then(Value::as_u64)
                    .map_or(200, |s| s as u16);
                warp::reply::with_status(
                    warp::reply::json(&body),
                    warp::http::StatusCode::from_u16(status).unwrap(),
                )
            });
        let (addr, server) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
        let task = tokio::spawn(server);
        let mut verifier = issuer.verifier().with_anonymous(AnonymousConfig {
            token_url: format!("http://{addr}/token"),
            client_id: "econgraph-anonymous".into(),
            client_secret: "test-only".into(),
            stale_max,
        });
        verifier.anonymous.as_mut().unwrap().client =
            reqwest::Client::builder().no_proxy().build().unwrap();
        (Arc::new(verifier), response, calls, task)
    }

    /// Move the current grant set's expiry to `ago` before now.
    fn expire(verifier: &OidcVerifier, ago: Duration) {
        let provider = verifier.anonymous.as_ref().unwrap();
        provider.state.write().unwrap().valid_until =
            Some(Instant::now().checked_sub(ago).unwrap());
    }

    fn renewal_failing(verifier: &OidcVerifier) -> bool {
        let provider = verifier.anonymous.as_ref().unwrap();
        provider.state.read().unwrap().renewal_failing
    }

    /// Point the verifier's token requests at a port where nothing listens: an outage.
    fn take_keycloak_down(verifier: &mut Arc<OidcVerifier>) {
        Arc::get_mut(verifier)
            .expect("no refresh task holds the verifier")
            .anonymous
            .as_mut()
            .unwrap()
            .config
            .token_url = "http://127.0.0.1:1/token".into();
    }

    const FRED: [Role; 1] = [Role::CatalogFredRead];

    #[test]
    fn stale_window_defaults_to_zero_and_rejects_bad_values() {
        let config = |stale: Option<&str>| {
            AnonymousConfig::from_vars(|key| match key {
                "OIDC_ANONYMOUS_TOKEN_URL" => Some("https://idp.test/token".into()),
                "OIDC_ANONYMOUS_CLIENT_SECRET" => Some("test-only".into()),
                "ANONYMOUS_GRANTS_STALE_MAX_SECS" => stale.map(str::to_owned),
                _ => None,
            })
        };
        let stale = |value| config(value).map(|c| c.unwrap().stale_max);
        assert_eq!(stale(None), Ok(Duration::ZERO));
        assert_eq!(stale(Some("0")), Ok(Duration::ZERO));
        assert_eq!(stale(Some(" 900 ")), Ok(Duration::from_secs(900)));
        assert_eq!(stale(Some("86400")), Ok(Duration::from_secs(86_400)));
        for bad in ["-1", "5m", "1.5", "86401"] {
            assert!(config(Some(bad)).is_err(), "{bad}");
        }
    }

    #[tokio::test]
    async fn renewal_filters_privileges_and_a_successful_fetch_narrows_access_at_once() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(
            Uuid::new_v4(),
            &[
                "catalog.fred:read",
                "catalog.bea:read",
                "admin.users:delete",
                "annotation:create",
            ],
        );
        for stale_max in [Duration::ZERO, Duration::from_secs(600)] {
            let (verifier, response, calls, task) = provider(
                json!({"access_token":token,"expires_in":300}),
                &issuer,
                stale_max,
            )
            .await;
            assert!(
                verifier.anonymous_catalog_roles().is_empty(),
                "nothing is granted before the first renewal"
            );
            assert!(matches!(
                verifier.renew_anonymous_grants().await,
                Renewal::Renewed(_)
            ));
            assert_eq!(
                verifier.anonymous_catalog_roles(),
                BTreeSet::from([Role::CatalogFredRead, Role::CatalogBeaRead])
            );
            // A change in Keycloak does not touch the grant set until the next renewal ...
            let narrower = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
            *response.write().unwrap() = json!({"access_token":narrower,"expires_in":300});
            assert_eq!(verifier.anonymous_catalog_roles().len(), 2);
            // ... which narrows access at once, while the old set was still unexpired, and
            // whatever the stale window.
            verifier.renew_anonymous_grants().await;
            assert_eq!(verifier.anonymous_catalog_roles(), BTreeSet::from(FRED));
            // Even after an outage left stale grants in place, a later success replaces them.
            expire(&verifier, Duration::from_millis(1));
            *response.write().unwrap() = json!({"status":503});
            assert_eq!(verifier.renew_anonymous_grants().await, Renewal::Failed);
            let revoked = issuer.mint_token(Uuid::new_v4(), &[]);
            *response.write().unwrap() = json!({"access_token":revoked,"expires_in":300});
            verifier.renew_anonymous_grants().await;
            assert!(
                verifier.anonymous_catalog_roles().is_empty(),
                "{stale_max:?}"
            );
            assert!(!renewal_failing(&verifier));
            assert_eq!(calls.load(Ordering::SeqCst), 4);
            task.abort();
        }
    }

    /// Readers copy the current grant set: a renewal stuck on Keycloak never delays them.
    #[tokio::test]
    async fn readers_are_served_from_memory_while_renewal_hangs() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
        let (verifier, _, _, task) = provider(
            json!({"access_token":token,"expires_in":300}),
            &issuer,
            Duration::ZERO,
        )
        .await;
        verifier.renew_anonymous_grants().await;
        let hanging = warp::post().and_then(|| async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            Ok::<_, std::convert::Infallible>(warp::reply())
        });
        let (addr, server) = warp::serve(hanging).bind_ephemeral(([127, 0, 0, 1], 0));
        let hang = tokio::spawn(server);
        let mut verifier = verifier;
        Arc::get_mut(&mut verifier)
            .unwrap()
            .anonymous
            .as_mut()
            .unwrap()
            .config
            .token_url = format!("http://{addr}/token");
        let renewing = {
            let verifier = verifier.clone();
            tokio::spawn(async move { verifier.renew_anonymous_grants().await })
        };
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            !renewing.is_finished(),
            "the renewal is waiting on Keycloak"
        );
        let started = Instant::now();
        for _ in 0..100 {
            assert_eq!(verifier.anonymous_catalog_roles(), BTreeSet::from(FRED));
        }
        assert!(started.elapsed() < Duration::from_secs(1));
        renewing.abort();
        hang.abort();
        task.abort();
    }

    #[tokio::test]
    async fn outages_fail_closed_after_expiry_by_default() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
        let (mut verifier, _, _, task) = provider(
            json!({"access_token":token,"expires_in":300}),
            &issuer,
            Duration::ZERO,
        )
        .await;
        verifier.renew_anonymous_grants().await;
        take_keycloak_down(&mut verifier);
        assert_eq!(verifier.renew_anonymous_grants().await, Renewal::Failed);
        assert_eq!(
            verifier.anonymous_catalog_roles(),
            BTreeSet::from(FRED),
            "unexpired grants survive an outage"
        );
        expire(&verifier, Duration::from_millis(1));
        assert!(verifier.anonymous_catalog_roles().is_empty());
        assert_eq!(verifier.renew_anonymous_grants().await, Renewal::Failed);
        assert!(verifier.anonymous_catalog_roles().is_empty());
        task.abort();
    }

    #[tokio::test]
    async fn stale_window_serves_the_last_grants_only_while_renewal_fails() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
        let window = Duration::from_secs(60);
        let (mut verifier, _, _, task) = provider(
            json!({"access_token":token,"expires_in":300}),
            &issuer,
            window,
        )
        .await;
        verifier.renew_anonymous_grants().await;
        expire(&verifier, Duration::from_millis(1));
        assert!(
            verifier.anonymous_catalog_roles().is_empty(),
            "expired grants are not served until a renewal has failed"
        );
        take_keycloak_down(&mut verifier);
        assert_eq!(verifier.renew_anonymous_grants().await, Renewal::Failed);
        assert!(renewal_failing(&verifier));
        assert_eq!(verifier.anonymous_catalog_roles(), BTreeSet::from(FRED));
        expire(&verifier, window - Duration::from_secs(1));
        assert_eq!(verifier.anonymous_catalog_roles(), BTreeSet::from(FRED));
        expire(&verifier, window);
        assert!(
            verifier.anonymous_catalog_roles().is_empty(),
            "the stale window is bounded"
        );
        task.abort();
    }

    /// Refusals, rejected tokens and expired tokens are not outages: they drop the grants at
    /// once, stale window or not.
    #[tokio::test]
    async fn refusals_and_bad_tokens_clear_grants_even_with_a_stale_window() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
        let mut expired = issuer.claims(Uuid::new_v4(), &["catalog.fred:read"]);
        // The normal verifier permits clock skew; the anonymous grants must still expire.
        expired["exp"] = json!(crate::testkit::now() - 1);
        let bad_key = crate::testkit::SigningKey::generate("test-key-1");
        let wrong_signature = bad_key.sign(&issuer.claims(Uuid::new_v4(), &["catalog.fred:read"]));
        for refusal in [
            json!({"status":401,"error":"unauthorized_client"}),
            json!({"status":400,"error":"invalid_client"}),
            json!({"access_token":"not.a.jwt","expires_in":300}),
            json!({"access_token":issuer.sign(&expired),"expires_in":300}),
            json!({"access_token":wrong_signature,"expires_in":300}),
        ] {
            let (verifier, response, _, task) = provider(
                json!({"access_token":token,"expires_in":300}),
                &issuer,
                Duration::from_secs(600),
            )
            .await;
            verifier.renew_anonymous_grants().await;
            assert_eq!(verifier.anonymous_catalog_roles(), BTreeSet::from(FRED));
            *response.write().unwrap() = refusal.clone();
            assert_eq!(verifier.renew_anonymous_grants().await, Renewal::Failed);
            assert!(
                verifier.anonymous_catalog_roles().is_empty(),
                "{refusal} must revoke at once"
            );
            task.abort();
        }
    }

    /// Deny-all is a valid grant set, while signature failures remain failed renewals.
    #[tokio::test]
    async fn omitted_roles_are_an_empty_grant_set() {
        let issuer = TestIssuer::start().await;
        let mut claims = issuer.claims(Uuid::new_v4(), &[]);
        claims.as_object_mut().unwrap().remove("roles");
        let token = issuer.sign(&claims);
        assert!(
            issuer.verifier().verify(&token).await.is_err(),
            "the ordinary signed-in verification contract remains strict"
        );
        let (verifier, _, _, task) = provider(
            json!({"access_token":token,"expires_in":300}),
            &issuer,
            Duration::ZERO,
        )
        .await;
        assert!(matches!(
            verifier.renew_anonymous_grants().await,
            Renewal::Renewed(_)
        ));
        assert!(verifier.anonymous_catalog_roles().is_empty());
        let provider = verifier.anonymous.as_ref().unwrap();
        assert!(provider
            .state
            .read()
            .unwrap()
            .valid_until
            .is_some_and(|expiry| expiry > Instant::now()));
        task.abort();
    }

    #[tokio::test]
    async fn grant_lifetime_is_bounded_by_the_response() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
        let (verifier, _, _, task) = provider(
            json!({"access_token":token,"expires_in":1}),
            &issuer,
            Duration::ZERO,
        )
        .await;
        assert_eq!(
            verifier.renew_anonymous_grants().await,
            Renewal::Renewed(Duration::from_secs(1))
        );
        let provider = verifier.anonymous.as_ref().unwrap();
        assert!(
            provider.state.read().unwrap().valid_until.unwrap()
                <= Instant::now() + Duration::from_secs(1)
        );
        assert!(
            issuer.verifier().anonymous_catalog_roles().is_empty(),
            "disabled provider denies"
        );
        task.abort();
    }

    /// The background task fetches at once and keeps renewing without any request.
    #[tokio::test]
    async fn background_refresh_fetches_and_renews_without_requests() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
        let (verifier, _, calls, task) = provider(
            json!({"access_token":token,"expires_in":1}),
            &issuer,
            Duration::ZERO,
        )
        .await;
        let refresh = verifier.spawn_anonymous_refresh().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while calls.load(Ordering::SeqCst) < 3 {
            assert!(Instant::now() < deadline, "renewals did not continue");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(verifier.anonymous_catalog_roles(), BTreeSet::from(FRED));
        refresh.abort();
        task.abort();
        assert!(Arc::new(issuer.verifier())
            .spawn_anonymous_refresh()
            .is_none());
    }

    /// However long the token lives, a grant set lasts at most five minutes, so a revocation
    /// is fetched within that bound.
    #[tokio::test]
    async fn grant_lifetime_is_capped_at_five_minutes() {
        let issuer = TestIssuer::start().await;
        let mut claims = issuer.claims(Uuid::new_v4(), &["catalog.fred:read"]);
        claims["exp"] = json!(crate::testkit::now() + 3600);
        let (verifier, _, _, task) = provider(
            json!({"access_token":issuer.sign(&claims),"expires_in":3600}),
            &issuer,
            Duration::ZERO,
        )
        .await;
        assert_eq!(
            verifier.renew_anonymous_grants().await,
            Renewal::Renewed(MAX_LIFETIME)
        );
        assert_eq!(MAX_LIFETIME, Duration::from_secs(300));
        let provider = verifier.anonymous.as_ref().unwrap();
        assert!(
            provider.state.read().unwrap().valid_until.unwrap()
                <= Instant::now() + Duration::from_secs(300)
        );
        task.abort();
    }

    /// The counters metrics read: outcomes, failures since the last success, the grants held
    /// now, and whether stale grants are being served.
    #[tokio::test]
    async fn renewal_stats_count_outcomes_and_the_current_grants() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read", "catalog.bea:read"]);
        let (mut verifier, response, _, task) = provider(
            json!({"access_token":token,"expires_in":300}),
            &issuer,
            Duration::from_secs(600),
        )
        .await;
        let stats = |v: &OidcVerifier| v.anonymous_renewal_stats().unwrap();
        assert_eq!(stats(&verifier).current_grants, 0);
        verifier.renew_anonymous_grants().await;
        let after_success = stats(&verifier);
        assert_eq!(
            (after_success.successes, after_success.current_grants),
            (1, 2)
        );
        *response.write().unwrap() = json!({"status":401});
        verifier.renew_anonymous_grants().await;
        let after_refusal = stats(&verifier);
        assert_eq!(
            (after_refusal.refusals, after_refusal.consecutive_failures),
            (1, 1)
        );
        assert_eq!(after_refusal.current_grants, 0);
        *response.write().unwrap() = json!({"access_token":token,"expires_in":300});
        verifier.renew_anonymous_grants().await;
        expire(&verifier, Duration::from_millis(1));
        take_keycloak_down(&mut verifier);
        verifier.renew_anonymous_grants().await;
        verifier.renew_anonymous_grants().await;
        let stale = stats(&verifier);
        assert_eq!((stale.successes, stale.outages), (2, 2));
        assert_eq!(stale.consecutive_failures, 2);
        assert!(stale.serving_stale);
        assert_eq!(stale.current_grants, 2);
        assert!(!stale.task_running);
        assert!(issuer.verifier().anonymous_renewal_stats().is_none());
        task.abort();
    }

    /// The refresh task reports itself running, and not running once it has ended.
    #[tokio::test]
    async fn a_stopped_refresh_task_is_reported() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
        let (verifier, _, _, task) = provider(
            json!({"access_token":token,"expires_in":300}),
            &issuer,
            Duration::ZERO,
        )
        .await;
        let refresh = verifier.spawn_anonymous_refresh().unwrap();
        assert!(verifier.anonymous_renewal_stats().unwrap().task_running);
        refresh.abort();
        assert!(refresh.await.unwrap_err().is_cancelled());
        assert!(!verifier.anonymous_renewal_stats().unwrap().task_running);
        task.abort();
    }
}
