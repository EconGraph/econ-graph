//! # Test identity provider
//!
//! A stand-in for Keycloak in tests: [`TestIssuer`] serves OpenID Connect discovery and a JWKS
//! on a local port, and mints RS256 access tokens shaped like the `econ-graph` realm's (a
//! top-level `roles` claim, `aud` naming `econ-graph-api` and Keycloak's `account`). Point an
//! [`OidcVerifier`] at it with [`TestIssuer::verifier`].
//!
//! Enabled by the `testkit` feature; add `econ-graph-auth = { ..., features = ["testkit"] }`
//! to a crate's dev-dependencies.

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use aws_lc_rs::encoding::AsDer as _;
use aws_lc_rs::rsa::{KeyPair, KeySize};
use aws_lc_rs::signature::KeyPair as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::{json, Value};
use uuid::Uuid;
use warp::Filter as _;

use crate::oidc::{OidcConfig, OidcVerifier, DEFAULT_AUDIENCE};

/// The realm path the test issuer serves, like Keycloak's `/realms/<realm>`.
const REALM_PATH: &str = "realms/test";

/// An RSA signing key with its key id and public JWK.
pub struct SigningKey {
    kid: String,
    encoding: EncodingKey,
    jwk: Value,
}

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningKey")
            .field("kid", &self.kid)
            .finish()
    }
}

impl SigningKey {
    /// A fresh 2048-bit RSA key with key id `kid`.
    pub fn generate(kid: &str) -> Self {
        let pair = KeyPair::generate(KeySize::Rsa2048).expect("RSA key generation");
        let pkcs8 = pair.as_der().expect("PKCS#8 encoding");
        let body = STANDARD.encode(pkcs8.as_ref());
        let lines: Vec<&str> = body
            .as_bytes()
            .chunks(64)
            .map(|line| std::str::from_utf8(line).expect("base64 is ASCII"))
            .collect();
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            lines.join("\n")
        );
        let encoding = EncodingKey::from_rsa_pem(pem.as_bytes()).expect("RSA encoding key");

        let public = pair.public_key();
        let jwk = json!({
            "kid": kid,
            "kty": "RSA",
            "alg": "RS256",
            "use": "sig",
            "n": URL_SAFE_NO_PAD.encode(public.modulus().big_endian_without_leading_zero()),
            "e": URL_SAFE_NO_PAD.encode(public.exponent().big_endian_without_leading_zero()),
        });
        Self {
            kid: kid.to_string(),
            encoding,
            jwk,
        }
    }

    /// The key id tokens signed with this key carry.
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// The public key as a JWK, as the JWKS publishes it.
    pub fn jwk(&self) -> &Value {
        &self.jwk
    }

    /// Sign `claims` with RS256, naming this key in the header.
    pub fn sign(&self, claims: &Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.kid.clone());
        jsonwebtoken::encode(&header, claims, &self.encoding).expect("signing a token")
    }
}

/// Sign `claims` with HS256 and `secret`, as the retired in-house login did. Tests use it to
/// check such tokens are refused.
pub fn sign_hs256(claims: &Value, kid: Option<&str>, secret: &[u8]) -> String {
    let mut header = Header::new(Algorithm::HS256);
    header.kid = kid.map(str::to_string);
    jsonwebtoken::encode(&header, claims, &EncodingKey::from_secret(secret))
        .expect("signing a token")
}

/// Seconds since the Unix epoch.
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

struct State {
    current: Arc<SigningKey>,
    published: Vec<Value>,
    jwks_requests: usize,
}

/// A local OpenID Connect provider serving discovery and a JWKS, with a current signing key.
pub struct TestIssuer {
    issuer: String,
    state: Arc<RwLock<State>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server: Option<tokio::task::JoinHandle<()>>,
}

impl TestIssuer {
    /// Start serving on a free local port, publishing one signing key.
    pub async fn start() -> Self {
        let key = Arc::new(SigningKey::generate("test-key-1"));
        let state = Arc::new(RwLock::new(State {
            published: vec![key.jwk().clone()],
            current: key,
            jwks_requests: 0,
        }));

        // The issuer URL needs the port, which is only known once bound; the discovery
        // handler reads it from here.
        let issuer_cell: Arc<std::sync::OnceLock<String>> = Arc::default();

        let discovery = {
            let issuer_cell = issuer_cell.clone();
            warp::path!("realms" / "test" / ".well-known" / "openid-configuration").map(move || {
                let issuer = issuer_cell.get().expect("issuer set after bind");
                warp::reply::json(&json!({
                    "issuer": issuer,
                    "jwks_uri": format!("{issuer}/protocol/openid-connect/certs"),
                    "id_token_signing_alg_values_supported": ["RS256"],
                }))
            })
        };
        let certs = {
            let state = state.clone();
            warp::path!("realms" / "test" / "protocol" / "openid-connect" / "certs").map(
                move || {
                    let mut state = state.write().expect("test issuer state");
                    state.jwks_requests += 1;
                    warp::reply::json(&json!({ "keys": state.published }))
                },
            )
        };

        let (tx, rx) = tokio::sync::oneshot::channel();
        let (addr, server): (SocketAddr, _) = warp::serve(warp::get().and(discovery.or(certs)))
            .bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async {
                let _ = rx.await;
            });
        let server = tokio::spawn(server);

        let issuer = format!("http://{addr}/{REALM_PATH}");
        issuer_cell.set(issuer.clone()).expect("set once");
        Self {
            issuer,
            state,
            shutdown: Some(tx),
            server: Some(server),
        }
    }

    /// The issuer URL, as tokens name it in `iss`.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// Settings for a verifier that trusts this issuer, found through discovery.
    pub fn config(&self) -> OidcConfig {
        OidcConfig {
            issuer: self.issuer.clone(),
            audience: DEFAULT_AUDIENCE.to_string(),
            jwks_url: None,
        }
    }

    /// A verifier for this issuer that ignores proxy settings (the issuer is on localhost) and
    /// refetches keys on every unknown key id.
    pub fn verifier(&self) -> OidcVerifier {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("HTTP client");
        OidcVerifier::with_client(self.config(), client).with_min_refresh_interval(Duration::ZERO)
    }

    /// Claims of a valid access token for `sub` holding `roles`, valid for five minutes:
    /// edit them to build an invalid one.
    pub fn claims(&self, sub: Uuid, roles: &[&str]) -> Value {
        let now = now();
        json!({
            "iss": self.issuer,
            "aud": [DEFAULT_AUDIENCE, "account"],
            "sub": sub.to_string(),
            "iat": now,
            "nbf": now,
            "exp": now + 300,
            "typ": "Bearer",
            "azp": "econ-graph-web",
            "roles": roles,
            "email": format!("{sub}@example.test"),
            "email_verified": true,
            "name": "Test User",
            "preferred_username": "test-user",
        })
    }

    /// Sign `claims` with the current key.
    pub fn sign(&self, claims: &Value) -> String {
        self.current_key().sign(claims)
    }

    /// A valid access token for `sub` holding `roles`.
    pub fn mint_token(&self, sub: Uuid, roles: &[&str]) -> String {
        self.sign(&self.claims(sub, roles))
    }

    /// The key new tokens are signed with.
    pub fn current_key(&self) -> Arc<SigningKey> {
        self.state
            .read()
            .expect("test issuer state")
            .current
            .clone()
    }

    /// Rotate to a new signing key with id `kid`. With `keep_old`, the JWKS keeps publishing
    /// the previous keys (as Keycloak does while old tokens are still valid); without it, only
    /// the new key is published.
    pub fn rotate(&self, kid: &str, keep_old: bool) -> Arc<SigningKey> {
        let key = Arc::new(SigningKey::generate(kid));
        let mut state = self.state.write().expect("test issuer state");
        if !keep_old {
            state.published.clear();
        }
        state.published.push(key.jwk().clone());
        state.current = key.clone();
        key
    }

    /// How many times the JWKS has been fetched.
    pub fn jwks_requests(&self) -> usize {
        self.state.read().expect("test issuer state").jwks_requests
    }
}

impl TestIssuer {
    /// Stop serving and wait until the port is closed, so later fetches fail.
    pub async fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(server) = self.server.take() {
            let _ = server.await;
        }
    }
}

impl Drop for TestIssuer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}
