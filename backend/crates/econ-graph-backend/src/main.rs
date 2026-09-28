// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::GraphQLResponse;
use hyper::service::Service as _;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, Instrument};
use warp::{Filter, Reply as _};

// Import from our new crates
use econ_graph_auth::auth::{routes::auth_routes, services::AuthService};
use econ_graph_auth::{authenticate, BearerError, Caller, OidcConfig, OidcVerifier, Role};
use econ_graph_core::{
    create_pool, redact_database_url, AppError, AppResult, Config, DatabasePool,
};
use econ_graph_graphql::graphql::schema::create_schema_with_data;
use econ_graph_mcp::mcp_server::{mcp_handler, EconGraphMcpServer};

mod integration_tests;
mod metrics;

#[derive(Clone)]
pub struct AppState {
    pub pool: DatabasePool,
    pub schema: async_graphql::Schema<
        econ_graph_graphql::graphql::query::Query,
        econ_graph_graphql::graphql::mutation::Mutation,
        async_graphql::EmptySubscription,
    >,
}

async fn graphql_handler(
    schema: async_graphql::Schema<
        econ_graph_graphql::graphql::query::Query,
        econ_graph_graphql::graphql::mutation::Mutation,
        async_graphql::EmptySubscription,
    >,
    request: async_graphql::Request,
) -> Result<GraphQLResponse, Infallible> {
    let start_time = std::time::Instant::now();

    // Extract operation info for metrics before consuming request
    let operation_name_clone = request.operation_name.clone();
    let operation_type = operation_name_clone.as_deref().unwrap_or("unknown");
    let operation_name = operation_name_clone.as_deref().unwrap_or("anonymous");

    let response = schema.execute(request).await;
    let duration = start_time.elapsed().as_secs_f64();

    // Record GraphQL metrics
    metrics::record_graphql_query(
        operation_type,
        operation_name,
        duration,
        1.0, // Basic complexity for now
    );

    Ok(GraphQLResponse::from(response))
}

/// Whether `/playground` is served: only when `ENABLE_GRAPHQL_PLAYGROUND` is `true` or `1`.
///
/// Deployed builds leave it unset, so the playground (and its schema explorer) is not public.
fn playground_enabled(setting: Option<String>) -> bool {
    setting
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| s == "1" || s.eq_ignore_ascii_case("true"))
}

async fn graphql_playground() -> Result<impl warp::Reply, Infallible> {
    Ok(warp::reply::html(playground_source(
        GraphQLPlaygroundConfig::new("/graphql"),
    )))
}

async fn health_check() -> Result<impl warp::Reply, Infallible> {
    // Record health check metrics
    metrics::record_http_request("GET", "/health", 200, 0.0);

    Ok(warp::reply::json(&json!({
        "status": "healthy",
        "service": "econ-graph-backend",
        "version": env!("CARGO_PKG_VERSION")
    })))
}

/// Landing-page entry for `/playground`, shown only when the playground is served.
const PLAYGROUND_ENDPOINT_HTML: &str = r#"        <div class="endpoint">
            <div><span class="method">GET</span> <code>/playground</code></div>
            <p><a href="/playground">Interactive GraphQL Playground</a> - Test queries and explore the schema</p>
        </div>"#;

/// Landing-page quick start pointing at the playground, shown only when it is served.
const PLAYGROUND_QUICK_START_HTML: &str = r#"        <p>Visit the <a href="/playground">GraphQL Playground</a> to start exploring economic data!</p>"#;

/// The landing page at `/`. It lists `/playground` only when the playground is served.
async fn root_handler(playground: bool) -> Result<impl warp::Reply, Infallible> {
    // Record root endpoint metrics
    metrics::record_http_request("GET", "/", 200, 0.0);

    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>EconGraph API</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }
        .container { max-width: 800px; margin: 0 auto; background: white; padding: 40px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
        h1 { color: #2c3e50; border-bottom: 3px solid #3498db; padding-bottom: 10px; }
        .endpoint { background: #ecf0f1; padding: 15px; margin: 10px 0; border-radius: 5px; border-left: 4px solid #3498db; }
        .method { font-weight: bold; color: #27ae60; }
        a { color: #3498db; text-decoration: none; }
        a:hover { text-decoration: underline; }
        .status { color: #27ae60; font-weight: bold; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🏢 EconGraph API Server</h1>
        <p class="status">✅ Server is running and healthy!</p>

        <h2>📊 Available Endpoints</h2>

        <div class="endpoint">
            <div><span class="method">POST/GET</span> <code>/graphql</code></div>
            <p>GraphQL endpoint for economic data queries and mutations</p>
        </div>

<!--PLAYGROUND_ENDPOINT-->

        <div class="endpoint">
            <div><span class="method">GET</span> <code>/health</code></div>
            <p><a href="/health">Health check endpoint</a> - API status and version info</p>
        </div>

        <div class="endpoint">
            <div><span class="method">GET</span> <code>/metrics</code></div>
            <p><a href="/metrics">Prometheus metrics endpoint</a> - Application metrics for monitoring</p>
        </div>

        <div class="endpoint">
            <div><span class="method">POST</span> <code>/mcp</code></div>
            <p>MCP (Model Context Protocol) server endpoint - AI model integration for economic data access</p>
        </div>

        <h2>🚀 Quick Start</h2>
<!--PLAYGROUND_QUICK_START-->

        <h2>📈 Features</h2>
        <ul>
            <li><strong>Economic Data API</strong> - Access to FRED, BLS, and other economic data sources</li>
            <li><strong>Full-Text Search</strong> - Intelligent search with spelling correction and synonyms</li>
            <li><strong>Real-Time Collaboration</strong> - Chart annotations, comments, and sharing</li>
            <li><strong>Professional Analytics</strong> - Bloomberg Terminal-level functionality</li>
            <li><strong>Data Transformations</strong> - Growth rates, differences, logarithmic scaling</li>
        </ul>

        <p><em>Version: {}</em></p>
    </div>
</body>
</html>
    "#;

    let (endpoint, quick_start) = if playground {
        (PLAYGROUND_ENDPOINT_HTML, PLAYGROUND_QUICK_START_HTML)
    } else {
        ("", "<p>Send GraphQL queries to <code>/graphql</code>.</p>")
    };
    Ok(warp::reply::html(
        html.replace("{}", env!("CARGO_PKG_VERSION"))
            .replace("<!--PLAYGROUND_ENDPOINT-->", endpoint)
            .replace("<!--PLAYGROUND_QUICK_START-->", quick_start),
    ))
}

/// Check each configured CORS origin is a bare `http(s)://host[:port]` origin.
///
/// warp panics on an origin it cannot parse, and a wildcard would re-open the API to every
/// site, so reject both at startup with a clear message instead.
fn validate_cors_origins(origins: &[String]) -> AppResult<Vec<String>> {
    let origins: Vec<String> = origins
        .iter()
        .map(|o| o.trim().trim_end_matches('/').to_string())
        .filter(|o| !o.is_empty())
        .collect();
    if origins.is_empty() {
        return Err(AppError::ConfigError(
            "CORS_ALLOWED_ORIGINS must list at least one origin".to_string(),
        ));
    }
    for origin in &origins {
        let valid = origin
            .parse::<warp::http::Uri>()
            .ok()
            .filter(|uri| {
                matches!(uri.scheme_str(), Some("http") | Some("https"))
                    && uri.host().is_some_and(|h| !h.is_empty() && h != "*")
                    && uri.path_and_query().map_or(true, |pq| pq.as_str() == "/")
            })
            .is_some();
        if !valid {
            return Err(AppError::ConfigError(format!(
                "Invalid CORS origin {:?}: expected scheme://host[:port], like https://econ-graph.com",
                origin
            )));
        }
    }
    Ok(origins)
}

/// Build the CORS filter that lets browsers call the API only from `origins`.
///
/// `origins` must already have passed [`validate_cors_origins`].
fn cors_filter(origins: &[String]) -> warp::cors::Builder {
    warp::cors()
        .allow_origins(origins.iter().map(String::as_str))
        .allow_headers(vec!["content-type", "authorization"])
        .allow_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
}

/// The `Authorization` header as text; a value that is not UTF-8 counts as missing.
fn authorization_header(headers: &warp::http::HeaderMap) -> Option<&str> {
    headers
        .get(warp::http::header::AUTHORIZATION)
        .and_then(|value| std::str::from_utf8(value.as_bytes()).ok())
}

/// Why a GraphQL request that sent a bearer token was refused before execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphqlRefusal {
    /// The token is not acceptable: 401, so the client signs in again.
    InvalidToken,
    /// The account is suspended: 403.
    Suspended,
    /// The caller's account could not be loaded: 500.
    AccountUnavailable,
}

impl GraphqlRefusal {
    /// A GraphQL-shaped error reply with the matching HTTP status. A 401 carries the
    /// `WWW-Authenticate` challenge of RFC 6750.
    fn into_response(self) -> warp::reply::Response {
        use warp::http::StatusCode;
        let (status, code, message) = match self {
            Self::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Invalid or expired access token",
            ),
            Self::Suspended => (
                StatusCode::FORBIDDEN,
                "ACCOUNT_SUSPENDED",
                "This account is suspended",
            ),
            Self::AccountUnavailable => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_SERVER_ERROR",
                "Could not load the signed-in account",
            ),
        };
        let body = warp::reply::json(&json!({
            "data": null,
            "errors": [{ "message": message, "extensions": { "code": code } }]
        }));
        let reply = warp::reply::with_status(body, status).into_response();
        if self == Self::InvalidToken {
            warp::reply::with_header(reply, "WWW-Authenticate", r#"Bearer error="invalid_token""#)
                .into_response()
        } else {
            reply
        }
    }
}

/// The signed-in caller of a GraphQL request, `None` for an anonymous one, or why the request
/// is refused.
///
/// A token the identity provider's keys could not check (the provider is down and its keys
/// were never fetched) is dropped and the request runs anonymously: it grants nothing, and
/// public data stays browsable through an outage.
async fn graphql_caller(
    pool: &DatabasePool,
    verifier: Option<&OidcVerifier>,
    authorization: Option<&str>,
) -> Result<Option<Caller>, GraphqlRefusal> {
    match authenticate(pool, verifier, authorization).await {
        Ok(caller) => Ok(caller),
        Err(BearerError::Unavailable(why)) => {
            tracing::warn!("serving a request with an unverifiable token anonymously: {why}");
            Ok(None)
        }
        Err(BearerError::InvalidToken(why)) => {
            tracing::debug!("rejected bearer token: {why}");
            Err(GraphqlRefusal::InvalidToken)
        }
        Err(BearerError::Inactive(id)) => {
            tracing::info!("refused a request from suspended account {id}");
            Err(GraphqlRefusal::Suspended)
        }
        Err(BearerError::Database(why)) => {
            tracing::error!("could not load the caller's account: {why}");
            Err(GraphqlRefusal::AccountUnavailable)
        }
    }
}

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

#[tokio::main]
async fn main() -> AppResult<()> {
    // Initialize tracing with more detailed output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();

    info!(
        "🚀 Starting EconGraph Backend Server v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Log environment variables (non-sensitive ones)
    info!("🔧 Environment Configuration:");
    info!(
        "  - RUST_LOG: {:?}",
        std::env::var("RUST_LOG").unwrap_or_else(|_| "not set".to_string())
    );
    info!(
        "  - BACKEND_PORT: {:?}",
        std::env::var("BACKEND_PORT").unwrap_or_else(|_| "not set".to_string())
    );
    info!(
        "  - FRONTEND_PORT: {:?}",
        std::env::var("FRONTEND_PORT").unwrap_or_else(|_| "not set".to_string())
    );
    info!(
        "  - DATABASE_URL: {:?}",
        if std::env::var("DATABASE_URL").is_ok() {
            "set"
        } else {
            "not set"
        }
    );
    info!(
        "  - JWT_SECRET: {:?}",
        if std::env::var("JWT_SECRET").is_ok() {
            "set"
        } else {
            "not set"
        }
    );

    // Load configuration
    info!("📋 Loading configuration from environment...");
    let config = Config::from_env().map_err(|e| {
        let error = AppError::ConfigError(format!("Failed to load configuration: {}", e));
        error.log_with_context("Application startup configuration loading");
        eprintln!("❌ Failed to load configuration: {}", e);
        error
    })?;

    // Refuse to start without a JWT signing secret rather than sign tokens with a known key.
    econ_graph_auth::auth::services::jwt_secret().map_err(|e| {
        e.log_with_context("Application startup JWT secret check");
        eprintln!("❌ {}", e);
        e
    })?;

    // Validate CORS origins (CORS_ALLOWED_ORIGINS, comma-separated; defaults to the local
    // frontend) before touching the database, so bad config fails fast without side effects.
    let cors_origins = validate_cors_origins(&config.cors.allowed_origins).map_err(|e| {
        e.log_with_context("Application startup CORS configuration");
        eprintln!("❌ {}", e);
        e
    })?;

    // Sign-in through the identity provider: OIDC_ISSUER unset disables it (every caller is
    // anonymous); a malformed setting, or OIDC_REQUIRED=true without an issuer, stops startup.
    // Nothing is fetched from the provider here, so it may be down while the API starts.
    let oidc = OidcConfig::from_env().map_err(|e| {
        let error = AppError::ConfigError(e.to_string());
        error.log_with_context("Application startup OIDC configuration");
        eprintln!("❌ {}", e);
        error
    })?;
    match &oidc {
        Some(oidc) => info!(
            "🔐 Sign-in: tokens from {} for audience {}",
            oidc.issuer, oidc.audience
        ),
        None => info!("🔓 Sign-in disabled (OIDC_ISSUER unset): every caller is anonymous"),
    }
    let verifier = oidc.map(|config| Arc::new(OidcVerifier::new(config)));

    info!("📊 Configuration loaded successfully:");
    info!("  - Server host: {}", config.server.host);
    info!("  - Server port: {}", config.server.port);
    info!("  - CORS origins: {:?}", cors_origins);
    info!(
        "  - Database URL: {}",
        redact_database_url(&config.database_url)
    );

    // Create database connection pool
    info!("🗄️  Creating database connection pool...");
    info!(
        "  - Database URL: {}",
        redact_database_url(&config.database_url)
    );

    let pool = create_pool(&config.database_url).await.map_err(|e| {
        let error = AppError::DatabaseError(format!("Failed to create database pool: {}", e));
        error.log_with_context("Application startup database pool creation");
        eprintln!("❌ Failed to create database pool: {}", e);
        error
    })?;

    info!("✅ Database connection pool created successfully");

    // Run migrations
    info!("🔄 Running database migrations...");
    econ_graph_core::run_migrations(&config.database_url)
        .await
        .map_err(|e| {
            let error = AppError::DatabaseError(format!("Failed to run migrations: {}", e));
            error.log_with_context("Application startup database migrations");
            eprintln!("❌ Failed to run migrations: {}", e);
            error
        })?;

    info!("✅ Database migrations completed successfully");

    // Create GraphQL schema
    let schema = create_schema_with_data(pool.clone(), ());
    info!("🎯 GraphQL schema created");

    // Create authentication service
    let auth_service = AuthService::new(pool.clone());
    info!("🔐 Authentication service created");

    // Initialize metrics
    info!("📊 Initializing Prometheus metrics...");
    let _metrics = &metrics::METRICS; // Initialize metrics
    info!("✅ Prometheus metrics initialized");

    // Start uptime counter task
    let uptime_task = tokio::spawn(async {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            metrics::increment_uptime(60);
        }
    });

    // Crawling does not run in this process: the API only enqueues crawl_queue jobs, and the
    // separate `crawler-worker` binary (econ-graph-crawler) processes them.

    // Create Warp filters. Browsers may call the API only from the configured frontend origins.
    let cors = cors_filter(&cors_origins);

    // GraphQL endpoint: anonymous without a token, the token's caller with a valid one.
    let pool_for_graphql = pool.clone();
    let verifier_for_graphql = verifier.clone();
    let graphql_filter = warp::path("graphql")
        .and(warp::header::headers_cloned())
        .and(async_graphql_warp::graphql(schema.clone()))
        .and_then(
            move |headers: warp::http::HeaderMap<warp::http::HeaderValue>,
                  (_schema, request): (
                async_graphql::Schema<
                    econ_graph_graphql::graphql::query::Query,
                    econ_graph_graphql::graphql::mutation::Mutation,
                    async_graphql::EmptySubscription,
                >,
                async_graphql::Request,
            )| {
                let pool_for_graphql = pool_for_graphql.clone();
                let verifier = verifier_for_graphql.clone();
                async move {
                    let caller = match graphql_caller(
                        &pool_for_graphql,
                        verifier.as_deref(),
                        authorization_header(&headers),
                    )
                    .await
                    {
                        Ok(caller) => caller,
                        Err(refusal) => return Ok::<_, Infallible>(refusal.into_response()),
                    };

                    let auth_context = std::sync::Arc::new(
                        econ_graph_graphql::graphql::context::GraphQLContext::new(caller),
                    );
                    let auth_schema = econ_graph_graphql::graphql::schema::create_schema_with_data(
                        pool_for_graphql.clone(),
                        auth_context,
                    );

                    Ok(GraphQLResponse::from(auth_schema.execute(request).await).into_response())
                }
            },
        );

    // GraphQL Playground: off unless ENABLE_GRAPHQL_PLAYGROUND=true (local development).
    let playground_enabled = playground_enabled(std::env::var("ENABLE_GRAPHQL_PLAYGROUND").ok());
    let playground_filter =
        warp::path("playground")
            .and(warp::get())
            .and_then(move || async move {
                if playground_enabled {
                    graphql_playground().await.map_err(|never| match never {})
                } else {
                    Err(warp::reject::not_found())
                }
            });

    // Health check
    let health_filter = warp::path("health").and(warp::get()).and_then(health_check);

    // Metrics endpoint for Prometheus
    let metrics_filter = warp::path("metrics")
        .and(warp::get())
        .and_then(metrics::metrics_handler);

    // Root endpoint
    let root_filter = warp::path::end()
        .and(warp::get())
        .and_then(move || root_handler(playground_enabled));

    // Authentication routes
    let auth_filter = auth_routes(auth_service);

    // MCP Server routes
    let mcp_server = Arc::new(EconGraphMcpServer::new(Arc::new(pool.clone())));

    // MCP requires a signed-in user's bearer token holding `api:mcp`.
    // MCP OAuth (auth roadmap phase 6) will replace this.
    let mcp_filter = mcp_route(pool.clone(), verifier.clone(), mcp_server.clone());

    // Combine all routes
    let routes = root_filter
        .or(graphql_filter)
        .or(playground_filter)
        .or(health_filter)
        .or(metrics_filter)
        .or(auth_filter)
        .or(mcp_filter)
        .with(cors)
        .with(warp::trace::request());

    // Initialize metrics
    info!("📊 Initializing Prometheus metrics...");
    let _metrics = &metrics::METRICS; // Initialize metrics
    info!("✅ Prometheus metrics initialized");

    let port = config.server.port;
    info!("🌐 Server starting on http://0.0.0.0:{}", port);
    if playground_enabled {
        info!(
            "🎮 GraphQL Playground available at http://localhost:{}/playground",
            port
        );
    }
    info!(
        "❤️  Health check available at http://localhost:{}/health",
        port
    );
    info!(
        "📊 Prometheus metrics available at http://localhost:{}/metrics",
        port
    );
    info!("🔗 API endpoints:");
    info!("  - POST/GET /graphql - GraphQL API");
    if playground_enabled {
        info!("  - GET /playground - GraphQL Playground");
    }
    info!("  - GET /health - Health check");
    info!("  - GET /metrics - Prometheus metrics");
    info!("  - GET / - API documentation");

    // Start the server. HTTP/1.1 only: TLS terminates at the ingress, which proxies HTTP/1.1,
    // and refusing cleartext HTTP/2 keeps h2 0.3 (RUSTSEC-2026-0258, pulled in by warp 0.3)
    // unreachable even via the NodePort. warp::serve offers no way to disable HTTP/2.
    info!("🚀 Starting HTTP server...");
    // warp::service() cannot pass the peer address to warp (warp::serve does that through a
    // crate-private hook), so record it on a span around each request instead; the
    // warp::trace::request() span nests inside it and its events carry remote.addr.
    let make_svc =
        hyper::service::make_service_fn(move |conn: &hyper::server::conn::AddrStream| {
            let remote_addr = conn.remote_addr();
            let svc = warp::service(routes.clone());
            async move {
                Ok::<_, Infallible>(hyper::service::service_fn(move |req| {
                    let span = tracing::info_span!("connection", remote.addr = %remote_addr);
                    // warp creates its request span inside call(), so enter ours for the call too.
                    let mut svc = svc.clone();
                    let response = span.in_scope(|| svc.call(req));
                    response.instrument(span)
                }))
            }
        });
    let server = hyper::Server::try_bind(&([0, 0, 0, 0], port).into())
        .map_err(|e| AppError::InternalError(format!("Failed to bind port {port}: {e}")))?
        .http1_only(true)
        .serve(make_svc)
        .with_graceful_shutdown(async {
            signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
            info!("🛑 Received shutdown signal, gracefully shutting down...");
        });

    info!("✅ Server is now running and accepting connections!");
    server
        .await
        .map_err(|e| AppError::InternalError(format!("HTTP server error: {e}")))?;

    info!("✅ Server shutdown complete");
    Ok(())
}

#[cfg(test)]
mod cors_tests {
    use super::{cors_filter, validate_cors_origins};
    use warp::Filter;

    /// Turn string literals into the owned list the config holds.
    fn origins(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Valid origins pass, with whitespace and a trailing slash trimmed.
    #[test]
    fn accepts_plain_origins_and_trims_them() {
        let validated = validate_cors_origins(&origins(&[
            "http://localhost:3000",
            " https://econ-graph.com/ ",
        ]))
        .unwrap();
        assert_eq!(
            validated,
            origins(&["http://localhost:3000", "https://econ-graph.com"])
        );
        // warp panics on origins it cannot parse; validated ones must not.
        let _ = cors_filter(&validated);
    }

    /// Wildcards, missing or non-http(s) schemes, paths and empty lists are all refused.
    #[test]
    fn rejects_wildcards_paths_and_empty_lists() {
        for bad in [
            vec!["*"],
            vec!["https://*"],
            vec!["econ-graph.com"],
            vec!["ftp://econ-graph.com"],
            vec!["https://econ-graph.com/app"],
            vec![""],
            vec![],
        ] {
            assert!(
                validate_cors_origins(&origins(&bad)).is_err(),
                "{:?} should be rejected",
                bad
            );
        }
    }

    /// A route wrapped in the backend's CORS filter for the local frontend only.
    fn app() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::any()
            .map(|| "ok")
            .with(cors_filter(&origins(&["http://localhost:3000"])))
    }

    /// A request from the configured origin succeeds and is told it may read the response.
    #[tokio::test]
    async fn allows_requests_from_a_configured_origin() {
        let res = warp::test::request()
            .method("GET")
            .header("origin", "http://localhost:3000")
            .reply(&app())
            .await;
        assert_eq!(res.status(), 200);
        assert_eq!(
            res.headers()["access-control-allow-origin"],
            "http://localhost:3000"
        );
    }

    /// A request from any other origin is refused.
    #[tokio::test]
    async fn refuses_requests_from_other_origins() {
        let res = warp::test::request()
            .method("GET")
            .header("origin", "https://evil.example")
            .reply(&app())
            .await;
        assert_eq!(res.status(), 403);
        assert!(res.headers().get("access-control-allow-origin").is_none());
    }

    /// A preflight from the configured origin gets the allowed methods and headers,
    /// and one from another origin is refused.
    #[tokio::test]
    async fn answers_preflight_only_for_configured_origins() {
        let preflight = |origin: &'static str| {
            warp::test::request()
                .method("OPTIONS")
                .header("origin", origin)
                .header("access-control-request-method", "POST")
                .header(
                    "access-control-request-headers",
                    "authorization, content-type",
                )
        };

        let res = preflight("http://localhost:3000").reply(&app()).await;
        assert_eq!(res.status(), 200);
        assert_eq!(
            res.headers()["access-control-allow-origin"],
            "http://localhost:3000"
        );
        let methods = res.headers()["access-control-allow-methods"]
            .to_str()
            .unwrap()
            .to_string();
        assert!(methods.contains("POST"), "{}", methods);

        let res = preflight("https://evil.example").reply(&app()).await;
        assert_eq!(res.status(), 403);
        assert!(res.headers().get("access-control-allow-origin").is_none());
    }
}

#[cfg(test)]
mod request_auth_tests {
    use super::{
        graphql_caller, mcp_access, mcp_route, mcp_unauthorized, read_body_limited, GraphqlRefusal,
        McpRejection,
    };
    use econ_graph_auth::testkit::{now, sign_hs256, TestIssuer};
    use econ_graph_auth::{OidcConfig, OidcVerifier, Role};
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
            eprintln!("DATABASE_URL not set; skipping DB-backed request auth test");
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

    /// With sign-in disabled, GraphQL ignores any token and serves the request anonymously.
    #[tokio::test]
    async fn graphql_is_anonymous_when_sign_in_is_disabled() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["admin.users:read"]);
        let pool = unreachable_pool();
        for header in [
            None,
            Some("Bearer not.a.jwt".to_string()),
            Some(bearer(&token)),
        ] {
            let caller = graphql_caller(&pool, None, header.as_deref()).await;
            assert!(matches!(caller, Ok(None)), "{header:?}");
        }
    }

    /// With sign-in on, a request without a token is anonymous and never contacts the
    /// identity provider, so an outage cannot block browsing.
    #[tokio::test]
    async fn graphql_without_a_token_is_anonymous_and_needs_no_provider() {
        let verifier = unreachable_verifier();
        let pool = unreachable_pool();
        for header in [None, Some("Basic dXNlcjpwYXNz")] {
            let caller = graphql_caller(&pool, Some(&verifier), header).await;
            assert!(matches!(caller, Ok(None)), "{header:?}");
        }
    }

    /// A token that cannot be checked because the provider is down runs anonymously.
    #[tokio::test]
    async fn graphql_drops_a_token_it_cannot_check() {
        let issuer = TestIssuer::start().await;
        let token = issuer.mint_token(Uuid::new_v4(), &["annotation:create"]);
        let caller = graphql_caller(
            &unreachable_pool(),
            Some(&unreachable_verifier()),
            Some(&bearer(&token)),
        )
        .await;
        assert!(matches!(caller, Ok(None)));
    }

    /// Bad tokens get a 401 with an RFC 6750 challenge, never an anonymous response.
    #[tokio::test]
    async fn graphql_refuses_invalid_tokens_with_401() {
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        let sub = Uuid::new_v4();
        let mut expired = issuer.claims(sub, &[]);
        expired["exp"] = (now() - 3600).into();
        let hs256 = sign_hs256(
            &issuer.claims(sub, &[]),
            Some(issuer.current_key().kid()),
            b"a-shared-secret",
        );
        for header in [
            "Bearer ".to_string(),
            bearer("not.a.jwt"),
            bearer(&issuer.sign(&expired)),
            bearer(&hs256),
        ] {
            let reply = graphql_caller(&unreachable_pool(), Some(&verifier), Some(&header))
                .await
                .expect_err(&header)
                .into_response();
            assert_eq!(reply.status(), StatusCode::UNAUTHORIZED, "{header}");
            assert_eq!(
                reply.headers()["WWW-Authenticate"],
                r#"Bearer error="invalid_token""#
            );
            let body = warp::hyper::body::to_bytes(reply.into_body())
                .await
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["errors"][0]["extensions"]["code"], "UNAUTHENTICATED");
        }
    }

    /// A valid token signs the caller in with the token's roles, creating their `users` row
    /// on first sight; a suspended account is refused with 403.
    // Shares the database with `integration_tests`, which drop the schema.
    #[tokio::test]
    #[serial_test::serial]
    async fn graphql_signs_in_a_valid_token_and_refuses_a_suspended_account() {
        let Some(pool) = database().await else {
            return;
        };
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        let sub = Uuid::new_v4();
        let token = issuer.mint_token(sub, &["annotation:create", "offline_access"]);

        for _ in 0..2 {
            let caller = graphql_caller(&pool, Some(&verifier), Some(&bearer(&token)))
                .await
                .expect("signed in")
                .expect("a caller");
            assert_eq!(caller.user.id, sub);
            assert_eq!(caller.principal.user_id, sub);
            assert_eq!(
                caller.principal.roles,
                [Role::AnnotationCreate].into_iter().collect()
            );
            assert!(caller.user.is_active);
            assert_eq!(caller.user.email, format!("{sub}@example.test"));
            assert_eq!(caller.user.provider, "keycloak");
        }

        set_active(&pool, sub, false).await;
        let reply = graphql_caller(&pool, Some(&verifier), Some(&bearer(&token)))
            .await
            .expect_err("suspended");
        assert_eq!(reply, GraphqlRefusal::Suspended);
        assert_eq!(reply.into_response().status(), StatusCode::FORBIDDEN);
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

    /// The `users` row for a new subject: created once, with the token's email only when it
    /// is verified and free, and the same row for concurrent first requests.
    // Shares the database with `integration_tests`, which drop the schema.
    #[tokio::test]
    #[serial_test::serial]
    async fn the_users_row_is_created_on_first_sight() {
        use econ_graph_core::models::User;
        let Some(pool) = database().await else {
            return;
        };
        let placeholder = |user: &User| {
            user.email.starts_with(&format!("{}.", user.id))
                && user.email.ends_with("@users.invalid")
                && !user.email_verified
        };

        let id = Uuid::new_v4();
        let email = format!("{id}@example.test");
        let user = User::get_or_create_for_subject(&pool, id, Some(&email), true, Some("Ada"))
            .await
            .unwrap();
        assert_eq!(
            (user.id, user.email.as_str(), user.name.as_str()),
            (id, email.as_str(), "Ada")
        );
        assert_eq!(user.provider, "keycloak");
        assert!(user.email_verified && user.is_active);
        // Seen again, with other claims: the row is returned unchanged.
        let again = User::get_or_create_for_subject(&pool, id, Some("x@example.test"), true, None)
            .await
            .unwrap();
        assert_eq!(
            (again.email, again.name),
            (email.clone(), "Ada".to_string())
        );

        // Unverified, taken by another row, or too long: the placeholder, never the address.
        let too_long = format!("{}@example.test", "a".repeat(250));
        for (claimed, verified) in [
            (format!("{}@example.test", Uuid::new_v4()), false),
            (email.clone(), true),
            (too_long, true),
        ] {
            let user = User::get_or_create_for_subject(
                &pool,
                Uuid::new_v4(),
                Some(&claimed),
                verified,
                None,
            )
            .await
            .unwrap();
            assert!(placeholder(&user), "{claimed}: {}", user.email);
            assert_eq!(user.name, "New user");
        }
        let no_email = User::get_or_create_for_subject(&pool, Uuid::new_v4(), None, true, None)
            .await
            .unwrap();
        assert!(placeholder(&no_email));

        // Two first requests at once end up with one row.
        let id = Uuid::new_v4();
        let email = format!("{id}@example.test");
        let (a, b) = tokio::join!(
            User::get_or_create_for_subject(&pool, id, Some(&email), true, None),
            User::get_or_create_for_subject(&pool, id, Some(&email), true, None),
        );
        let (a, b) = (a.unwrap(), b.unwrap());
        assert_eq!((a.id, &a.email), (b.id, &b.email));
        assert_eq!(a.email, email);
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
}

#[cfg(test)]
mod playground_tests {
    use super::{playground_enabled, root_handler};
    use warp::Reply;

    /// The landing page body, as served with the playground on or off.
    async fn landing_page(playground: bool) -> String {
        let response = root_handler(playground).await.unwrap().into_response();
        let body = warp::hyper::body::to_bytes(response.into_body())
            .await
            .unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    /// The landing page links to `/playground` only when it is served.
    #[tokio::test]
    async fn landing_page_mentions_playground_only_when_enabled() {
        assert!(landing_page(true).await.contains("href=\"/playground\""));
        assert!(!landing_page(false).await.contains("/playground"));
    }

    /// The playground is served only when explicitly turned on.
    #[test]
    fn playground_is_off_unless_explicitly_enabled() {
        for on in ["true", "TRUE", "tRuE", "1", " true "] {
            assert!(playground_enabled(Some(on.to_string())), "{on:?}");
        }
        for off in ["", "false", "0", "yes", "on"] {
            assert!(!playground_enabled(Some(off.to_string())), "{off:?}");
        }
        assert!(!playground_enabled(None));
    }
}
