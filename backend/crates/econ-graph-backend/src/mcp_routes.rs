// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The `/mcp` route. Compiled in only when the `mcp` build flag is on
//! (`config/flags/flags.flagd.json`), which it is in the dev profile and not in release.

use econ_graph_auth::{authenticate, BearerError, OidcVerifier, Role};
use econ_graph_core::DatabasePool;
use econ_graph_mcp::mcp_server::{mcp_handler, EconGraphMcpServer};
use serde_json::json;
use std::sync::Arc;
use warp::{Filter, Reply as _};

use super::authorization_header;

/// Check that a request may use `/mcp`: an active, signed-in caller holding `api:mcp`.
async fn mcp_access(
    pool: &DatabasePool,
    verifier: Option<&OidcVerifier>,
    authorization: Option<&str>,
) -> Result<(), McpRejection> {
    match authenticate(pool, verifier, authorization).await {
        Ok(Some(caller)) if caller.principal.has_role(Role::ApiMcp) => Ok(()),
        Ok(Some(_)) => Err(McpRejection::MissingRole),
        Ok(None) => Err(McpRejection::Unauthorized),
        Err(BearerError::InvalidToken(why)) => {
            tracing::debug!("refused an MCP request: {why}");
            Err(McpRejection::InvalidToken)
        }
        Err(BearerError::Inactive(id)) => {
            tracing::info!("refused an MCP request from suspended account {id}");
            Err(McpRejection::Suspended)
        }
        Err(BearerError::Unavailable(why)) => {
            tracing::warn!("refused an MCP request whose token could not be checked: {why}");
            Err(McpRejection::AuthUnavailable)
        }
        Err(BearerError::Database(why)) => {
            tracing::error!("could not load the MCP caller's account: {why}");
            Err(McpRejection::AccountUnavailable)
        }
    }
}

/// JSON-RPC error for an MCP request without a valid bearer token.
fn mcp_unauthorized() -> warp::reply::Response {
    let reply = mcp_error(
        warp::http::StatusCode::UNAUTHORIZED,
        -32001,
        "Authentication required",
    );
    warp::reply::with_header(reply, "WWW-Authenticate", "Bearer").into_response()
}

/// Why `mcp_route` refused a request; its `recover` turns each into a JSON-RPC error reply.
#[derive(Debug, PartialEq, Eq)]
enum McpRejection {
    /// No bearer token (or sign-in is disabled): 401 with a `WWW-Authenticate: Bearer`
    /// challenge.
    Unauthorized,
    /// A bearer token that is not acceptable: 401, `Bearer error="invalid_token"`.
    InvalidToken,
    /// A valid token without `api:mcp`: 403, `Bearer error="insufficient_scope"`.
    MissingRole,
    /// The account is suspended: 403.
    Suspended,
    /// The token could not be checked because the identity provider is unreachable: 503.
    AuthUnavailable,
    /// The caller's account could not be loaded: 500.
    AccountUnavailable,
    /// More than [`MCP_BODY_LIMIT`] bytes of body: 413.
    BodyTooLarge,
    /// The body could not be read: 400.
    BodyUnreadable,
}

impl warp::reject::Reject for McpRejection {}

/// Largest MCP request body accepted, in bytes. JSON-RPC requests are small.
const MCP_BODY_LIMIT: usize = 1024 * 1024;

/// Collect a request body, giving up as soon as more than `limit` bytes have arrived.
///
/// This counts the bytes actually received rather than trusting `Content-Length`, which a
/// chunked request can carry alongside a much larger body.
async fn read_body_limited<S, B>(
    body: S,
    limit: usize,
) -> Result<warp::hyper::body::Bytes, warp::Rejection>
where
    S: tokio_stream::Stream<Item = Result<B, warp::Error>>,
    B: warp::hyper::body::Buf,
{
    use tokio_stream::StreamExt as _;
    use warp::hyper::body::Buf as _;

    let mut body = std::pin::pin!(body);
    let mut collected = Vec::new();
    while let Some(chunk) = body.next().await {
        let mut chunk = chunk.map_err(|_| warp::reject::custom(McpRejection::BodyUnreadable))?;
        if collected.len() + chunk.remaining() > limit {
            return Err(warp::reject::custom(McpRejection::BodyTooLarge));
        }
        while chunk.has_remaining() {
            let bytes = chunk.chunk();
            collected.extend_from_slice(bytes);
            let read = bytes.len();
            chunk.advance(read);
        }
    }
    Ok(collected.into())
}

/// JSON-RPC error reply with the given HTTP status, for requests `mcp_route` refuses.
fn mcp_error(status: warp::http::StatusCode, code: i32, message: &str) -> warp::reply::Response {
    let body = warp::reply::json(&json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": { "code": code, "message": message }
    }));
    warp::reply::with_status(body, status).into_response()
}

/// `POST /mcp`, open only to an active user whose bearer token grants `api:mcp`.
///
/// The token is checked before the body is read, so unauthenticated clients cannot make the
/// server buffer a body, and bodies over [`MCP_BODY_LIMIT`] are refused as they stream in.
fn mcp_route(
    pool: DatabasePool,
    verifier: Option<Arc<OidcVerifier>>,
    server: Arc<EconGraphMcpServer>,
) -> impl Filter<Extract = (warp::reply::Response,), Error = warp::Rejection> + Clone {
    let authenticate = warp::header::headers_cloned()
        .and_then(move |headers: warp::http::HeaderMap| {
            let pool = pool.clone();
            let verifier = verifier.clone();
            async move {
                // A header that is not UTF-8 counts as missing, so it still gets the 401.
                let authorization = authorization_header(&headers);
                mcp_access(&pool, verifier.as_deref(), authorization)
                    .await
                    .map_err(warp::reject::custom)
            }
        })
        .untuple_one();

    warp::path("mcp")
        .and(warp::post())
        .and(authenticate)
        .and(warp::body::stream())
        .and_then(move |body| {
            let server = server.clone();
            async move {
                let body = read_body_limited(body, MCP_BODY_LIMIT).await?;
                mcp_handler(body, server)
                    .await
                    .map(warp::Reply::into_response)
            }
        })
        .recover(|rejection: warp::Rejection| async move {
            use warp::http::StatusCode;
            match rejection.find::<McpRejection>() {
                Some(McpRejection::Unauthorized) => Ok(mcp_unauthorized()),
                Some(McpRejection::InvalidToken) => Ok(warp::reply::with_header(
                    mcp_error(
                        StatusCode::UNAUTHORIZED,
                        -32001,
                        "Invalid or expired access token",
                    ),
                    "WWW-Authenticate",
                    r#"Bearer error="invalid_token""#,
                )
                .into_response()),
                Some(McpRejection::MissingRole) => Ok(warp::reply::with_header(
                    mcp_error(
                        StatusCode::FORBIDDEN,
                        -32003,
                        "The api:mcp role is required",
                    ),
                    "WWW-Authenticate",
                    r#"Bearer error="insufficient_scope""#,
                )
                .into_response()),
                Some(McpRejection::Suspended) => Ok(mcp_error(
                    StatusCode::FORBIDDEN,
                    -32003,
                    "This account is suspended",
                )),
                Some(McpRejection::AuthUnavailable) => Ok(mcp_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    -32002,
                    "Sign-in is temporarily unavailable",
                )),
                Some(McpRejection::AccountUnavailable) => Ok(mcp_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    -32603,
                    "Could not load the signed-in account",
                )),
                Some(McpRejection::BodyTooLarge) => Ok(mcp_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    -32600,
                    "Request body too large",
                )),
                Some(McpRejection::BodyUnreadable) => Ok(mcp_error(
                    StatusCode::BAD_REQUEST,
                    -32700,
                    "Could not read request body",
                )),
                None => Err(rejection),
            }
        })
        .unify()
}

/// `/mcp`, boxed so it has the same type as the flagged-off stand-in in `main.rs`.
pub(crate) fn mcp_filter(
    pool: DatabasePool,
    verifier: Option<Arc<OidcVerifier>>,
) -> warp::filters::BoxedFilter<(warp::reply::Response,)> {
    let server = Arc::new(EconGraphMcpServer::new(Arc::new(pool.clone())));
    mcp_route(pool, verifier, server).boxed()
}

#[cfg(test)]
mod tests {
    use super::{
        mcp_access, mcp_filter, mcp_route, mcp_unauthorized, read_body_limited, McpRejection,
    };
    use econ_graph_auth::testkit::TestIssuer;
    use econ_graph_auth::{OidcConfig, OidcVerifier};
    use econ_graph_core::DatabasePool;
    use econ_graph_mcp::mcp_server::EconGraphMcpServer;
    use std::sync::Arc;
    use uuid::Uuid;
    use warp::http::StatusCode;

    /// A pool that never connects: every case using it is decided before any database access.
    fn unreachable_pool() -> DatabasePool {
        let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
            diesel_async::AsyncPgConnection,
        >::new("postgres://nobody@127.0.0.1:1/none");
        DatabasePool::builder()
            .connection_timeout(std::time::Duration::from_secs(1))
            .build_unchecked(manager)
    }

    /// A pool on `DATABASE_URL` with migrations applied, or `None` to skip the test.
    async fn database() -> Option<DatabasePool> {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            eprintln!("DATABASE_URL not set; skipping DB-backed MCP auth test");
            return None;
        };
        econ_graph_core::database::run_migrations(&url)
            .await
            .expect("migrations");
        Some(
            econ_graph_core::database::create_pool(&url)
                .await
                .expect("pool"),
        )
    }

    fn bearer(token: &str) -> String {
        format!("Bearer {token}")
    }

    /// A verifier for an issuer nobody listens on: its keys can never be fetched.
    fn unreachable_verifier() -> OidcVerifier {
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        OidcVerifier::with_client(
            OidcConfig {
                issuer: "http://127.0.0.1:1/realms/econ-graph".into(),
                audience: "econ-graph-api".into(),
                jwks_url: None,
            },
            client,
        )
    }

    async fn set_active(pool: &DatabasePool, id: Uuid, active: bool) {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;
        let mut conn = pool.get().await.unwrap();
        diesel::update(users::table.find(id))
            .set(users::is_active.eq(active))
            .execute(&mut conn)
            .await
            .unwrap();
    }

    /// The rejection is an HTTP 401 carrying a `WWW-Authenticate: Bearer` challenge.
    #[test]
    fn mcp_unauthorized_is_a_401_with_a_bearer_challenge() {
        let response = mcp_unauthorized();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()["WWW-Authenticate"], "Bearer");
    }

    /// POSTs to the mounted `/mcp` route without an acceptable token all get a 401 challenge,
    /// even with a body over the size limit: no token or any token while sign-in is disabled
    /// (plain `Bearer`), and a bad or non-UTF-8 token (`invalid_token`). An identity provider
    /// outage is a 503.
    #[tokio::test]
    async fn mcp_route_refuses_unauthenticated_requests() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["api:mcp"]);
        let pool = unreachable_pool();
        let server = Arc::new(EconGraphMcpServer::new(Arc::new(pool.clone())));
        let too_big = vec![b' '; super::MCP_BODY_LIMIT + 1];
        let invalid = r#"Bearer error="invalid_token""#;
        let header = |v: &str| Some(warp::http::HeaderValue::from_str(v).unwrap());
        for (verifier, authorization, status, challenge) in [
            (
                Some(issuer.verifier()),
                None,
                StatusCode::UNAUTHORIZED,
                Some("Bearer"),
            ),
            (
                Some(issuer.verifier()),
                header("Bearer not.a.jwt"),
                StatusCode::UNAUTHORIZED,
                Some(invalid),
            ),
            (
                Some(issuer.verifier()),
                Some(warp::http::HeaderValue::from_bytes(b"Bearer \xff\xfe").unwrap()),
                StatusCode::UNAUTHORIZED,
                Some("Bearer"),
            ),
            (
                None,
                header(&bearer(&token)),
                StatusCode::UNAUTHORIZED,
                Some("Bearer"),
            ),
            (
                Some(unreachable_verifier()),
                header(&bearer(&token)),
                StatusCode::SERVICE_UNAVAILABLE,
                None,
            ),
        ] {
            let route = mcp_route(pool.clone(), verifier.map(Arc::new), server.clone());
            let mut request = warp::test::request()
                .method("POST")
                .path("/mcp")
                .body(too_big.clone());
            if let Some(value) = authorization.clone() {
                request = request.header("authorization", value);
            }
            let response = request.reply(&route).await;
            assert_eq!(response.status(), status, "{authorization:?}");
            assert_eq!(
                response
                    .headers()
                    .get("WWW-Authenticate")
                    .map(|v| v.to_str().unwrap()),
                challenge,
                "{authorization:?}"
            );
        }
    }

    /// `/mcp` needs an active account whose token grants `api:mcp`.
    // Shares the database with `integration_tests`, which drop the schema.
    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_needs_the_api_mcp_role_and_an_active_account() {
        let Some(pool) = database().await else {
            return;
        };
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();

        let sub = Uuid::new_v4();
        let with_role = bearer(&issuer.mint_token(sub, &["api:mcp"]));
        let without_role = bearer(&issuer.mint_token(sub, &["annotation:create"]));
        let access = |verifier, header: &str| {
            let pool = pool.clone();
            let header = header.to_string();
            async move { mcp_access(&pool, verifier, Some(&header)).await }
        };
        assert_eq!(access(Some(&verifier), &with_role).await, Ok(()));
        assert_eq!(
            access(Some(&verifier), &without_role).await,
            Err(McpRejection::MissingRole)
        );
        assert_eq!(
            access(None, &with_role).await,
            Err(McpRejection::Unauthorized)
        );

        set_active(&pool, sub, false).await;
        assert_eq!(
            access(Some(&verifier), &with_role).await,
            Err(McpRejection::Suspended)
        );
    }

    /// Chunks that together exceed the limit are refused, however they are split; this is
    /// what caps chunked bodies whose `Content-Length` understates their size.
    #[tokio::test]
    async fn read_body_limited_counts_received_bytes() {
        use warp::hyper::body::Bytes;
        let chunks = |sizes: &[usize]| {
            tokio_stream::iter(
                sizes
                    .iter()
                    .map(|&n| Ok::<_, warp::Error>(Bytes::from(vec![b'x'; n])))
                    .collect::<Vec<_>>(),
            )
        };

        let body = read_body_limited(chunks(&[4, 6]), 10).await.unwrap();
        assert_eq!(body.len(), 10);

        let rejection = read_body_limited(chunks(&[4, 4, 4]), 10).await.unwrap_err();
        assert!(matches!(
            rejection.find::<McpRejection>(),
            Some(McpRejection::BodyTooLarge)
        ));
    }

    /// With the `mcp` flag off nothing is routed at `/mcp`, so a request there gets a 404.
    /// With it on (this crate is only compiled with the flag), the mounted filter behaves
    /// like `mcp_route`: an unauthenticated POST gets the 401 challenge.
    #[tokio::test]
    async fn mcp_filter_keeps_the_authenticated_route() {
        let response = warp::test::request()
            .method("POST")
            .path("/mcp")
            .body("{}")
            .reply(&mcp_filter(unreachable_pool(), None))
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()["WWW-Authenticate"], "Bearer");
    }
}
