use warp::Filter;
use async_graphql::Schema;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::{GraphQLBadRequest, GraphQLResponse};
use std::sync::Arc;
use tokio::signal;
use tracing::{info, warn};
use std::convert::Infallible;
use serde_json::json;

mod config;
mod database;
mod error;
mod graphql;
mod models;
mod schema;
mod services;

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod integration_tests;

use config::Config;
use database::{create_pool, DatabasePool};
use error::{AppError, AppResult};
use graphql::schema::{create_schema_with_data};
use services::crawler::start_crawler;

#[derive(Clone)]
pub struct AppState {
    pub pool: DatabasePool,
    pub schema: async_graphql::Schema<graphql::query::Query, graphql::mutation::Mutation, async_graphql::EmptySubscription>,
}

async fn graphql_handler(
    schema: async_graphql::Schema<graphql::query::Query, graphql::mutation::Mutation, async_graphql::EmptySubscription>,
    request: async_graphql::Request,
) -> Result<GraphQLResponse, Infallible> {
    Ok(GraphQLResponse::from(schema.execute(request).await))
}

async fn graphql_playground() -> Result<impl warp::Reply, Infallible> {
    Ok(warp::reply::html(playground_source(
        GraphQLPlaygroundConfig::new("/graphql")
    )))
}

async fn health_check() -> Result<impl warp::Reply, Infallible> {
    Ok(warp::reply::json(&json!({
        "status": "healthy",
        "service": "econ-graph-backend",
        "version": env!("CARGO_PKG_VERSION")
    })))
}

async fn root_handler() -> Result<impl warp::Reply, Infallible> {
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
    
    Ok(warp::reply::html(html.replace("{}", env!("CARGO_PKG_VERSION"))))
}

#[tokio::main]
async fn main() -> AppResult<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("🚀 Starting EconGraph Backend Server v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = Config::from_env().map_err(|e| {
        AppError::ConfigError(format!("Failed to load configuration: {}", e))
    })?;

    info!("📊 Configuration loaded successfully");

    // Create database connection pool
    let pool = create_pool(&config.database_url).await.map_err(|e| {
        AppError::DatabaseError(format!("Failed to create database pool: {}", e))
    })?;

    info!("🗄️  Database connection pool created");

    // Run migrations
    database::run_migrations(&config.database_url).await.map_err(|e| {
        AppError::DatabaseError(format!("Failed to run migrations: {}", e))
    })?;

    info!("🔄 Database migrations completed");

    // Create GraphQL schema
    let schema = create_schema_with_data(pool.clone());
    info!("🎯 GraphQL schema created");

    // Start background crawler (if enabled in config)
    // For now, crawler is always enabled - in production this could be configurable
    info!("🕷️  Starting background crawler...");
    start_crawler().await;
    info!("✅ Background crawler started");

    // Create Warp filters
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type", "authorization"])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"]);

    // GraphQL endpoint
    let graphql_filter = async_graphql_warp::graphql(schema.clone())
        .and_then(|(schema, request): (async_graphql::Schema<graphql::query::Query, graphql::mutation::Mutation, async_graphql::EmptySubscription>, async_graphql::Request)| async move {
            Ok::<_, Infallible>(GraphQLResponse::from(schema.execute(request).await))
        });

    // GraphQL Playground
    let playground_filter = warp::path("playground")
        .and(warp::get())
        .and_then(graphql_playground);

    // Health check
    let health_filter = warp::path("health")
        .and(warp::get())
        .and_then(health_check);

    // Root endpoint
    let root_filter = warp::path::end()
        .and(warp::get())
        .and_then(root_handler);

    // Combine all routes
    let routes = root_filter
        .or(graphql_filter)
        .or(playground_filter)
        .or(health_filter)
        .with(cors)
        .with(warp::trace::request());

    let port = config.server.port;
    info!("🌐 Server starting on http://0.0.0.0:{}", port);
    info!("🎮 GraphQL Playground available at http://localhost:{}/playground", port);
    info!("❤️  Health check available at http://localhost:{}/health", port);

    // Start the server
    let (_, server) = warp::serve(routes)
        .bind_with_graceful_shutdown(([0, 0, 0, 0], port), async {
            signal::ctrl_c()
                .await
                .expect("Failed to listen for ctrl+c");
            info!("🛑 Received shutdown signal, gracefully shutting down...");
        });

    server.await;

    info!("✅ Server shutdown complete");
    Ok(())
}