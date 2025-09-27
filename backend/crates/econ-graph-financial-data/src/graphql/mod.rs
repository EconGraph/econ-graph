use crate::database::Database;
use crate::monitoring::{HealthChecker, MetricsCollector};
use async_graphql::{EmptySubscription, Schema};

pub mod mutation;
pub mod query;

pub use mutation::Mutation;
pub use query::Query;

pub async fn create_schema(
    database: Database,
    metrics: MetricsCollector,
    health_checker: HealthChecker,
) -> anyhow::Result<Schema<Query, Mutation, EmptySubscription>> {
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(database)
        .data(metrics)
        .data(health_checker)
        .finish();
    Ok(schema)
}

/// Create schema for testing (with default monitoring components)
pub async fn create_test_schema(
    database: Database,
) -> anyhow::Result<Schema<Query, Mutation, EmptySubscription>> {
    let metrics = MetricsCollector::new();
    let health_checker = HealthChecker::new();
    create_schema(database, metrics, health_checker).await
}
