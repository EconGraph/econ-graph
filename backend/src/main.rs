// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::GraphQLResponse;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::signal;
use tracing::info;
use warp::Filter;

mod auth;
mod config;
mod database;
mod error;
mod graphql;
mod mcp_server;
mod metrics;
mod metrics_enhanced;
mod metrics_service;
mod models;
mod schema;
mod services;

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod integration_tests;

use auth::routes::auth_routes;
use auth::services::AuthService;
use config::Config;
use database::{create_pool, DatabasePool};
use error::{AppError, AppResult};
use graphql::schema::create_schema_with_data;
// use services::crawler::start_crawler; // TODO: Implement start_crawler function

#[derive(Clone)]
pub struct AppState {
    pub pool: DatabasePool,
    pub schema: async_graphql::Schema<
        graphql::query::Query,
        graphql::mutation::Mutation,
        async_graphql::EmptySubscription,
    >,
}

async fn graphql_handler(
    schema: async_graphql::Schema<
        graphql::query::Query,
        graphql::mutation::Mutation,
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

async fn root_handler() -> Result<impl warp::Reply, Infallible> {
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

        <div class="endpoint">
            <div><span class="method">GET</span> <code>/playground</code></div>
            <p><a href="/playground">Interactive GraphQL Playground</a> - Test queries and explore the schema</p>
        </div>

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
        <p>Visit the <a href="/playground">GraphQL Playground</a> to start exploring economic data!</p>

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

    Ok(warp::reply::html(
        html.replace("{}", env!("CARGO_PKG_VERSION")),
    ))
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

    info!("📊 Configuration loaded successfully:");
    info!("  - Server host: {}", config.server.host);
    info!("  - Server port: {}", config.server.port);
    info!("  - CORS origins: {:?}", config.cors.allowed_origins);
    info!("  - Database URL: {}", config.database_url);

    // Create database connection pool
    info!("🗄️  Creating database connection pool...");
    info!("  - Database URL: {}", config.database_url);

    let pool = create_pool(&config.database_url).await.map_err(|e| {
        let error = AppError::DatabaseError(format!("Failed to create database pool: {}", e));
        error.log_with_context("Application startup database pool creation");
        eprintln!("❌ Failed to create database pool: {}", e);
        error
    })?;

    info!("✅ Database connection pool created successfully");

    // Run migrations
    info!("🔄 Running database migrations...");
    database::run_migrations(&config.database_url)
        .await
        .map_err(|e| {
            let error = AppError::DatabaseError(format!("Failed to run migrations: {}", e));
            error.log_with_context("Application startup database migrations");
            eprintln!("❌ Failed to run migrations: {}", e);
            error
        })?;

    info!("✅ Database migrations completed successfully");

    // Create GraphQL schema
    let schema = create_schema_with_data(pool.clone());
    info!("🎯 GraphQL schema created");

    // Create authentication service
    let auth_service = AuthService::new(pool.clone());
    info!("🔐 Authentication service created");

    // Initialize metrics
    info!("📊 Initializing Prometheus metrics...");
    let _metrics = &metrics::METRICS; // Initialize original metrics
    let _enhanced_metrics = &metrics_enhanced::METRICS; // Initialize enhanced metrics
    info!("✅ Prometheus metrics initialized");

    // Start metrics service for periodic updates
    info!("🔄 Starting metrics service...");
    metrics_service::start_metrics_service(Arc::new(pool.clone())).await
        .map_err(|e| {
            let error = AppError::DatabaseError(format!("Failed to start metrics service: {}", e));
            error.log_with_context("Application startup metrics service");
            eprintln!("❌ Failed to start metrics service: {}", e);
            error
        })?;
    info!("✅ Metrics service started successfully");

    // Start uptime counter task
    let uptime_task = tokio::spawn(async {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            metrics::increment_uptime(60);
        }
    });

    // Start background crawler (if enabled in config)
    // For now, crawler is always enabled - in production this could be configurable
    info!("🕷️  Starting background crawler...");
    // TODO: Implement crawler startup
    // match start_crawler().await {
    //     Ok(_) => info!("✅ Background crawler started successfully"),
    //     Err(e) => {
    //         eprintln!("⚠️  Warning: Failed to start background crawler: {}", e);
    //         info!("⚠️  Background crawler failed to start, continuing without crawler");
    //     }
    // }
    info!("⚠️  Background crawler startup temporarily disabled");

    // Create Warp filters
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type", "authorization"])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"]);

    // GraphQL endpoint with authentication
    let pool_for_graphql = pool.clone();
    let graphql_filter = warp::path("graphql")
        .and(warp::header::headers_cloned())
        .and(async_graphql_warp::graphql(schema.clone()))
        .and_then(
            move |headers: warp::http::HeaderMap<warp::http::HeaderValue>,
                  (schema, request): (
                async_graphql::Schema<
                    graphql::query::Query,
                    graphql::mutation::Mutation,
                    async_graphql::EmptySubscription,
                >,
                async_graphql::Request,
            )| {
                let pool_for_graphql = pool_for_graphql.clone();
                async move {
                    // Extract JWT token from Authorization header
                    let user = if let Some(auth_header) = headers.get("authorization") {
                        if let Ok(auth_str) = std::str::from_utf8(auth_header.as_bytes()) {
                            if auth_str.starts_with("Bearer ") {
                                let token = auth_str.trim_start_matches("Bearer ");
                                // Validate token and get user
                                let auth_service = crate::auth::services::AuthService::new(
                                    pool_for_graphql.clone(),
                                );
                                match auth_service.verify_token(token) {
                                    Ok(claims) => crate::models::User::get_by_id(
                                        &pool_for_graphql,
                                        claims.sub.parse().unwrap_or_default(),
                                    )
                                    .await
                                    .ok(),
                                    Err(_) => None, // Invalid token, continue without user
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Create authenticated GraphQL context
                    let auth_context =
                        std::sync::Arc::new(graphql::context::GraphQLContext::new(user));
                    let auth_schema = graphql::schema::create_schema_with_auth(
                        pool_for_graphql.clone(),
                        auth_context,
                    );

                    Ok::<_, Infallible>(GraphQLResponse::from(auth_schema.execute(request).await))
                }
            },
        );

    // GraphQL Playground
    let playground_filter = warp::path("playground")
        .and(warp::get())
        .and_then(graphql_playground);

    // Health check
    let health_filter = warp::path("health").and(warp::get()).and_then(health_check);

    // Metrics endpoint for Prometheus
    let metrics_filter = warp::path("metrics")
        .and(warp::get())
        .and_then(metrics_enhanced::metrics_handler);

    // Root endpoint
    let root_filter = warp::path::end().and(warp::get()).and_then(root_handler);

    // Authentication routes
    let auth_filter = auth_routes(auth_service);

    // MCP Server routes
    let mcp_server = Arc::new(mcp_server::EconGraphMcpServer::new(Arc::new(pool.clone())));

    // Create a simple MCP handler that doesn't rely on complex filter chaining
    let mcp_handler = {
        let server = mcp_server.clone();
        move |body: warp::hyper::body::Bytes| {
            let server = server.clone();
            async move { mcp_server::mcp_handler(body, server).await }
        }
    };

    let mcp_filter = warp::path("mcp")
        .and(warp::post())
        .and(warp::body::bytes())
        .and_then(mcp_handler);

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
    info!(
        "🎮 GraphQL Playground available at http://localhost:{}/playground",
        port
    );
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
    info!("  - GET /playground - GraphQL Playground");
    info!("  - GET /health - Health check");
    info!("  - GET /metrics - Prometheus metrics");
    info!("  - GET / - API documentation");

    // Start the server
    info!("🚀 Starting HTTP server...");
    let (_, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([0, 0, 0, 0], port), async {
            signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
            info!("🛑 Received shutdown signal, gracefully shutting down...");
        });

    info!("✅ Server is now running and accepting connections!");
    server.await;

    info!("✅ Server shutdown complete");
    Ok(())
}
