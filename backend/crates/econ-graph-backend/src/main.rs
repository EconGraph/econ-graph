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
use econ_graph_auth::{authenticate, BearerError, Caller, OidcConfig, OidcVerifier, Role};
use econ_graph_core::{
    create_pool, redact_database_url, AppError, AppResult, Config, DatabasePool,
};
use econ_graph_graphql::graphql::schema::create_schema_with_data;

mod integration_tests;
#[cfg(flag_mcp)]
mod mcp_routes;
mod metrics;

#[cfg(flag_mcp)]
use mcp_routes::mcp_filter;

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

/// The `/mcp` entry on the API index page, shown only when this build routes it.
const MCP_ENDPOINT_HTML: &str = r#"        <div class="endpoint">
            <div><span class="method">POST</span> <code>/mcp</code></div>
            <p>MCP (Model Context Protocol) server endpoint - AI model integration for economic data access</p>
        </div>

"#;

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

<!--MCP_ENDPOINT-->
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
    let mcp = if cfg!(flag_mcp) {
        MCP_ENDPOINT_HTML
    } else {
        ""
    };
    Ok(warp::reply::html(
        html.replace("{}", env!("CARGO_PKG_VERSION"))
            .replace("<!--PLAYGROUND_ENDPOINT-->", endpoint)
            .replace("<!--PLAYGROUND_QUICK_START-->", quick_start)
            .replace("<!--MCP_ENDPOINT-->", mcp),
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

/// Stands in for `/mcp` when the `mcp` build flag is off: nothing answers there, so it
/// gets a 404, and none of the MCP route's code is compiled into this binary.
#[cfg(not(flag_mcp))]
fn mcp_filter(
    _pool: DatabasePool,
    _verifier: Option<Arc<OidcVerifier>>,
) -> warp::filters::BoxedFilter<(warp::reply::Response,)> {
    warp::any()
        .and_then(|| async { Err::<warp::reply::Response, _>(warp::reject::not_found()) })
        .boxed()
}

/// Every route this server answers on. There is no in-house sign-in: `/auth/*` answers nowhere
/// on this backend (the frontend's own `/auth/callback` is a separate ingress rule that serves
/// the frontend, not this process).
fn build_routes(
    pool: DatabasePool,
    schema: async_graphql::Schema<
        econ_graph_graphql::graphql::query::Query,
        econ_graph_graphql::graphql::mutation::Mutation,
        async_graphql::EmptySubscription,
    >,
    verifier: Option<Arc<OidcVerifier>>,
    cors_origins: &[String],
    playground_enabled: bool,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let cors = cors_filter(cors_origins);

    // GraphQL endpoint: anonymous without a token, the token's caller with a valid one.
    let pool_for_graphql = pool.clone();
    let verifier_for_graphql = verifier.clone();
    let graphql_filter = warp::path("graphql")
        .and(warp::header::headers_cloned())
        .and(async_graphql_warp::graphql(schema))
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

    // MCP route, compiled in only with the `mcp` build flag (see mcp_routes.rs). It requires
    // a signed-in user's bearer token holding `api:mcp`; MCP OAuth (auth roadmap phase 6)
    // will replace this.
    let mcp_filter = mcp_filter(pool, verifier);

    root_filter
        .or(graphql_filter)
        .or(playground_filter)
        .or(health_filter)
        .or(metrics_filter)
        .or(mcp_filter)
        .with(cors)
        .with(warp::trace::request())
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

    // Load configuration
    info!("📋 Loading configuration from environment...");
    let config = Config::from_env().map_err(|e| {
        let error = AppError::ConfigError(format!("Failed to load configuration: {}", e));
        error.log_with_context("Application startup configuration loading");
        eprintln!("❌ Failed to load configuration: {}", e);
        error
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

    // Reference data read at runtime ($REFERENCE_DATA_DIR): fail at startup, not on first use.
    let areas = econ_graph_core::reference::areas().map_err(|e| {
        let error = AppError::ConfigError(e.to_string());
        error.log_with_context("Application startup reference data");
        eprintln!("❌ {}", e);
        error
    })?;
    info!(
        "🌍 Reference data loaded from {}: {} areas",
        econ_graph_core::reference::data_dir().display(),
        areas.all().len()
    );

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

    // GraphQL Playground: off unless ENABLE_GRAPHQL_PLAYGROUND=true (local development).
    let playground_enabled = playground_enabled(std::env::var("ENABLE_GRAPHQL_PLAYGROUND").ok());
    let routes = build_routes(
        pool.clone(),
        schema.clone(),
        verifier.clone(),
        &cors_origins,
        playground_enabled,
    );

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
        .with_graceful_shutdown(shutdown_signal());

    info!("✅ Server is now running and accepting connections!");
    server
        .await
        .map_err(|e| AppError::InternalError(format!("HTTP server error: {e}")))?;

    info!("✅ Server shutdown complete");
    Ok(())
}

/// Resolves on SIGINT (Ctrl-C) or SIGTERM, so orchestrators that stop a container with SIGTERM
/// (Kubernetes, `docker stop`) get the same graceful drain as a local Ctrl-C.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            tracing::error!(error = %e, "listening for Ctrl-C failed");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let term = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => {
                tracing::error!(error = %e, "listening for SIGTERM failed");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => info!("🛑 Received SIGINT, gracefully shutting down..."),
        () = term => info!("🛑 Received SIGTERM, gracefully shutting down..."),
    }
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
    use super::{graphql_caller, GraphqlRefusal};
    use econ_graph_auth::testkit::{now, sign_hs256, TestIssuer};
    use econ_graph_auth::{OidcConfig, OidcVerifier, Role};
    use econ_graph_core::DatabasePool;
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

/// Only compiled when the `mcp` build flag is off, so these assert the actual behavior of a
/// release build rather than a stand-in checked by hand.
#[cfg(all(test, not(flag_mcp)))]
mod mcp_flag_off_tests {
    use super::build_routes;
    use econ_graph_core::DatabasePool;
    use econ_graph_graphql::graphql::schema::create_schema_with_data;

    fn unreachable_pool() -> DatabasePool {
        let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
            diesel_async::AsyncPgConnection,
        >::new("postgres://nobody@127.0.0.1:1/none");
        DatabasePool::builder()
            .connection_timeout(std::time::Duration::from_secs(1))
            .build_unchecked(manager)
    }

    /// With the `mcp` flag off, nothing is routed at `/mcp`: the full route set 404s on it,
    /// and no MCP-specific code is compiled into this binary.
    #[tokio::test]
    async fn mcp_answers_404() {
        let pool = unreachable_pool();
        let schema = create_schema_with_data(pool.clone(), ());
        let routes = build_routes(
            pool,
            schema,
            None,
            &["http://localhost:3000".to_string()],
            false,
        );
        let res = warp::test::request()
            .method("POST")
            .path("/mcp")
            .reply(&routes)
            .await;
        assert_eq!(res.status(), 404);
    }

    mod root_page_tests {
        use super::super::root_handler;
        use warp::Reply as _;

        /// The landing page lists `/mcp` only when this build routes it; with the flag off it
        /// does not.
        #[tokio::test]
        async fn lists_mcp_only_when_routed() {
            let response = root_handler(false).await.unwrap().into_response();
            let body = warp::hyper::body::to_bytes(response.into_body())
                .await
                .unwrap();
            let body = String::from_utf8(body.to_vec()).unwrap();
            assert!(!body.contains("/mcp"), "{body}");
        }
    }
}

#[cfg(test)]
mod route_tests {
    use super::build_routes;
    use econ_graph_core::DatabasePool;
    use econ_graph_graphql::graphql::schema::create_schema_with_data;

    /// A pool that never connects: routes that answer without touching the database don't need it.
    fn unreachable_pool() -> DatabasePool {
        let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
            diesel_async::AsyncPgConnection,
        >::new("postgres://nobody@127.0.0.1:1/none");
        DatabasePool::builder()
            .connection_timeout(std::time::Duration::from_secs(1))
            .build_unchecked(manager)
    }

    fn routes(
        playground_enabled: bool,
    ) -> impl warp::Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        let pool = unreachable_pool();
        let schema = create_schema_with_data(pool.clone(), ());
        build_routes(
            pool,
            schema,
            None,
            &["http://localhost:3000".to_string()],
            playground_enabled,
        )
    }

    /// There is no in-house sign-in: no route under `/auth/` answers on the backend, for any
    /// method. (The frontend's own `/auth/callback` is a separate ingress rule that serves the
    /// frontend, not this process, so it isn't covered here.)
    #[tokio::test]
    async fn no_route_under_auth_answers() {
        for path in [
            "/auth/login",
            "/auth/register",
            "/auth/logout",
            "/auth/refresh",
            "/auth/google",
            "/auth/facebook",
            "/auth/facebook/data-deletion",
            "/auth/callback",
            "/auth/",
            "/auth",
        ] {
            for method in ["GET", "POST", "DELETE"] {
                let res = warp::test::request()
                    .method(method)
                    .path(path)
                    .reply(&routes(false))
                    .await;
                assert_eq!(
                    res.status(),
                    404,
                    "{method} {path} should not be answered by the backend"
                );
            }
        }
    }

    /// Sanity check that the test harness's route set is wired up correctly: real routes still
    /// answer, so the 404s above are `/auth/*` being genuinely absent, not a broken filter.
    #[tokio::test]
    async fn other_routes_still_answer() {
        let res = warp::test::request()
            .method("GET")
            .path("/health")
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 200);
    }

    /// `GET /playground` answers 200 through the full route set when enabled.
    #[tokio::test]
    async fn playground_route_answers_when_enabled() {
        let res = warp::test::request()
            .method("GET")
            .path("/playground")
            .reply(&routes(true))
            .await;
        assert_eq!(res.status(), 200);
    }

    /// `GET /playground` answers 404 through the full route set when disabled.
    #[tokio::test]
    async fn playground_route_answers_404_when_disabled() {
        let res = warp::test::request()
            .method("GET")
            .path("/playground")
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 404);
    }
}
