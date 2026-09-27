//! # Keycloak access-token verification
//!
//! The API trusts access tokens issued by one OpenID Connect provider (Keycloak), configured
//! by [`OidcConfig`]. [`OidcVerifier`] checks a token's RS256 signature against the provider's
//! JWKS and its `iss`, `aud`, `exp` and `nbf` claims, and returns the caller's `sub` and the
//! top-level `roles` claim.
//!
//! The JWKS is fetched lazily, on the first token that needs it, so the API starts and serves
//! anonymous requests while the provider is down. Keys are cached; a token signed with a key id
//! the cache does not know triggers a refetch (rate-limited, since the key id is chosen by the
//! caller), which is how a rotated signing key is picked up. The cache is also refreshed when it
//! is older than [`MAX_KEY_AGE`], so a key the provider has removed stops being trusted; while
//! the provider is unreachable, cached keys stay in use for up to [`MAX_STALE_KEY_AGE`].

use std::collections::HashMap;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet, KeyAlgorithm, PublicKeyUse};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::roles::Principal;

/// Audience the API expects when `OIDC_AUDIENCE` is unset: the Keycloak client of the API.
pub const DEFAULT_AUDIENCE: &str = "econ-graph-api";

/// How long fetched keys are trusted before the next token triggers a refetch.
pub const MAX_KEY_AGE: Duration = Duration::from_secs(15 * 60);

/// How long fetched keys stay usable while the provider cannot be reached to refresh them.
pub const MAX_STALE_KEY_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Minimum time between two JWKS fetches caused by unknown key ids or failed fetches.
pub const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(10);

/// Clock skew allowed on `exp` and `nbf`, in seconds.
const LEEWAY_SECS: u64 = 30;

/// Timeout for each request to the provider.
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

/// Where the API gets its tokens from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcConfig {
    /// The provider's issuer URL, exactly as it appears in the tokens' `iss` claim
    /// (for Keycloak, `<base>/realms/<realm>`). Discovery is read from
    /// `<issuer>/.well-known/openid-configuration`.
    pub issuer: String,
    /// The audience a token must name in `aud`.
    pub audience: String,
    /// Fetch the JWKS from this URL instead of the one discovery names. Needed when the issuer
    /// URL, which is what browsers see, is not reachable from the API (Keycloak behind an
    /// ingress, or on the host while the API runs in a container).
    pub jwks_url: Option<String>,
}

/// Why the OIDC settings were refused at startup.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OidcConfigError {
    /// `OIDC_REQUIRED` is set but `OIDC_ISSUER` is not.
    #[error("OIDC_ISSUER must be set: this build requires sign-in through the identity provider")]
    Missing,
    /// A URL setting is not an absolute http(s) URL.
    #[error("{name} must be an absolute http(s) URL, got {value:?}")]
    InvalidUrl { name: &'static str, value: String },
    /// `OIDC_AUDIENCE` is set but empty.
    #[error("OIDC_AUDIENCE must not be empty")]
    EmptyAudience,
    /// `OIDC_REQUIRED` is not a boolean.
    #[error("OIDC_REQUIRED must be true or false, got {0:?}")]
    InvalidRequired(String),
}

impl OidcConfig {
    /// Read the settings from the environment; see [`Self::from_vars`].
    pub fn from_env() -> Result<Option<Self>, OidcConfigError> {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// Read the settings through `var`:
    ///
    /// - `OIDC_ISSUER`: unset or empty disables sign-in, and every caller is anonymous.
    /// - `OIDC_AUDIENCE`: defaults to [`DEFAULT_AUDIENCE`].
    /// - `OIDC_JWKS_URL`: optional, see [`OidcConfig::jwks_url`].
    /// - `OIDC_REQUIRED`: `true` refuses to start without `OIDC_ISSUER` (production).
    ///
    /// A setting that is present but malformed is an error, so a typo fails at startup rather
    /// than turning sign-in off.
    pub fn from_vars(
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, OidcConfigError> {
        let set = |name: &str| {
            var(name)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };

        let required = match set("OIDC_REQUIRED").map(|v| v.to_ascii_lowercase()) {
            None => false,
            Some(v) if v == "true" || v == "1" => true,
            Some(v) if v == "false" || v == "0" => false,
            Some(v) => return Err(OidcConfigError::InvalidRequired(v)),
        };

        let Some(issuer) = set("OIDC_ISSUER") else {
            return if required {
                Err(OidcConfigError::Missing)
            } else {
                Ok(None)
            };
        };
        let issuer = checked_url("OIDC_ISSUER", &issuer)?
            .trim_end_matches('/')
            .to_string();

        let audience = match var("OIDC_AUDIENCE") {
            None => DEFAULT_AUDIENCE.to_string(),
            Some(v) if v.trim().is_empty() => return Err(OidcConfigError::EmptyAudience),
            Some(v) => v.trim().to_string(),
        };

        let jwks_url = set("OIDC_JWKS_URL")
            .map(|url| checked_url("OIDC_JWKS_URL", &url).map(str::to_string))
            .transpose()?;

        Ok(Some(Self {
            issuer,
            audience,
            jwks_url,
        }))
    }

    /// The discovery document's URL.
    pub fn discovery_url(&self) -> String {
        format!("{}/.well-known/openid-configuration", self.issuer)
    }
}

fn checked_url<'a>(name: &'static str, value: &'a str) -> Result<&'a str, OidcConfigError> {
    let invalid = || OidcConfigError::InvalidUrl {
        name,
        value: value.to_string(),
    };
    let url = reqwest::Url::parse(value).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none_or(str::is_empty)
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(value)
}

/// Why a bearer token was not accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    /// The token is malformed, wrongly signed, expired, for another issuer or audience, or
    /// lacks a claim the API needs. The caller should get a new token.
    #[error("invalid token: {0}")]
    Invalid(String),
    /// The provider's keys could not be fetched, so the token could not be checked. Nothing
    /// about the token is known; it is not accepted.
    #[error("identity provider unavailable: {0}")]
    Unavailable(String),
}

impl VerifyError {
    fn invalid(why: impl Into<String>) -> Self {
        Self::Invalid(why.into())
    }
}

/// The claims of an accepted access token that the API uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedToken {
    /// The caller's user id (`sub`), which keys their `users` row.
    pub subject: Uuid,
    /// The top-level `roles` claim, as sent: names outside the catalog are kept here and
    /// skipped by [`Principal::from_claim`].
    pub roles: Vec<String>,
    /// `email`, if the token carries one.
    pub email: Option<String>,
    /// `email_verified`, false when absent.
    pub email_verified: bool,
    /// A display name: `name`, else `preferred_username`.
    pub name: Option<String>,
}

impl VerifiedToken {
    /// The caller and the fine-grained roles the token grants.
    pub fn principal(&self) -> Principal {
        Principal::from_claim(self.subject, &self.roles)
    }
}

#[derive(Deserialize)]
struct AccessClaims {
    sub: String,
    typ: Option<String>,
    roles: Option<Vec<String>>,
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
    preferred_username: Option<String>,
}

#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    jwks_uri: String,
}

#[derive(Default)]
struct KeyCache {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Option<Instant>,
}

#[derive(Default)]
struct RefreshState {
    /// The JWKS URL, once discovery (or `OIDC_JWKS_URL`) has named it.
    jwks_url: Option<String>,
    /// When the last fetch was attempted, successful or not.
    last_attempt: Option<Instant>,
    /// Why the last fetch failed, until one succeeds.
    last_error: Option<String>,
}

/// Verifies access tokens from the configured provider. Cheap to share: wrap it in an `Arc`.
pub struct OidcVerifier {
    config: OidcConfig,
    client: reqwest::Client,
    validation: Validation,
    min_refresh_interval: Duration,
    cache: RwLock<KeyCache>,
    refresh: Mutex<RefreshState>,
}

impl std::fmt::Debug for OidcVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcVerifier")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl OidcVerifier {
    /// A verifier for `config` with its own HTTP client. Makes no request until a token needs
    /// checking.
    pub fn new(config: OidcConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .connect_timeout(HTTP_TIMEOUT)
            .build()
            .expect("HTTP client with default TLS settings");
        Self::with_client(config, client)
    }

    /// A verifier that fetches keys with `client`.
    pub fn with_client(config: OidcConfig, client: reqwest::Client) -> Self {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&config.issuer]);
        validation.set_audience(&[&config.audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.leeway = LEEWAY_SECS;
        let refresh = RefreshState {
            jwks_url: config.jwks_url.clone(),
            last_attempt: None,
            last_error: None,
        };
        Self {
            config,
            client,
            validation,
            min_refresh_interval: MIN_REFRESH_INTERVAL,
            cache: RwLock::default(),
            refresh: Mutex::new(refresh),
        }
    }

    /// Change the minimum time between fetches triggered by unknown key ids (tests use zero).
    pub fn with_min_refresh_interval(mut self, interval: Duration) -> Self {
        self.min_refresh_interval = interval;
        self
    }

    /// Pretend the cached keys were fetched `by` earlier (tests of expiry).
    #[cfg(test)]
    async fn age_cache(&self, by: Duration) {
        let mut cache = self.cache.write().await;
        cache.fetched_at = cache.fetched_at.and_then(|at| at.checked_sub(by));
    }

    /// The settings this verifier checks tokens against.
    pub fn config(&self) -> &OidcConfig {
        &self.config
    }

    /// Check `token` and return the claims the API uses.
    pub async fn verify(&self, token: &str) -> Result<VerifiedToken, VerifyError> {
        let header = jsonwebtoken::decode_header(token)
            .map_err(|e| VerifyError::invalid(format!("malformed token: {e}")))?;
        if header.alg != Algorithm::RS256 {
            return Err(VerifyError::invalid(format!(
                "algorithm {:?} is not accepted",
                header.alg
            )));
        }
        let kid = header
            .kid
            .ok_or_else(|| VerifyError::invalid("token has no key id"))?;
        let key = self.key(&kid).await?;

        let data = jsonwebtoken::decode::<AccessClaims>(token, &key, &self.validation)
            .map_err(|e| VerifyError::invalid(e.to_string()))?;
        let claims = data.claims;

        // Keycloak marks access tokens `Bearer`; ID and refresh tokens say `ID` and `Refresh`.
        match claims.typ.as_deref() {
            Some(typ) if typ.eq_ignore_ascii_case("Bearer") => {}
            typ => {
                return Err(VerifyError::invalid(format!(
                    "token type {typ:?} is not accepted"
                )))
            }
        }
        let subject = Uuid::parse_str(&claims.sub)
            .map_err(|_| VerifyError::invalid("subject is not a UUID"))?;
        let roles = claims
            .roles
            .ok_or_else(|| VerifyError::invalid("token has no roles claim"))?;
        let non_empty =
            |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

        Ok(VerifiedToken {
            subject,
            roles,
            email: non_empty(claims.email),
            email_verified: claims.email_verified.unwrap_or(false),
            name: non_empty(claims.name).or_else(|| non_empty(claims.preferred_username)),
        })
    }

    /// The signing key with id `kid`.
    ///
    /// A fresh cache answers directly. A cache older than [`MAX_KEY_AGE`] still answers, while
    /// one request refreshes it (others keep using the old keys rather than wait on the
    /// provider). A key id the cache lacks makes the request wait for a refetch, at most one per
    /// [`Self::with_min_refresh_interval`]. Keys older than [`MAX_STALE_KEY_AGE`] are not used:
    /// if the provider stays unreachable that long, tokens can no longer be checked.
    async fn key(&self, kid: &str) -> Result<DecodingKey, VerifyError> {
        let started = Instant::now();
        if let Some(key) = self.cached(kid, MAX_KEY_AGE).await {
            return Ok(key);
        }

        if self.cached(kid, MAX_STALE_KEY_AGE).await.is_some() {
            // Known key, stale cache: refresh unless another request already is.
            if let Ok(mut refresh) = self.refresh.try_lock() {
                self.refresh_keys(&mut refresh).await;
            }
            return match self.cached(kid, MAX_STALE_KEY_AGE).await {
                Some(key) => Ok(key),
                None => Err(VerifyError::invalid(format!("unknown key id {kid:?}"))),
            };
        }

        let mut refresh = self.refresh.lock().await;
        // Another request may have refreshed while this one waited for the lock.
        if self.cached(kid, MAX_KEY_AGE).await.is_none() {
            self.refresh_keys(&mut refresh).await;
        }
        let last_error = refresh.last_error.clone();
        drop(refresh);

        if let Some(key) = self.cached(kid, MAX_STALE_KEY_AGE).await {
            return Ok(key);
        }
        let fetched_at = self.cache.read().await.fetched_at;
        // A key id is unknown only if keys fetched since this request arrived lack it. When
        // the refetch was throttled, or failed, the kid may be a new key the provider has not
        // told us about yet, so the token can't be checked rather than being invalid.
        let checked = fetched_at.is_some_and(|at| at >= started);
        if checked && last_error.is_none() {
            Err(VerifyError::invalid(format!("unknown key id {kid:?}")))
        } else {
            Err(VerifyError::Unavailable(last_error.unwrap_or_else(|| {
                format!("key id {kid:?} not checked: signing keys were refetched too recently")
            })))
        }
    }

    /// Refetch the JWKS unless the last attempt was too recent. A failed fetch keeps the old
    /// keys and is remembered in `refresh.last_error`.
    async fn refresh_keys(&self, refresh: &mut RefreshState) {
        let throttled = refresh
            .last_attempt
            .is_some_and(|at| at.elapsed() < self.min_refresh_interval);
        if throttled {
            return;
        }
        refresh.last_attempt = Some(Instant::now());
        match self.fetch_keys(refresh).await {
            Ok(keys) => {
                tracing::info!(
                    "fetched {} signing keys from the identity provider",
                    keys.len()
                );
                refresh.last_error = None;
                *self.cache.write().await = KeyCache {
                    keys,
                    fetched_at: Some(Instant::now()),
                };
            }
            Err(err) => {
                tracing::warn!("could not fetch the identity provider's keys: {err}");
                refresh.last_error = Some(err);
            }
        }
    }

    /// The cached key `kid`, if the cache was fetched less than `max_age` ago.
    async fn cached(&self, kid: &str, max_age: Duration) -> Option<DecodingKey> {
        let cache = self.cache.read().await;
        if !cache.fetched_at.is_some_and(|at| at.elapsed() < max_age) {
            return None;
        }
        cache.keys.get(kid).cloned()
    }

    async fn fetch_keys(
        &self,
        refresh: &mut RefreshState,
    ) -> Result<HashMap<String, DecodingKey>, String> {
        let jwks_url = match &refresh.jwks_url {
            Some(url) => url.clone(),
            None => {
                let url = self.discover().await?;
                refresh.jwks_url = Some(url.clone());
                url
            }
        };
        let jwks: JwkSet = self.get_json(&jwks_url).await?;
        Ok(signing_keys(&jwks))
    }

    async fn discover(&self) -> Result<String, String> {
        let discovery: Discovery = self.get_json(&self.config.discovery_url()).await?;
        // OpenID Connect Discovery 1.0, section 4.3: the document must name the same issuer.
        if discovery.issuer.trim_end_matches('/') != self.config.issuer {
            return Err(format!(
                "discovery names issuer {:?}, expected {:?}",
                discovery.issuer, self.config.issuer
            ));
        }
        checked_url("jwks_uri", &discovery.jwks_uri).map_err(|e| e.to_string())?;
        Ok(discovery.jwks_uri)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| format!("GET {url}: {e}"))?;
        response
            .json()
            .await
            .map_err(|e| format!("GET {url}: unreadable response: {e}"))
    }
}

/// The RSA signing keys of `jwks`, by key id. Keys for encryption, for other algorithms or
/// without an id are left out.
fn signing_keys(jwks: &JwkSet) -> HashMap<String, DecodingKey> {
    jwks.keys
        .iter()
        .filter(|jwk| matches!(jwk.algorithm, AlgorithmParameters::RSA(_)))
        .filter(|jwk| {
            matches!(
                jwk.common.public_key_use,
                None | Some(PublicKeyUse::Signature)
            )
        })
        .filter(|jwk| matches!(jwk.common.key_algorithm, None | Some(KeyAlgorithm::RS256)))
        .filter_map(|jwk| {
            let kid = jwk.common.key_id.clone()?;
            match DecodingKey::from_jwk(jwk) {
                Ok(key) => Some((kid, key)),
                Err(err) => {
                    tracing::warn!("skipping unusable signing key {kid:?}: {err}");
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn unset_issuer_disables_sign_in() {
        assert_eq!(OidcConfig::from_vars(vars(&[])), Ok(None));
        assert_eq!(
            OidcConfig::from_vars(vars(&[("OIDC_ISSUER", " ")])),
            Ok(None)
        );
        assert_eq!(
            OidcConfig::from_vars(vars(&[("OIDC_REQUIRED", "false")])),
            Ok(None)
        );
    }

    #[test]
    fn required_without_issuer_is_an_error() {
        for required in ["true", "TRUE", "1"] {
            assert_eq!(
                OidcConfig::from_vars(vars(&[("OIDC_REQUIRED", required)])),
                Err(OidcConfigError::Missing)
            );
        }
        assert_eq!(
            OidcConfig::from_vars(vars(&[("OIDC_REQUIRED", "yes please")])),
            Err(OidcConfigError::InvalidRequired("yes please".into()))
        );
    }

    #[test]
    fn issuer_defaults_audience_and_trims_trailing_slash() {
        let config = OidcConfig::from_vars(vars(&[
            ("OIDC_ISSUER", "http://localhost:8081/realms/econ-graph/"),
            ("OIDC_REQUIRED", "true"),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(config.issuer, "http://localhost:8081/realms/econ-graph");
        assert_eq!(config.audience, DEFAULT_AUDIENCE);
        assert_eq!(config.jwks_url, None);
        assert_eq!(
            config.discovery_url(),
            "http://localhost:8081/realms/econ-graph/.well-known/openid-configuration"
        );
    }

    #[test]
    fn jwks_url_and_audience_are_read() {
        let config = OidcConfig::from_vars(vars(&[
            ("OIDC_ISSUER", "http://localhost/idp/realms/econ-graph"),
            ("OIDC_AUDIENCE", "other-api"),
            (
                "OIDC_JWKS_URL",
                "http://keycloak-service:8080/idp/realms/econ-graph/protocol/openid-connect/certs",
            ),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(config.audience, "other-api");
        assert_eq!(
            config.jwks_url.as_deref(),
            Some(
                "http://keycloak-service:8080/idp/realms/econ-graph/protocol/openid-connect/certs"
            )
        );
    }

    #[test]
    fn malformed_settings_fail_instead_of_disabling_sign_in() {
        for bad in [
            "localhost:8081/realms/x",
            "ftp://idp.example/realms/x",
            "https://",
            "https://idp.example/realms/x?a=b",
            "not a url",
        ] {
            assert!(
                matches!(
                    OidcConfig::from_vars(vars(&[("OIDC_ISSUER", bad)])),
                    Err(OidcConfigError::InvalidUrl {
                        name: "OIDC_ISSUER",
                        ..
                    })
                ),
                "{bad}"
            );
        }
        assert_eq!(
            OidcConfig::from_vars(vars(&[
                ("OIDC_ISSUER", "https://idp.example/realms/x"),
                ("OIDC_AUDIENCE", "  ")
            ])),
            Err(OidcConfigError::EmptyAudience)
        );
        assert!(matches!(
            OidcConfig::from_vars(vars(&[
                ("OIDC_ISSUER", "https://idp.example/realms/x"),
                ("OIDC_JWKS_URL", "keycloak:8080/certs")
            ])),
            Err(OidcConfigError::InvalidUrl {
                name: "OIDC_JWKS_URL",
                ..
            })
        ));
    }
}

/// Token checks against a local issuer ([`crate::testkit::TestIssuer`]).
#[cfg(test)]
mod verify_tests {
    use super::*;
    use crate::roles::Role;
    use crate::testkit::{now, sign_hs256, SigningKey, TestIssuer};
    use serde_json::{json, Value};

    fn invalid(result: Result<VerifiedToken, VerifyError>) -> String {
        match result {
            Err(VerifyError::Invalid(why)) => why,
            other => panic!("expected an invalid token, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_valid_token_is_accepted() {
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        let sub = Uuid::new_v4();
        let token = issuer.mint_token(sub, &["annotation:create", "offline_access", "api:mcp"]);

        let verified = verifier.verify(&token).await.unwrap();
        assert_eq!(verified.subject, sub);
        assert_eq!(
            verified.email.as_deref(),
            Some(&*format!("{sub}@example.test"))
        );
        assert!(verified.email_verified);
        assert_eq!(verified.name.as_deref(), Some("Test User"));
        assert_eq!(
            verified.principal(),
            Principal::new(sub, [Role::AnnotationCreate, Role::ApiMcp])
        );
    }

    #[tokio::test]
    async fn aud_may_be_a_single_string() {
        let issuer = TestIssuer::start().await;
        let mut claims = issuer.claims(Uuid::new_v4(), &[]);
        claims["aud"] = json!(DEFAULT_AUDIENCE);
        assert!(issuer
            .verifier()
            .verify(&issuer.sign(&claims))
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn wrong_issuer_audience_or_times_are_rejected() {
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        let sub = Uuid::new_v4();
        type Edit = Box<dyn Fn(&mut Value)>;
        let edits: Vec<(&str, Edit)> = vec![
            (
                "other issuer",
                Box::new(|c| c["iss"] = json!("http://127.0.0.1:9/realms/other")),
            ),
            (
                "issuer with a trailing slash",
                Box::new(|c| {
                    let iss = format!("{}/", c["iss"].as_str().unwrap());
                    c["iss"] = json!(iss);
                }),
            ),
            (
                "other audience",
                Box::new(|c| c["aud"] = json!(["account", "econ-graph-web"])),
            ),
            (
                "no audience",
                Box::new(|c| {
                    c.as_object_mut().unwrap().remove("aud");
                }),
            ),
            (
                "expired",
                Box::new(|c| {
                    c["exp"] = json!(now() - 3600);
                    c["iat"] = json!(now() - 7200);
                }),
            ),
            (
                "no expiry",
                Box::new(|c| {
                    c.as_object_mut().unwrap().remove("exp");
                }),
            ),
            (
                "not yet valid",
                Box::new(|c| c["nbf"] = json!(now() + 3600)),
            ),
            (
                "no roles claim",
                Box::new(|c| {
                    c.as_object_mut().unwrap().remove("roles");
                }),
            ),
            (
                "roles only under resource_access",
                Box::new(|c| {
                    let roles = c.as_object_mut().unwrap().remove("roles").unwrap();
                    c["resource_access"] = json!({ DEFAULT_AUDIENCE: { "roles": roles } });
                }),
            ),
            (
                "subject not a UUID",
                Box::new(|c| c["sub"] = json!("alice")),
            ),
            (
                "no subject",
                Box::new(|c| {
                    c.as_object_mut().unwrap().remove("sub");
                }),
            ),
        ];
        for (case, edit) in edits {
            let mut claims = issuer.claims(sub, &["annotation:create"]);
            edit(&mut claims);
            let result = verifier.verify(&issuer.sign(&claims)).await;
            assert!(
                matches!(result, Err(VerifyError::Invalid(_))),
                "{case}: {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn only_rs256_signed_by_a_published_key_is_accepted() {
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        let claims = issuer.claims(Uuid::new_v4(), &["annotation:create"]);
        let kid = issuer.current_key().kid().to_string();

        // HS256, as the retired in-house login signed, even naming the published key id.
        let why = invalid(
            verifier
                .verify(&sign_hs256(&claims, Some(&kid), b"secret"))
                .await,
        );
        assert!(why.contains("HS256"), "{why}");

        // The same header and claims signed by a key the issuer never published.
        let forged = SigningKey::generate(&kid).sign(&claims);
        invalid(verifier.verify(&forged).await);

        // A key id the issuer does not publish.
        let unknown = SigningKey::generate("nobody-published-this").sign(&claims);
        let why = invalid(verifier.verify(&unknown).await);
        assert!(why.contains("unknown key id"), "{why}");

        // Tampered payload.
        let token = issuer.sign(&claims);
        let mut parts: Vec<String> = token.split('.').map(str::to_string).collect();
        let mut forged_claims = claims.clone();
        forged_claims["roles"] = json!(["admin.users:delete"]);
        parts[1] = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            serde_json::to_vec(&forged_claims).unwrap(),
        );
        invalid(verifier.verify(&parts.join(".")).await);

        // Unsigned ("alg": "none") and garbage.
        let none_header = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            br#"{"alg":"none","typ":"JWT"}"#,
        );
        invalid(
            verifier
                .verify(&format!("{none_header}.{}.", parts[1]))
                .await,
        );
        invalid(verifier.verify("not.a.jwt").await);
        invalid(verifier.verify("").await);

        // A token without a key id.
        let mut header = jsonwebtoken::Header::new(Algorithm::RS256);
        header.kid = None;
        let with_kid = issuer.sign(&claims);
        let (_, rest) = with_kid.split_once('.').unwrap();
        let no_kid_header = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            serde_json::to_vec(&header).unwrap(),
        );
        let why = invalid(verifier.verify(&format!("{no_kid_header}.{rest}")).await);
        assert!(why.contains("key id"), "{why}");
    }

    #[tokio::test]
    async fn keys_are_fetched_once_and_cached() {
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        assert_eq!(issuer.jwks_requests(), 0, "nothing is fetched up front");
        for _ in 0..3 {
            let token = issuer.mint_token(Uuid::new_v4(), &[]);
            verifier.verify(&token).await.unwrap();
        }
        assert_eq!(issuer.jwks_requests(), 1);
    }

    #[tokio::test]
    async fn a_rotated_key_is_picked_up() {
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        let sub = Uuid::new_v4();
        let old_token = issuer.mint_token(sub, &[]);
        verifier.verify(&old_token).await.unwrap();

        // Keycloak publishes the new key next to the old one while old tokens live on.
        issuer.rotate("test-key-2", true);
        let new_token = issuer.mint_token(sub, &[]);
        verifier.verify(&new_token).await.unwrap();
        verifier.verify(&old_token).await.unwrap();
        assert_eq!(issuer.jwks_requests(), 2);

        // Once the old key is withdrawn and the cache refreshed, its tokens are refused.
        issuer.rotate("test-key-3", false);
        verifier.verify(&issuer.mint_token(sub, &[])).await.unwrap();
        let why = invalid(verifier.verify(&old_token).await);
        assert!(why.contains("unknown key id"), "{why}");
    }

    #[tokio::test]
    async fn only_access_tokens_are_accepted() {
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        for typ in [json!("ID"), json!("Refresh"), json!("Logout"), Value::Null] {
            let mut claims = issuer.claims(Uuid::new_v4(), &[]);
            if typ.is_null() {
                claims.as_object_mut().unwrap().remove("typ");
            } else {
                claims["typ"] = typ.clone();
            }
            let why = invalid(verifier.verify(&issuer.sign(&claims)).await);
            assert!(why.contains("token type"), "{typ}: {why}");
        }
    }

    #[tokio::test]
    async fn a_stale_cache_keeps_answering_and_refreshes() {
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        let token = issuer.mint_token(Uuid::new_v4(), &[]);
        verifier.verify(&token).await.unwrap();

        verifier
            .age_cache(MAX_KEY_AGE + Duration::from_secs(1))
            .await;
        verifier.verify(&token).await.unwrap();
        assert_eq!(issuer.jwks_requests(), 2, "a stale cache is refreshed");

        // Provider gone: cached keys keep working until MAX_STALE_KEY_AGE, then nothing does.
        issuer.stop().await;
        verifier
            .age_cache(MAX_KEY_AGE + Duration::from_secs(1))
            .await;
        verifier.verify(&token).await.unwrap();
        verifier.age_cache(MAX_STALE_KEY_AGE).await;
        let result = verifier.verify(&token).await;
        assert!(
            matches!(result, Err(VerifyError::Unavailable(_))),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn a_new_key_the_provider_cannot_confirm_is_unavailable_not_invalid() {
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        verifier
            .verify(&issuer.mint_token(Uuid::new_v4(), &[]))
            .await
            .unwrap();
        issuer.rotate("test-key-2", true);
        let token = issuer.mint_token(Uuid::new_v4(), &[]);
        issuer.stop().await;
        let result = verifier.verify(&token).await;
        assert!(
            matches!(result, Err(VerifyError::Unavailable(_))),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn unknown_key_ids_do_not_refetch_more_than_the_interval_allows() {
        let issuer = TestIssuer::start().await;
        let verifier = OidcVerifier::with_client(
            issuer.config(),
            reqwest::Client::builder().no_proxy().build().unwrap(),
        );
        let claims = issuer.claims(Uuid::new_v4(), &[]);
        verifier.verify(&issuer.sign(&claims)).await.unwrap();
        for i in 0..5 {
            let token = SigningKey::generate(&format!("random-{i}")).sign(&claims);
            let result = verifier.verify(&token).await;
            // Not refetched, so not known to be invalid.
            assert!(
                matches!(result, Err(VerifyError::Unavailable(_))),
                "{result:?}"
            );
        }
        assert_eq!(
            issuer.jwks_requests(),
            1,
            "{MIN_REFRESH_INTERVAL:?} throttle"
        );
    }

    #[tokio::test]
    async fn a_key_rotated_while_refetches_are_throttled_is_not_called_invalid() {
        let issuer = TestIssuer::start().await;
        // Long enough to cover generating the new RSA key in a debug build.
        let interval = Duration::from_secs(3);
        let verifier = OidcVerifier::with_client(
            issuer.config(),
            reqwest::Client::builder().no_proxy().build().unwrap(),
        )
        .with_min_refresh_interval(interval);
        let claims = issuer.claims(Uuid::new_v4(), &[]);
        let random = SigningKey::generate("random").sign(&claims);
        verifier.verify(&issuer.sign(&claims)).await.unwrap();

        // Anyone can start the throttle window with a made-up kid.
        verifier.verify(&random).await.unwrap_err();
        issuer.rotate("test-key-2", true);
        let rotated = issuer.sign(&claims);
        let result = verifier.verify(&rotated).await;
        assert!(
            matches!(result, Err(VerifyError::Unavailable(_))),
            "{result:?}"
        );

        tokio::time::sleep(interval + Duration::from_millis(50)).await;
        verifier.verify(&rotated).await.unwrap();
        // Once refetched, a kid the provider doesn't publish is invalid.
        tokio::time::sleep(interval + Duration::from_millis(50)).await;
        invalid(verifier.verify(&random).await);
    }

    #[tokio::test]
    async fn an_unreachable_provider_is_unavailable_not_invalid() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &[]);
        let config = OidcConfig {
            issuer: "http://127.0.0.1:1/realms/econ-graph".into(),
            ..issuer.config()
        };
        let verifier = OidcVerifier::with_client(
            config,
            reqwest::Client::builder().no_proxy().build().unwrap(),
        );
        for _ in 0..2 {
            let result = verifier.verify(&token).await;
            assert!(
                matches!(result, Err(VerifyError::Unavailable(_))),
                "{result:?}"
            );
        }
    }

    #[tokio::test]
    async fn discovery_must_name_the_configured_issuer() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &[]);
        // Discovery is served at the real issuer; configure a different issuer string that
        // still resolves to it (the host spelled differently).
        let spoofed = issuer.issuer().replace("127.0.0.1", "localhost");
        let verifier = OidcVerifier::with_client(
            OidcConfig {
                issuer: spoofed,
                ..issuer.config()
            },
            reqwest::Client::builder().no_proxy().build().unwrap(),
        );
        let result = verifier.verify(&token).await;
        assert!(
            matches!(result, Err(VerifyError::Unavailable(ref why)) if why.contains("discovery names issuer")),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn an_explicit_jwks_url_skips_discovery() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &[]);
        let verifier = OidcVerifier::with_client(
            OidcConfig {
                jwks_url: Some(format!("{}/protocol/openid-connect/certs", issuer.issuer())),
                ..issuer.config()
            },
            reqwest::Client::builder().no_proxy().build().unwrap(),
        );
        verifier.verify(&token).await.unwrap();
    }

    #[test]
    fn encryption_and_non_rsa_keys_are_not_signing_keys() {
        let sig = SigningKey::generate("sig");
        let mut enc = SigningKey::generate("enc").jwk().clone();
        enc["use"] = json!("enc");
        enc["alg"] = json!("RSA-OAEP");
        let mut ps256 = SigningKey::generate("ps").jwk().clone();
        ps256["alg"] = json!("PS256");
        let mut no_kid = SigningKey::generate("x").jwk().clone();
        no_kid.as_object_mut().unwrap().remove("kid");
        let ec = json!({"kty": "EC", "kid": "ec", "crv": "P-256", "use": "sig",
            "x": "f83OJ3D2xF1Bg8vub9tLe1gHMzV76e8Tus9uPHvRVEU",
            "y": "x_FEzRu9m36HLN_tue659LNpXW6pCyStikYjKIWI5a0"});
        let jwks: JwkSet =
            serde_json::from_value(json!({ "keys": [sig.jwk(), enc, ps256, no_kid, ec] })).unwrap();
        let keys = signing_keys(&jwks);
        assert_eq!(keys.keys().collect::<Vec<_>>(), vec!["sig"]);
    }
}
