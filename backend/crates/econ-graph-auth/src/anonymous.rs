//! Shared anonymous catalog grants, obtained with server-side client credentials.
//! No service-account identity or non-catalog capability is attached to visitors.

use crate::{OidcVerifier, Role};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Server-only client configuration. Deliberately does not implement Debug.
pub struct AnonymousConfig {
    /// Internal token endpoint, which may differ from the public issuer URL.
    pub token_url: String,
    /// Dedicated client holding the anonymous catalog grants.
    pub client_id: String,
    /// Confidential client credential, never sent to visitors.
    pub client_secret: String,
}

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
        if !matches!(parsed.scheme(), "http" | "https")
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
        Ok(Some(Self {
            token_url,
            client_id,
            client_secret,
        }))
    }
}

pub(crate) struct AnonymousPermissions {
    config: AnonymousConfig,
    client: reqwest::Client,
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    roles: BTreeSet<Role>,
    valid_until: Option<Instant>,
    retry_after: Option<Instant>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

impl OidcVerifier {
    /// Enable anonymous grants without fetching a token at startup.
    pub fn with_anonymous(mut self, config: AnonymousConfig) -> Self {
        self.anonymous = Some(AnonymousPermissions {
            config,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("HTTP client with default TLS settings"),
            cache: Mutex::new(Cache::default()),
        });
        self
    }

    /// Return only unexpired catalog roles. Failed refreshes deny access and are rate-limited.
    /// The mutex coalesces concurrent renewals across requests in this process.
    pub async fn anonymous_catalog_roles(&self) -> BTreeSet<Role> {
        let Some(provider) = &self.anonymous else {
            return BTreeSet::new();
        };
        let mut cache = provider.cache.lock().await;
        let now = Instant::now();
        if cache.valid_until.is_some_and(|expiry| now < expiry) {
            return cache.roles.clone();
        }
        cache.roles.clear();
        cache.valid_until = None;
        if cache.retry_after.is_some_and(|retry| now < retry) {
            return BTreeSet::new();
        }
        cache.retry_after = Some(now + Duration::from_secs(10));
        match provider.fetch(self).await {
            Ok((roles, valid_for)) => {
                // Measure from BEFORE the HTTP requests, never extending the token lifetime.
                cache.valid_until = now.checked_add(valid_for);
                cache.roles = roles;
                cache.retry_after = None;
                if cache
                    .valid_until
                    .is_some_and(|expiry| Instant::now() < expiry)
                {
                    cache.roles.clone()
                } else {
                    BTreeSet::new()
                }
            }
            Err(()) => {
                tracing::warn!("Anonymous catalog grants unavailable; denying catalog access");
                BTreeSet::new()
            }
        }
    }
}

impl AnonymousPermissions {
    async fn fetch(&self, verifier: &OidcVerifier) -> Result<(BTreeSet<Role>, Duration), ()> {
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
            .map_err(|_| ())?
            .error_for_status()
            .map_err(|_| ())?
            .json::<TokenResponse>()
            .await
            .map_err(|_| ())?;
        let token = verifier
            .verify_catalog_token(&response.access_token)
            .await
            .map_err(|_| ())?;
        let remaining = token
            .expires_at
            .checked_sub(chrono::Utc::now().timestamp().try_into().map_err(|_| ())?)
            .filter(|seconds| *seconds > 0)
            .ok_or(())?;
        let roles = token
            .principal()
            .roles
            .into_iter()
            .filter(|role| role.is_catalog())
            .collect();
        let seconds = remaining.min(response.expires_in).min(300);
        if seconds == 0 {
            return Err(());
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

    /// Serve arbitrary OAuth token responses while the real verifier trusts TestIssuer's keys.
    async fn provider(
        response: Value,
        issuer: &TestIssuer,
    ) -> (
        OidcVerifier,
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
                warp::reply::json(&*body.read().unwrap())
            });
        let (addr, server) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
        let task = tokio::spawn(server);
        let mut verifier = issuer.verifier().with_anonymous(AnonymousConfig {
            token_url: format!("http://{addr}/token"),
            client_id: "econgraph-anonymous".into(),
            client_secret: "test-only".into(),
        });
        verifier.anonymous.as_mut().unwrap().client =
            reqwest::Client::builder().no_proxy().build().unwrap();
        (verifier, response, calls, task)
    }

    #[tokio::test]
    async fn cache_coalesces_fetches_filters_privileges_and_applies_revocations() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(
            Uuid::new_v4(),
            &[
                "catalog.fred:read",
                "admin.users:delete",
                "annotation:create",
            ],
        );
        let (verifier, response, calls, task) =
            provider(json!({"access_token":token,"expires_in":300}), &issuer).await;
        let (a, b, c) = tokio::join!(
            verifier.anonymous_catalog_roles(),
            verifier.anonymous_catalog_roles(),
            verifier.anonymous_catalog_roles()
        );
        let expected = BTreeSet::from([Role::CatalogFredRead]);
        assert_eq!(a, expected);
        assert_eq!(b, expected);
        assert_eq!(c, expected);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // A change in Keycloak does not mutate an already issued grant set.
        let next = issuer.mint_token(Uuid::new_v4(), &["catalog.bea:read"]);
        *response.write().unwrap() = json!({"access_token":next,"expires_in":300});
        assert_eq!(verifier.anonymous_catalog_roles().await, expected);
        verifier
            .anonymous
            .as_ref()
            .unwrap()
            .cache
            .lock()
            .await
            .valid_until = Some(Instant::now());
        assert_eq!(
            verifier.anonymous_catalog_roles().await,
            BTreeSet::from([Role::CatalogBeaRead])
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        task.abort();
    }

    #[tokio::test]
    async fn expired_cache_and_invalid_or_expired_responses_never_extend_grants() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
        let (verifier, response, calls, task) =
            provider(json!({"access_token":token,"expires_in":300}), &issuer).await;
        assert_eq!(
            verifier.anonymous_catalog_roles().await,
            BTreeSet::from([Role::CatalogFredRead])
        );
        *response.write().unwrap() = json!({"access_token":"not.a.jwt","expires_in":300});
        verifier
            .anonymous
            .as_ref()
            .unwrap()
            .cache
            .lock()
            .await
            .valid_until = Some(Instant::now());
        assert!(verifier.anonymous_catalog_roles().await.is_empty());
        assert!(verifier.anonymous_catalog_roles().await.is_empty());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "failed renewals are rate limited"
        );
        let mut claims = issuer.claims(Uuid::new_v4(), &["catalog.fred:read"]);
        // The normal verifier permits clock skew; the anonymous cache must still expire strictly.
        claims["exp"] = json!(crate::testkit::now() - 1);
        *response.write().unwrap() = json!({"access_token":issuer.sign(&claims),"expires_in":300});
        verifier
            .anonymous
            .as_ref()
            .unwrap()
            .cache
            .lock()
            .await
            .retry_after = None;
        assert!(verifier.anonymous_catalog_roles().await.is_empty());
        task.abort();
    }

    /// Deny-all is a valid cached policy, while signature failures remain failed renewals.
    #[tokio::test]
    async fn omitted_roles_cache_empty_grants_but_invalid_signatures_never_cache() {
        let issuer = TestIssuer::start().await;
        let mut claims = issuer.claims(Uuid::new_v4(), &[]);
        claims.as_object_mut().unwrap().remove("roles");
        let token = issuer.sign(&claims);
        assert!(
            issuer.verifier().verify(&token).await.is_err(),
            "the ordinary signed-in verification contract remains strict"
        );
        let (verifier, response, calls, task) =
            provider(json!({"access_token":token,"expires_in":300}), &issuer).await;
        let (first, second) = tokio::join!(
            verifier.anonymous_catalog_roles(),
            verifier.anonymous_catalog_roles()
        );
        assert!(first.is_empty() && second.is_empty());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "empty grants are cached and coalesced"
        );
        let provider = verifier.anonymous.as_ref().unwrap();
        assert!(provider
            .cache
            .lock()
            .await
            .valid_until
            .is_some_and(|expiry| expiry > Instant::now()));
        let bad_key = crate::testkit::SigningKey::generate("test-key-1");
        *response.write().unwrap() = json!({"access_token":bad_key.sign(&claims),"expires_in":300});
        provider.cache.lock().await.valid_until = Some(Instant::now());
        assert!(verifier.anonymous_catalog_roles().await.is_empty());
        assert!(
            provider.cache.lock().await.valid_until.is_none(),
            "missing roles cannot bypass signature verification"
        );
        assert!(verifier.anonymous_catalog_roles().await.is_empty());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "invalid signatures are rate-limited, never cached as grants"
        );
        task.abort();
    }

    #[tokio::test]
    async fn cache_lifetime_is_bounded_by_response_and_outages_deny_after_expiry() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
        let (mut verifier, _, _, task) =
            provider(json!({"access_token":token,"expires_in":1}), &issuer).await;
        assert!(!verifier.anonymous_catalog_roles().await.is_empty());
        let provider = verifier.anonymous.as_ref().unwrap();
        assert!(
            provider.cache.lock().await.valid_until.unwrap()
                <= Instant::now() + Duration::from_secs(1)
        );
        task.abort();
        verifier.anonymous.as_mut().unwrap().config.token_url = "http://127.0.0.1:1/token".into();
        assert!(
            !verifier.anonymous_catalog_roles().await.is_empty(),
            "valid cache survives an outage"
        );
        verifier
            .anonymous
            .as_ref()
            .unwrap()
            .cache
            .lock()
            .await
            .valid_until = Some(Instant::now());
        assert!(verifier.anonymous_catalog_roles().await.is_empty());
        assert!(
            issuer.verifier().anonymous_catalog_roles().await.is_empty(),
            "disabled provider denies"
        );
    }
}
