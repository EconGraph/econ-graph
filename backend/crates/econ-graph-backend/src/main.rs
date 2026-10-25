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
    config::{
        backend_database_urls, backend_pool_sizes, data_graphql_config, BackendDatabaseUrls,
        BackendPoolSizes, DataGraphqlConfig,
    },
    create_pool_with_size,
    database::{readiness_check_set, run_migration_set, MigrationSet},
    models::admin::{ClientInfo, NewSecurityEvent},
    redact_database_url, AppError, AppPool, AppResult, Config, DataPool,
};
use econ_graph_graphql::graphql::schema::create_schema_with_data;
use econ_graph_services::services::series_check::DataPlaneClient;

#[cfg(test)]
mod databases_tests;
mod integration_tests;
#[cfg(flag_mcp)]
mod mcp_routes;
mod metrics;

#[cfg(flag_mcp)]
use mcp_routes::mcp_filter;

/// The backend's databases: the app database always, the data database when
/// `DATA_DATABASE_URL` is set. Both may be the same database, and then share one pool.
#[derive(Clone)]
pub(crate) struct Databases {
    pub(crate) app: AppPool,
    pub(crate) data: Option<DataPool>,
}

/// How long `/readyz` waits for each database. k8s's probe timeout (3s, see
/// backend-deployment.yaml) is well under the pool's 30s connection timeout, so each check is
/// bounded rather than letting a down database make the handler outlive the probe. The two
/// databases are checked at the same time.
const READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// The data database's state, as `/readyz` reports it. None of them makes the backend unready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DataReadiness {
    /// Reachable, with the newest data migration this binary embeds recorded.
    Ok,
    /// No connection within [`READINESS_TIMEOUT`], or the connection failed.
    Unreachable,
    /// Reachable, but the data set is behind this binary (`crawler migrate` has not run).
    PendingMigrations,
    /// This backend has no `DATA_DATABASE_URL`.
    NotConfigured,
}

impl DataReadiness {
    fn as_str(self) -> &'static str {
        match self {
            DataReadiness::Ok => "ok",
            DataReadiness::Unreachable => "unreachable",
            DataReadiness::PendingMigrations => "pending_migrations",
            DataReadiness::NotConfigured => "not_configured",
        }
    }
}

impl Databases {
    /// Connect to the app database and apply its migration set, then connect to the data
    /// database when there is one.
    ///
    /// When `APP_DATABASE_URL` and `DATA_DATABASE_URL` are the same, one pool of
    /// `sizes.app + sizes.data` connections serves both, so the database sees no more
    /// connections than from a backend with one database URL. The data database is never
    /// migrated here: `crawler migrate` applies the data set.
    async fn open(urls: &BackendDatabaseUrls, sizes: BackendPoolSizes) -> AppResult<Self> {
        let pool = |url: &str, size: u32, which: &'static str| {
            let url = url.to_string();
            async move {
                create_pool_with_size(&url, size).await.map_err(|e| {
                    AppError::DatabaseError(format!(
                        "Failed to create the {which} database pool: {e}"
                    ))
                })
            }
        };

        let shared = urls.data.as_deref() == Some(urls.app.as_str());
        info!("🗄️  App database: {}", redact_database_url(&urls.app));
        let app_size = if shared {
            sizes.app + sizes.data
        } else {
            sizes.app
        };
        let app = AppPool(pool(&urls.app, app_size, "app").await?);

        info!("🔄 Applying the app migration set...");
        let applied = run_migration_set(&urls.app, MigrationSet::App)
            .await
            .map_err(|e| {
                AppError::DatabaseError(format!("Failed to run the app migrations: {e}"))
            })?;
        info!(
            "✅ App migrations are current (applied now: {})",
            if applied.is_empty() {
                "none".to_string()
            } else {
                applied.join(", ")
            }
        );

        let data = match &urls.data {
            Some(_) if shared => {
                info!("🗄️  Data database: the app database, sharing its pool");
                Some(DataPool(app.0.clone()))
            }
            Some(url) => {
                info!("🗄️  Data database: {}", redact_database_url(url));
                Some(DataPool(pool(url, sizes.data, "data").await?))
            }
            None => {
                tracing::warn!(
                    "DATA_DATABASE_URL is not set: data fields answer DATA_SERVED_REMOTELY"
                );
                None
            }
        };
        Ok(Self { app, data })
    }

    /// The app database's readiness: reachable within [`READINESS_TIMEOUT`] with its set
    /// current. The error says why, naming the app database.
    async fn app_readiness(&self) -> Result<(), String> {
        match tokio::time::timeout(
            READINESS_TIMEOUT,
            readiness_check_set(&self.app, MigrationSet::App),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(format!("app database: {e}")),
            Err(_) => Err("app database: readiness check timed out".to_string()),
        }
    }

    /// The data database's state; each step is bounded by [`READINESS_TIMEOUT`] in all.
    async fn data_readiness(&self) -> DataReadiness {
        let Some(data) = &self.data else {
            return DataReadiness::NotConfigured;
        };
        let check = async {
            if data.get().await.is_err() {
                return DataReadiness::Unreachable;
            }
            match readiness_check_set(data, MigrationSet::Data).await {
                Ok(()) => DataReadiness::Ok,
                Err(e) => {
                    tracing::warn!("data database readiness: {e}");
                    DataReadiness::PendingMigrations
                }
            }
        };
        tokio::time::timeout(READINESS_TIMEOUT, check)
            .await
            .unwrap_or(DataReadiness::Unreachable)
    }
}

/// Where `createAnnotation` checks that a new annotation's series exists, logged in one line at
/// startup: in the data database when the backend has one; otherwise through the data plane at
/// `DATA_GRAPHQL_URL` (the returned client); otherwise not at all.
fn data_plane_for_series_check(
    has_data_database: bool,
    config: Option<DataGraphqlConfig>,
) -> AppResult<Option<Arc<DataPlaneClient>>> {
    match (has_data_database, config) {
        (true, config) => {
            info!(
                "🔎 createAnnotation checks series in the data database{}",
                if config.is_some() {
                    " (DATA_GRAPHQL_URL is not used)"
                } else {
                    ""
                }
            );
            Ok(None)
        }
        (false, Some(config)) => {
            let client = DataPlaneClient::new(&config)?;
            info!(
                "🔎 createAnnotation checks series through the data plane at {} (Host: {}, timeout {:?})",
                client.url(),
                client.host().unwrap_or("from the URL"),
                client.timeout()
            );
            Ok(Some(Arc::new(client)))
        }
        (false, None) => {
            tracing::warn!(
                "Neither DATA_DATABASE_URL nor DATA_GRAPHQL_URL is set: createAnnotation does not \
                 check that the series exists"
            );
            Ok(None)
        }
    }
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

/// Liveness: this process is up and serving HTTP. No database access, so a slow or down
/// database never gets this pod killed and restarted.
async fn livez_handler() -> Result<impl warp::Reply, Infallible> {
    metrics::record_http_request("GET", "/livez", 200, 0.0);
    Ok(warp::reply::json(&json!({ "status": "alive" })))
}

/// Readiness (and startup): ready when the app database is reachable and its app migrations
/// are current ([`Databases::app_readiness`]). Unlike `/health`, this can fail, so a pod whose
/// app database is down or behind is taken out of rotation instead of serving errors.
///
/// The data database's state (`ok`, `unreachable`, `pending_migrations` or `not_configured`) is
/// reported as `data` in the body but never fails the check: sign-in and the other app
/// operations keep working through a data outage, and data fields fail on their own until it
/// is over.
async fn readyz_handler(databases: Databases) -> Result<impl warp::Reply, Infallible> {
    use warp::http::StatusCode;

    let (app, data) = tokio::join!(databases.app_readiness(), databases.data_readiness());
    let data = data.as_str();
    let (status, body) = match app {
        Ok(()) => (
            StatusCode::OK,
            json!({ "status": "ready", "app": "ok", "data": data }),
        ),
        Err(reason) => {
            tracing::warn!("readiness check failed: {reason}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                json!({ "status": "not_ready", "reason": reason, "data": data }),
            )
        }
    };
    metrics::record_http_request("GET", "/readyz", status.as_u16(), 0.0);
    Ok(warp::reply::with_status(warp::reply::json(&body), status))
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
                "Invalid CORS origin {:?}: expected scheme://host[:port], like https://econgraph.com",
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

/// The TCP peer of the connection a request arrived on. The server puts it on every request's
/// extensions, since `warp::service` cannot hand warp the peer address itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PeerAddr(std::net::SocketAddr);

/// An address range written `ip/prefix` (or a bare `ip`), as listed in `TRUSTED_PROXIES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IpRange {
    network: std::net::IpAddr,
    prefix: u8,
}

impl IpRange {
    fn parse(text: &str) -> Option<Self> {
        let (ip, prefix) = match text.split_once('/') {
            Some((ip, prefix)) => (ip, Some(prefix)),
            None => (text, None),
        };
        let network: std::net::IpAddr = ip.trim().parse().ok()?;
        let max = if network.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            Some(p) => p.trim().parse::<u8>().ok().filter(|&p| p <= max)?,
            None => max,
        };
        Some(Self { network, prefix })
    }

    fn contains(&self, ip: std::net::IpAddr) -> bool {
        use std::net::IpAddr;
        let mask = |bits: u32, prefix: u8| -> u128 {
            if prefix == 0 {
                0
            } else {
                (!0u128 << (bits - u32::from(prefix))) & (!0u128 >> (128 - bits))
            }
        };
        match (self.network, ip.to_canonical()) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                let m = mask(32, self.prefix);
                u128::from(u32::from(net)) & m == u128::from(u32::from(ip)) & m
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                let m = mask(128, self.prefix);
                u128::from(net) & m == u128::from(ip) & m
            }
            _ => false,
        }
    }
}

/// Parse `TRUSTED_PROXIES`: comma-separated addresses or `ip/prefix` ranges. Entries that do
/// not parse are logged and skipped.
fn parse_trusted_proxies(setting: Option<&str>) -> Vec<IpRange> {
    setting
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let range = IpRange::parse(entry);
            if range.is_none() {
                tracing::warn!("ignoring TRUSTED_PROXIES entry {entry:?}: not an address or range");
            }
            range
        })
        .collect()
}

/// The proxies whose `X-Forwarded-For` the backend believes, from `TRUSTED_PROXIES`. Empty
/// (the default) means the header is ignored and the peer address is the client.
static TRUSTED_PROXIES: once_cell::sync::Lazy<Vec<IpRange>> = once_cell::sync::Lazy::new(|| {
    parse_trusted_proxies(std::env::var("TRUSTED_PROXIES").ok().as_deref())
});

/// The client address to record for a request from `peer` carrying `forwarded_for`
/// (`X-Forwarded-For`).
///
/// The header is anyone's to write, so it counts only when the peer is a trusted proxy: then
/// the entries are read right to left, past further trusted proxies, and the first address
/// no trusted proxy owns is the client. A malformed entry stops the walk at the last address
/// that could be checked. With no trusted proxies the peer is always the client.
fn client_ip(
    peer: Option<std::net::IpAddr>,
    forwarded_for: Option<&str>,
    trusted: &[IpRange],
) -> Option<String> {
    let is_trusted = |ip: std::net::IpAddr| trusted.iter().any(|range| range.contains(ip));
    let mut client = peer?.to_canonical();
    if is_trusted(client) {
        let hops = forwarded_for.unwrap_or_default().rsplit(',').map(str::trim);
        for hop in hops {
            let Ok(ip) = hop.parse::<std::net::IpAddr>() else {
                break;
            };
            client = ip.to_canonical();
            if !is_trusted(client) {
                break;
            }
        }
    }
    Some(client.to_string())
}

/// Every `X-Forwarded-For` field line of a request, in order, joined with commas: RFC 9110
/// (section 5.3) makes that the header's value, so a proxy that adds its own line instead of
/// extending the last one is still read in full. A line that is not text stands in as an
/// empty entry, which stops [`client_ip`]'s walk there rather than skipping past it.
fn forwarded_for(headers: &warp::http::HeaderMap) -> Option<String> {
    let lines: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .map(|value| std::str::from_utf8(value.as_bytes()).unwrap_or_default())
        .collect();
    (!lines.is_empty()).then(|| lines.join(","))
}

/// Who sent a request: [`client_ip`] for its address and its `User-Agent` header, cut short.
fn client_info(peer: Option<PeerAddr>, headers: &warp::http::HeaderMap) -> ClientInfo {
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| std::str::from_utf8(value.as_bytes()).ok());
    ClientInfo::new(
        client_ip(
            peer.map(|PeerAddr(addr)| addr.ip()),
            forwarded_for(headers).as_deref(),
            &TRUSTED_PROXIES,
        ),
        user_agent,
    )
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
    pool: &AppPool,
    verifier: Option<&OidcVerifier>,
    authorization: Option<&str>,
    client: &ClientInfo,
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
            NewSecurityEvent::suspended_account(id, "/graphql", client)
                .record(pool)
                .await;
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
    _databases: Databases,
    _verifier: Option<Arc<OidcVerifier>>,
) -> warp::filters::BoxedFilter<(warp::reply::Response,)> {
    warp::any()
        .and_then(|| async { Err::<warp::reply::Response, _>(warp::reject::not_found()) })
        .boxed()
}

/// A `/graphql` body that isn't a GraphQL request (malformed JSON, wrong content type) is the
/// caller's mistake: answer 400 in GraphQL's own error shape rather than warp's plain-text
/// "Unhandled rejection: ..." 500. Every other rejection passes through to the next route.
async fn graphql_bad_request(
    rejection: warp::Rejection,
) -> Result<warp::reply::Response, warp::Rejection> {
    match rejection.find::<async_graphql_warp::GraphQLBadRequest>() {
        Some(bad) => Ok(warp::reply::with_status(
            warp::reply::json(&json!({"errors": [{"message": bad.0.to_string()}]})),
            bad.status(),
        )
        .into_response()),
        None => Err(rejection),
    }
}

/// The largest `/graphql` request body accepted, in bytes. GraphQL operations are small; the
/// biggest in the repo is under 10 KB.
const GRAPHQL_BODY_LIMIT: usize = 1024 * 1024;

/// A `/graphql` body of more than [`GRAPHQL_BODY_LIMIT`] bytes.
#[derive(Debug)]
struct GraphqlBodyTooLarge;

impl warp::reject::Reject for GraphqlBodyTooLarge {}

/// Collects a request body, giving up as soon as more than `limit` bytes have arrived.
///
/// This counts the bytes actually received rather than trusting `Content-Length`, which a
/// chunked request can carry alongside a much larger body.
async fn read_graphql_body<S, B>(body: S, limit: usize) -> Result<Vec<u8>, warp::Rejection>
where
    S: tokio_stream::Stream<Item = Result<B, warp::Error>>,
    B: warp::hyper::body::Buf,
{
    use tokio_stream::StreamExt as _;
    use warp::hyper::body::Buf as _;

    let mut body = std::pin::pin!(body);
    let mut collected = Vec::new();
    while let Some(chunk) = body.next().await {
        let mut chunk = chunk.map_err(|_| warp::reject::reject())?;
        if collected.len() + chunk.remaining() > limit {
            return Err(warp::reject::custom(GraphqlBodyTooLarge));
        }
        while chunk.has_remaining() {
            let bytes = chunk.chunk();
            collected.extend_from_slice(bytes);
            let read = bytes.len();
            chunk.advance(read);
        }
    }
    Ok(collected)
}

/// The `/graphql` request filter: a `GET` carries its query in the URL, and a `POST` body is
/// refused once it exceeds [`GRAPHQL_BODY_LIMIT`] bytes, before it is buffered or parsed. It
/// extracts what `async_graphql_warp::graphql` does, and rejects a body that isn't a GraphQL
/// request the same way.
fn graphql_request<E>(
    executor: E,
) -> impl Filter<Extract = ((E, async_graphql::Request),), Error = warp::Rejection> + Clone
where
    E: async_graphql::Executor,
{
    let post_executor = executor.clone();
    let get = warp::get().and(async_graphql_warp::graphql(executor));
    let post = warp::post()
        .and(warp::header::optional::<String>("content-type"))
        .and(warp::body::stream())
        .and_then(move |content_type: Option<String>, body| {
            let executor = post_executor.clone();
            async move {
                let bytes = read_graphql_body(body, GRAPHQL_BODY_LIMIT).await?;
                let request = async_graphql::http::receive_batch_body(
                    content_type,
                    bytes.as_slice(),
                    async_graphql::http::MultipartOptions::default(),
                )
                .await
                .and_then(async_graphql::BatchRequest::into_single)
                .map_err(|e| warp::reject::custom(async_graphql_warp::GraphQLBadRequest(e)))?;
                Ok::<_, warp::Rejection>((executor, request))
            }
        });
    get.or(post).unify()
}

/// An oversize `/graphql` body is answered 413 in GraphQL's own error shape. Every other
/// rejection passes through.
async fn graphql_body_too_large(
    rejection: warp::Rejection,
) -> Result<warp::reply::Response, warp::Rejection> {
    if rejection.find::<GraphqlBodyTooLarge>().is_none() {
        return Err(rejection);
    }
    Ok(warp::reply::with_status(
        warp::reply::json(&json!({"errors": [{"message":
            format!("Request body is larger than {GRAPHQL_BODY_LIMIT} bytes")}]})),
        warp::http::StatusCode::PAYLOAD_TOO_LARGE,
    )
    .into_response())
}

/// Every route this server answers on. There is no in-house sign-in: `/auth/*` answers nowhere
/// on this backend (the frontend's own `/auth/callback` is a separate ingress rule that serves
/// the frontend, not this process).
///
/// `data_plane` is where `createAnnotation` checks series when there is no data database
/// (see [`data_plane_for_series_check`]).
fn build_routes(
    databases: Databases,
    data_plane: Option<Arc<DataPlaneClient>>,
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
    let databases_for_graphql = databases.clone();
    let data_plane_for_graphql = data_plane;
    let verifier_for_graphql = verifier.clone();
    let graphql_filter = warp::path("graphql")
        .and(warp::header::headers_cloned())
        .and(warp::ext::optional::<PeerAddr>())
        .and(graphql_request(schema))
        .and_then(
            move |headers: warp::http::HeaderMap<warp::http::HeaderValue>,
                  peer: Option<PeerAddr>,
                  (_schema, request): (
                async_graphql::Schema<
                    econ_graph_graphql::graphql::query::Query,
                    econ_graph_graphql::graphql::mutation::Mutation,
                    async_graphql::EmptySubscription,
                >,
                async_graphql::Request,
            )| {
                let databases = databases_for_graphql.clone();
                let data_plane = data_plane_for_graphql.clone();
                let verifier = verifier_for_graphql.clone();
                async move {
                    let client = client_info(peer, &headers);
                    let caller = match graphql_caller(
                        &databases.app,
                        verifier.as_deref(),
                        authorization_header(&headers),
                        &client,
                    )
                    .await
                    {
                        Ok(caller) => caller,
                        Err(refusal) => return Ok::<_, Infallible>(refusal.into_response()),
                    };

                    let auth_context = std::sync::Arc::new(
                        econ_graph_graphql::graphql::context::GraphQLContext::new(caller)
                            .with_client(client),
                    );
                    let auth_schema =
                        econ_graph_graphql::graphql::schema::create_schema_with_data_plane(
                            databases.app,
                            databases.data,
                            data_plane,
                            auth_context,
                        );

                    Ok(GraphQLResponse::from(auth_schema.execute(request).await).into_response())
                }
            },
        );

    let graphql_filter = graphql_filter.recover(graphql_bad_request);
    let graphql_filter = graphql_filter.recover(graphql_body_too_large);

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

    // Health check (kept for existing callers; static, never fails)
    let health_filter = warp::path("health").and(warp::get()).and_then(health_check);

    // Liveness: process is up, no database access.
    let livez_filter = warp::path("livez").and(warp::get()).and_then(livez_handler);

    // Readiness: each database reachable and its migration set current.
    let databases_for_readyz = databases.clone();
    let readyz_filter = warp::path("readyz").and(warp::get()).and_then(move || {
        let databases = databases_for_readyz.clone();
        async move { readyz_handler(databases).await }
    });

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
    let mcp_filter = mcp_filter(databases, verifier);

    root_filter
        .or(graphql_filter)
        .or(playground_filter)
        .or(health_filter)
        .or(livez_filter)
        .or(readyz_filter)
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
    for name in [
        econ_graph_core::config::APP_DATABASE_URL,
        econ_graph_core::config::DATA_DATABASE_URL,
    ] {
        let set = std::env::var(name).is_ok_and(|v| !v.trim().is_empty());
        info!("  - {name}: {}", if set { "set" } else { "not set" });
    }

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

    // Reference data read at runtime ($REFERENCE_DATA_DIR): fail at startup, not on first use.
    let areas = econ_graph_core::reference::areas().map_err(|e| {
        let error = AppError::ConfigError(e.to_string());
        error.log_with_context("Application startup reference data");
        eprintln!("❌ {}", e);
        error
    })?;
    let abbreviations = econ_graph_core::reference::search_abbreviations().map_err(|e| {
        let error = AppError::ConfigError(e.to_string());
        error.log_with_context("Application startup reference data");
        eprintln!("❌ {}", e);
        error
    })?;
    info!(
        "🌍 Reference data loaded from {}: {} areas, {} search abbreviations",
        econ_graph_core::reference::data_dir().display(),
        areas.all().len(),
        abbreviations.len()
    );

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

    // APP_DATABASE_URL is required and DATA_DATABASE_URL optional; DATABASE_URL alone is a
    // startup error, never a fallback.
    let database_urls = backend_database_urls().map_err(|e| {
        let error = AppError::ConfigError(e.to_string());
        error.log_with_context("Application startup database configuration");
        eprintln!("❌ {}", e);
        error
    })?;
    let pool_sizes = backend_pool_sizes().map_err(|e| {
        let error = AppError::ConfigError(e.to_string());
        error.log_with_context("Application startup database configuration");
        eprintln!("❌ {}", e);
        error
    })?;
    // DATA_GRAPHQL_URL (with DATA_GRAPHQL_HOST and DATA_GRAPHQL_TIMEOUT_MS) is optional.
    let data_graphql = data_graphql_config().map_err(|e| {
        let error = AppError::ConfigError(e.to_string());
        error.log_with_context("Application startup data plane configuration");
        eprintln!("❌ {}", e);
        error
    })?;
    let databases = Databases::open(&database_urls, pool_sizes)
        .await
        .map_err(|e| {
            e.log_with_context("Application startup databases");
            eprintln!("❌ {}", e);
            e
        })?;
    let data_plane =
        data_plane_for_series_check(databases.data.is_some(), data_graphql).map_err(|e| {
            e.log_with_context("Application startup data plane client");
            eprintln!("❌ {}", e);
            e
        })?;

    // Create GraphQL schema
    let schema = econ_graph_graphql::graphql::schema::create_schema_with_data_plane(
        databases.app.clone(),
        databases.data.clone(),
        data_plane.clone(),
        (),
    );
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
        databases.clone(),
        data_plane,
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
    info!("  - GET /livez - Liveness probe");
    info!("  - GET /readyz - Readiness probe");
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
                Ok::<_, Infallible>(hyper::service::service_fn(
                    move |mut req: hyper::Request<hyper::Body>| {
                        req.extensions_mut().insert(PeerAddr(remote_addr));
                        let span = tracing::info_span!("connection", remote.addr = %remote_addr);
                        // warp creates its request span inside call(), so enter ours for the call too.
                        let mut svc = svc.clone();
                        let response = span.in_scope(|| svc.call(req));
                        response.instrument(span)
                    },
                ))
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
            " https://econgraph.com/ ",
        ]))
        .unwrap();
        assert_eq!(
            validated,
            origins(&["http://localhost:3000", "https://econgraph.com"])
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
            vec!["econgraph.com"],
            vec!["ftp://econgraph.com"],
            vec!["https://econgraph.com/app"],
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

    /// The list deploy.sh gives the kind cluster with PUBLIC_HOST=econgraph.com (ECO-457):
    /// the public site and the on-host http://localhost both reach the API. Browsers send
    /// Origin on same-origin POSTs as well, so the app's own origin has to be listed.
    /// www.econgraph.com only ever redirects to the apex, so it is never an origin.
    #[tokio::test]
    async fn the_deployed_public_and_local_origins_are_allowed() {
        let deployed: Vec<String> = "https://econgraph.com,http://localhost,http://localhost:3000"
            .split(',')
            .map(str::to_string)
            .collect();
        let app = warp::any()
            .map(|| "ok")
            .with(cors_filter(&validate_cors_origins(&deployed).unwrap()));
        for origin in ["https://econgraph.com", "http://localhost"] {
            let res = warp::test::request()
                .method("POST")
                .header("origin", origin)
                .reply(&app)
                .await;
            assert_eq!(res.status(), 200, "{origin}");
            assert_eq!(res.headers()["access-control-allow-origin"], origin);
        }
        for origin in ["https://www.econgraph.com", "http://econgraph.com"] {
            let res = warp::test::request()
                .method("POST")
                .header("origin", origin)
                .reply(&app)
                .await;
            assert_eq!(res.status(), 403, "{origin}");
        }
    }
}

#[cfg(test)]
mod client_ip_tests {
    use super::{client_info, client_ip, forwarded_for, parse_trusted_proxies, IpRange, PeerAddr};
    use std::net::IpAddr;

    fn ip(text: &str) -> Option<IpAddr> {
        Some(text.parse().unwrap())
    }

    /// With no trusted proxies the peer is the client and `X-Forwarded-For` is ignored.
    #[test]
    fn untrusted_peer_is_the_client() {
        assert_eq!(
            client_ip(ip("203.0.113.9"), Some("1.2.3.4"), &[]).as_deref(),
            Some("203.0.113.9")
        );
        let trusted = parse_trusted_proxies(Some("10.0.0.0/8"));
        assert_eq!(
            client_ip(ip("203.0.113.9"), Some("1.2.3.4"), &trusted).as_deref(),
            Some("203.0.113.9")
        );
        assert_eq!(client_ip(None, Some("1.2.3.4"), &trusted), None);
    }

    /// Behind trusted proxies the client is the rightmost address no trusted proxy owns, so a
    /// client cannot pass itself off as anyone by prepending entries.
    #[test]
    fn trusted_proxies_vouch_for_the_forwarded_address() {
        let trusted = parse_trusted_proxies(Some(" 10.0.0.0/8, 192.168.1.1 ,bogus, ::1"));
        assert_eq!(trusted.len(), 3);
        let from = |xff| client_ip(ip("10.1.2.3"), xff, &trusted);
        assert_eq!(
            from(Some("6.6.6.6, 203.0.113.9")).as_deref(),
            Some("203.0.113.9")
        );
        assert_eq!(
            from(Some("6.6.6.6, 203.0.113.9, 192.168.1.1, 10.9.9.9")).as_deref(),
            Some("203.0.113.9")
        );
        // Nothing usable forwarded: the proxy itself is all that is known.
        assert_eq!(from(None).as_deref(), Some("10.1.2.3"));
        assert_eq!(from(Some("not-an-ip")).as_deref(), Some("10.1.2.3"));
        assert_eq!(
            from(Some("203.0.113.9, garbage, 10.0.0.1")).as_deref(),
            Some("10.0.0.1")
        );
        // An IPv4-mapped IPv6 peer is matched and recorded as IPv4.
        assert_eq!(
            client_ip(ip("::ffff:10.1.2.3"), Some("203.0.113.9"), &trusted).as_deref(),
            Some("203.0.113.9")
        );
        assert_eq!(
            client_ip(ip("::1"), Some("2001:db8::7"), &trusted).as_deref(),
            Some("2001:db8::7")
        );
    }

    /// `X-Forwarded-For` sent as several field lines is read as one list, in order, so a
    /// client cannot hide behind an extra line: the proxy's appended line is the one believed.
    #[test]
    fn forwarded_for_joins_every_field_line() {
        let mut headers = warp::http::HeaderMap::new();
        assert_eq!(forwarded_for(&headers), None);
        // The client sends its own line claiming to be 6.6.6.6 behind a trusted proxy; the
        // edge proxy adds a second line with the address it actually saw.
        headers.append("x-forwarded-for", "6.6.6.6, 10.0.0.7".parse().unwrap());
        headers.append("x-forwarded-for", "203.0.113.9".parse().unwrap());
        assert_eq!(
            forwarded_for(&headers).as_deref(),
            Some("6.6.6.6, 10.0.0.7,203.0.113.9")
        );
        let trusted = parse_trusted_proxies(Some("10.0.0.0/8"));
        let peer = ip("10.1.2.3");
        assert_eq!(
            client_ip(peer, forwarded_for(&headers).as_deref(), &trusted).as_deref(),
            Some("203.0.113.9")
        );
        // Reading only the first line would have believed the spoofed 6.6.6.6.
        assert_eq!(
            client_ip(peer, Some("6.6.6.6, 10.0.0.7"), &trusted).as_deref(),
            Some("6.6.6.6")
        );
        // A trusted hop in the later line is walked past to the address before it.
        let mut chained = warp::http::HeaderMap::new();
        chained.append("x-forwarded-for", "6.6.6.6".parse().unwrap());
        chained.append("x-forwarded-for", "198.51.100.4, 10.0.0.8".parse().unwrap());
        assert_eq!(
            client_ip(peer, forwarded_for(&chained).as_deref(), &trusted).as_deref(),
            Some("198.51.100.4")
        );
        // A line that is not text stops the walk there instead of being skipped.
        let mut opaque = warp::http::HeaderMap::new();
        opaque.append("x-forwarded-for", "6.6.6.6".parse().unwrap());
        opaque.append(
            "x-forwarded-for",
            warp::http::HeaderValue::from_bytes(b"\xff").unwrap(),
        );
        opaque.append("x-forwarded-for", "10.0.0.9".parse().unwrap());
        assert_eq!(
            client_ip(peer, forwarded_for(&opaque).as_deref(), &trusted).as_deref(),
            Some("10.0.0.9")
        );
    }

    #[test]
    fn ip_ranges() {
        let range = IpRange::parse("10.0.0.0/8").unwrap();
        assert!(range.contains("10.255.0.1".parse().unwrap()));
        assert!(!range.contains("11.0.0.1".parse().unwrap()));
        assert!(IpRange::parse("0.0.0.0/0")
            .unwrap()
            .contains("8.8.8.8".parse().unwrap()));
        assert!(IpRange::parse("10.0.0.0/33").is_none());
        let v6 = IpRange::parse("2001:db8::/32").unwrap();
        assert!(v6.contains("2001:db8:1::1".parse().unwrap()));
        assert!(!v6.contains("2001:db9::1".parse().unwrap()));
        assert!(!v6.contains("10.0.0.1".parse().unwrap()));
    }

    /// The request's peer and `User-Agent` become its client info; a long agent is cut.
    #[test]
    fn client_info_reads_the_request() {
        let mut headers = warp::http::HeaderMap::new();
        headers.insert("user-agent", "x".repeat(5000).parse().unwrap());
        headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        let info = client_info(
            Some(PeerAddr("203.0.113.9:4242".parse().unwrap())),
            &headers,
        );
        assert_eq!(info.ip_address.as_deref(), Some("203.0.113.9"));
        assert_eq!(info.user_agent.unwrap().len(), 512);
        assert_eq!(
            client_info(None, &warp::http::HeaderMap::new()),
            Default::default()
        );
    }
}

#[cfg(test)]
mod request_auth_tests {
    use super::{graphql_caller, GraphqlRefusal};
    use econ_graph_auth::testkit::{now, sign_hs256, TestIssuer};
    use econ_graph_auth::{OidcConfig, OidcVerifier, Role};
    use econ_graph_core::models::admin::ClientInfo;
    use econ_graph_core::test_utils::ScratchDatabase;
    use econ_graph_core::{AppPool, DatabasePool, MigrationSet};
    use uuid::Uuid;
    use warp::http::StatusCode;

    /// A pool that never connects: every case using it is decided before any database access.
    fn unreachable_pool() -> AppPool {
        let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
            diesel_async::AsyncPgConnection,
        >::new("postgres://nobody@127.0.0.1:1/none");
        AppPool(
            DatabasePool::builder()
                .connection_timeout(std::time::Duration::from_secs(1))
                .build_unchecked(manager),
        )
    }

    /// A new, empty app database on `DATABASE_URL`'s server with only the app migration set
    /// applied, as the backend's own is, or `None` to skip the test.
    async fn database() -> Option<(ScratchDatabase, AppPool)> {
        if std::env::var("DATABASE_URL").is_err() {
            eprintln!("DATABASE_URL not set; skipping DB-backed request auth test");
            return None;
        }
        let db = ScratchDatabase::create("request_auth_app");
        econ_graph_core::run_migration_set(&db.url, MigrationSet::App)
            .await
            .expect("app migrations");
        let pool = econ_graph_core::create_pool(&db.url).await.expect("pool");
        Some((db, AppPool(pool)))
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
            let caller =
                graphql_caller(&pool, None, header.as_deref(), &ClientInfo::default()).await;
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
            let caller =
                graphql_caller(&pool, Some(&verifier), header, &ClientInfo::default()).await;
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
            &ClientInfo::default(),
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
            let reply = graphql_caller(
                &unreachable_pool(),
                Some(&verifier),
                Some(&header),
                &ClientInfo::default(),
            )
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
    /// on first sight in a new app database (one with only the app set); a suspended account is
    /// refused with 403.
    #[tokio::test]
    #[serial_test::serial]
    async fn graphql_signs_in_a_valid_token_and_refuses_a_suspended_account() {
        let Some((_db, pool)) = database().await else {
            return;
        };
        let issuer = TestIssuer::start().await;
        let verifier = issuer.verifier();
        let sub = Uuid::new_v4();
        let token = issuer.mint_token(sub, &["annotation:create", "offline_access"]);

        for _ in 0..2 {
            let caller = graphql_caller(
                &pool,
                Some(&verifier),
                Some(&bearer(&token)),
                &ClientInfo::default(),
            )
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
        let client = ClientInfo::new(Some("198.51.100.4".into()), Some("SuspendedTest/2.0"));
        let reply = graphql_caller(&pool, Some(&verifier), Some(&bearer(&token)), &client)
            .await
            .expect_err("suspended");
        assert_eq!(reply, GraphqlRefusal::Suspended);
        assert_eq!(reply.into_response().status(), StatusCode::FORBIDDEN);

        // The refusal is recorded as a security event for the admin console.
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::security_events;
        let mut conn = pool.get().await.unwrap();
        let events: Vec<(String, Option<String>, Option<String>)> = security_events::table
            .filter(security_events::user_id.eq(sub))
            .select((
                security_events::event_type,
                security_events::ip_address,
                security_events::user_agent,
            ))
            .load(&mut conn)
            .await
            .unwrap();
        assert_eq!(
            events,
            [(
                "suspended_account".to_string(),
                Some("198.51.100.4".to_string()),
                Some("SuspendedTest/2.0".to_string())
            )]
        );
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
    #[tokio::test]
    #[serial_test::serial]
    async fn the_users_row_is_created_on_first_sight() {
        use econ_graph_core::models::User;
        let Some((_db, pool)) = database().await else {
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

    /// The landing page is public, so it must not name the internal `/metrics` endpoint.
    #[tokio::test]
    async fn landing_page_does_not_mention_metrics() {
        assert!(!landing_page(true).await.contains("/metrics"));
        assert!(!landing_page(false).await.contains("/metrics"));
    }

    /// `/livez` and `/readyz` are probe endpoints for the orchestrator, not part of the public
    /// API, so the landing page must not name them either.
    #[tokio::test]
    async fn landing_page_does_not_mention_liveness_or_readiness_probes() {
        for playground in [true, false] {
            let page = landing_page(playground).await;
            assert!(!page.contains("/livez"), "{page}");
            assert!(!page.contains("/readyz"), "{page}");
        }
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
    use super::{build_routes, Databases};
    use econ_graph_core::{AppPool, DataPool, DatabasePool};
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
        let databases = Databases {
            app: AppPool(unreachable_pool()),
            data: Some(DataPool(unreachable_pool())),
        };
        let schema = create_schema_with_data(databases.app.clone(), databases.data.clone(), ());
        let routes = build_routes(
            databases,
            None,
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
    use super::{
        build_routes, read_graphql_body, Databases, GraphqlBodyTooLarge, GRAPHQL_BODY_LIMIT,
    };
    use econ_graph_core::{AppPool, DataPool, DatabasePool};
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
        let databases = Databases {
            app: AppPool(unreachable_pool()),
            data: Some(DataPool(unreachable_pool())),
        };
        let schema = create_schema_with_data(databases.app.clone(), databases.data.clone(), ());
        build_routes(
            databases,
            None,
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

    /// A malformed `/graphql` body is a 400 JSON error, not a plain-text 500, and doesn't
    /// swallow the other routes' own rejections.
    #[tokio::test]
    async fn malformed_graphql_body_is_a_json_400() {
        let res = warp::test::request()
            .method("POST")
            .path("/graphql")
            .header("content-type", "application/json")
            .body(r#"{"query": "{ __typename }"#)
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 400);
        let body: serde_json::Value = serde_json::from_slice(res.body()).unwrap();
        assert!(body["errors"][0]["message"].as_str().is_some(), "{body}");
        assert!(!String::from_utf8_lossy(res.body()).contains("Unhandled"));

        // The 400 carries CORS headers like any other answer.
        let res = warp::test::request()
            .method("POST")
            .path("/graphql")
            .header("origin", "http://localhost:3000")
            .header("content-type", "application/json")
            .body("{")
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 400);
        assert_eq!(
            res.headers()["access-control-allow-origin"],
            "http://localhost:3000"
        );

        // Rejections that aren't a bad GraphQL body still pass through.
        let res = warp::test::request()
            .method("GET")
            .path("/nowhere")
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 404);
        let res = warp::test::request()
            .method("PUT")
            .path("/graphql")
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 405);
    }

    /// A `{ __typename }` request padded with trailing spaces to exactly `len` bytes.
    fn padded_request(len: usize) -> String {
        let body = r#"{"query": "{ __typename }"}"#;
        format!("{body}{}", " ".repeat(len - body.len()))
    }

    /// A `/graphql` body of exactly the limit is answered; one byte more is a JSON 413 with
    /// CORS headers. `GET` and the preflight are untouched.
    #[tokio::test]
    async fn oversize_graphql_body_is_a_json_413() {
        let post = |body: String| {
            warp::test::request()
                .method("POST")
                .path("/graphql")
                .header("origin", "http://localhost:3000")
                .header("content-type", "application/json")
                .body(body)
        };
        let res = post(padded_request(GRAPHQL_BODY_LIMIT))
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 200, "{:?}", res.body());

        let res = post(padded_request(GRAPHQL_BODY_LIMIT + 1))
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 413);
        let body: serde_json::Value = serde_json::from_slice(res.body()).unwrap();
        assert!(
            body["errors"][0]["message"]
                .as_str()
                .is_some_and(|m| m.contains("1048576")),
            "{body}"
        );
        assert_eq!(
            res.headers()["access-control-allow-origin"],
            "http://localhost:3000"
        );

        let res = warp::test::request()
            .method("GET")
            .path("/graphql?query=%7B__typename%7D")
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 200);
        let res = warp::test::request()
            .method("OPTIONS")
            .path("/graphql")
            .header("origin", "http://localhost:3000")
            .header("access-control-request-method", "POST")
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 200);
    }

    /// The limit counts the bytes that arrive, not the `Content-Length` a chunked request may
    /// carry alongside a larger body (warp's test client always sets an honest length).
    #[tokio::test]
    async fn graphql_body_limit_counts_streamed_bytes() {
        let chunk = |n: usize| Ok::<_, warp::Error>(warp::hyper::body::Bytes::from(vec![b'a'; n]));
        let at_limit = tokio_stream::iter(vec![chunk(600), chunk(400)]);
        assert_eq!(read_graphql_body(at_limit, 1000).await.unwrap().len(), 1000);
        let over = tokio_stream::iter(vec![chunk(600), chunk(401)]);
        let rejection = read_graphql_body(over, 1000).await.unwrap_err();
        assert!(rejection.find::<GraphqlBodyTooLarge>().is_some());
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

    /// `/livez` never touches the database, so it answers 200 even behind an unreachable pool.
    #[tokio::test]
    async fn livez_answers_200_even_when_the_database_is_unreachable() {
        let res = warp::test::request()
            .method("GET")
            .path("/livez")
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 200);
    }

    /// `/readyz` must not report ready when the database can't be reached: this is the bug
    /// ECO-230 fixes, where `/health` served all three probes and never caught this.
    #[tokio::test]
    async fn readyz_answers_503_when_the_database_is_unreachable() {
        let res = warp::test::request()
            .method("GET")
            .path("/readyz")
            .reply(&routes(false))
            .await;
        assert_eq!(res.status(), 503);
    }
}

#[cfg(test)]
mod series_check_startup_tests {
    use super::data_plane_for_series_check;
    use econ_graph_core::config::DataGraphqlConfig;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// What `f` returns, and the lines it logs.
    fn logged<T>(f: impl FnOnce() -> T) -> (T, Vec<String>) {
        let buffer = Buffer(Arc::new(Mutex::new(Vec::new())));
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || writer.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let text = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        (result, text.lines().map(str::to_string).collect())
    }

    fn config() -> DataGraphqlConfig {
        DataGraphqlConfig {
            url: "http://gateway.example/data/graphql".to_string(),
            host: Some("br-1.localhost".to_string()),
            timeout: std::time::Duration::from_secs(2),
        }
    }

    /// With neither a data database nor `DATA_GRAPHQL_URL`, the check is skipped and startup
    /// says so in one line.
    #[test]
    fn neither_skips_the_check_with_one_log_line() {
        let (data_plane, lines) = logged(|| data_plane_for_series_check(false, None));
        assert!(data_plane.unwrap().is_none());
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("WARN"), "{lines:?}");
        assert!(
            lines[0].contains("createAnnotation does not check that the series exists"),
            "{lines:?}"
        );
    }

    /// Without a data database, `DATA_GRAPHQL_URL` gives the data plane client, logged with
    /// its URL and `Host`.
    #[test]
    fn a_data_plane_url_is_used_without_a_data_database() {
        let (data_plane, lines) = logged(|| data_plane_for_series_check(false, Some(config())));
        let data_plane = data_plane.unwrap().expect("a data plane client");
        assert_eq!(data_plane.url(), "http://gateway.example/data/graphql");
        assert_eq!(data_plane.host(), Some("br-1.localhost"));
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].contains("through the data plane at http://gateway.example/data/graphql"),
            "{lines:?}"
        );
        assert!(lines[0].contains("br-1.localhost"), "{lines:?}");
    }

    /// A data database wins over `DATA_GRAPHQL_URL`.
    #[test]
    fn a_data_database_is_preferred() {
        for config in [None, Some(config())] {
            let (data_plane, lines) = logged(|| data_plane_for_series_check(true, config));
            assert!(data_plane.unwrap().is_none());
            assert_eq!(lines.len(), 1, "{lines:?}");
            assert!(lines[0].contains("in the data database"), "{lines:?}");
        }
    }
}
