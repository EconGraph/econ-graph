use anyhow::Result;
use std::time::Instant;
use tracing::info;

mod crawler;
mod database;
mod graphql;
mod models;
mod monitoring;
mod storage;

use crate::database::Database;
use crate::graphql::create_schema;
use crate::monitoring::{HealthChecker, MetricsCollector};

// Type aliases to reduce complexity
type AppState = (
    async_graphql::Schema<graphql::Query, graphql::Mutation, async_graphql::EmptySubscription>,
    Database,
    MetricsCollector,
    HealthChecker,
);

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter("econ_graph_financial_data=debug,tower_http=debug")
        .init();

    let _start_time = Instant::now();
    info!("Starting EconGraph Financial Data Service");

    // Initialize monitoring
    let metrics = MetricsCollector::new();
    let health_checker = HealthChecker::new();

    info!("Monitoring initialized");

    // Initialize database with in-memory storage for now
    let database = Database::new_in_memory().await?;
    info!("Database initialized successfully");

    // Create GraphQL schema
    let schema = create_schema(database.clone()).await?;
    info!("GraphQL schema created successfully");

    // Start the server
    start_server(schema, database, metrics, health_checker).await?;

    Ok(())
}

async fn start_server(
    schema: async_graphql::Schema<
        graphql::Query,
        graphql::Mutation,
        async_graphql::EmptySubscription,
    >,
    database: Database,
    metrics: MetricsCollector,
    health_checker: HealthChecker,
) -> Result<()> {
    use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
    use axum::{extract::State, response::Html, routing::post, Router};

    async fn graphql_handler(
        State((schema, database, _metrics, _health_checker)): State<AppState>,
        req: GraphQLRequest,
    ) -> GraphQLResponse {
        let req = req.into_inner().data(database);
        schema.execute(req).await.into()
    }

    async fn graphql_playground() -> Html<String> {
        Html(async_graphql::http::playground_source(
            async_graphql::http::GraphQLPlaygroundConfig::new("/graphql"),
        ))
    }

    async fn health_handler(
        State((_schema, _database, metrics, health_checker)): State<AppState>,
    ) -> axum::response::Json<serde_json::Value> {
        let overall_health = health_checker.get_overall_health().await;
        let health_report = health_checker.get_health_report().await;
        let metrics_summary = metrics.get_metrics_summary().await;

        let status = match overall_health {
            crate::monitoring::HealthStatus::Healthy => "healthy",
            crate::monitoring::HealthStatus::Degraded => "degraded",
            crate::monitoring::HealthStatus::Unhealthy => "unhealthy",
        };

        axum::response::Json(serde_json::json!({
            "status": status,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "checks": health_report,
            "metrics": metrics_summary
        }))
    }

    async fn metrics_handler(
        State((_schema, _database, metrics, _health_checker)): State<AppState>,
    ) -> axum::response::Json<serde_json::Value> {
        let metrics_summary = metrics.get_metrics_summary().await;
        axum::response::Json(serde_json::json!(metrics_summary))
    }

    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/", axum::routing::get(graphql_playground))
        .route("/health", axum::routing::get(health_handler))
        .route("/metrics", axum::routing::get(metrics_handler))
        .with_state((schema, database, metrics, health_checker));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    info!("Financial Data Service listening on http://0.0.0.0:3001");
    info!("GraphQL Playground available at http://0.0.0.0:3001");

    axum::serve(listener, app).await?;

    Ok(())
}
