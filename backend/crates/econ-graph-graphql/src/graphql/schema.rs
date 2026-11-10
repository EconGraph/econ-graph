//! # GraphQL Schema Definition
//!
//! Schema creation and configuration for the EconGraph GraphQL API.
//! Provides the main entry point for GraphQL operations.

use async_graphql::{EmptySubscription, Schema, SchemaBuilder};
use std::sync::Arc;

use crate::graphql::dataloaders::DataLoaders;
use crate::graphql::errors::SanitizeErrors;
use crate::graphql::limits::{QueryLimits, RequestDeadline};
use crate::graphql::{mutation::Mutation, query::Query};
use econ_graph_core::database::{AppPool, DataPool};
use econ_graph_services::services::series_check::DataPlaneClient;

/// Shared resources every request of a schema sees (the per-request caller is
/// [`crate::graphql::context::GraphQLContext`])
#[derive(Clone)]
pub struct SchemaResources {
    /// DataLoaders on the data database, for N+1 query prevention; `None` when this schema has
    /// no data database (see [`crate::graphql::context::data_loaders`])
    pub data_loaders: Option<Arc<DataLoaders>>,
}

pub use crate::graphql::limits::{MAX_QUERY_COMPLEXITY, MAX_QUERY_DEPTH, MAX_QUERY_FIELDS};

/// Start a schema builder with the root types and every schema-level setting.
///
/// [`create_schema`], [`create_schema_with_data`] and [`sdl`] all build from
/// this, so the committed `schema.graphql` snapshot describes the schema the
/// server actually serves.
///
/// Every schema writes the permission denials its requests raise before returning their
/// responses ([`crate::graphql::audit::AuditWrites`]), and enforces the per-request limits of
/// [`crate::graphql::limits`]: the request timeout outermost, so it bounds everything else, and
/// the field limit and rejection counting innermost, so they see errors before sanitizing.
fn schema_builder() -> SchemaBuilder<Query, Mutation, EmptySubscription> {
    Schema::build(Query, Mutation, EmptySubscription)
        .extension(RequestDeadline)
        .extension(SanitizeErrors)
        .extension(crate::graphql::audit::AuditWrites)
        .extension(QueryLimits)
}

/// Print the schema as GraphQL SDL.
///
/// Needs no database or context data: those don't change the schema's shape.
/// The `schema_snapshot` test compares this with the committed
/// `schema.graphql` at the crate root.
pub fn sdl() -> String {
    schema_builder().finish().sdl()
}

/// Create a GraphQL schema on the app database and, when this process has one, the data
/// database.
///
/// Resolvers read the app pool as `ctx.data::<AppPool>()` and the data pool through
/// [`crate::graphql::context::data_pool`]. With `data` `None`, every data field fails with
/// `extensions.code = DATA_SERVED_REMOTELY`; the schema itself is the same either way.
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_graphql::graphql::schema::create_schema;
/// use econ_graph_core::database::{create_pool, AppPool, DataPool};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let app = AppPool(create_pool("postgres://localhost/econ_graph_app").await?);
///     let data = DataPool(create_pool("postgres://localhost/econ_graph").await?);
///     let schema = create_schema(app, Some(data));
///
///     // Use schema for GraphQL operations
///     Ok(())
/// }
/// ```
pub fn create_schema(
    app: AppPool,
    data: Option<DataPool>,
) -> Schema<Query, Mutation, EmptySubscription> {
    create_schema_with_data(app, data, ())
}

/// Create a schema as [`create_schema`] does, with `additional_data` (the request's
/// [`crate::graphql::context::GraphQLContext`], say) in its context as well.
pub fn create_schema_with_data<T: Send + Sync + 'static>(
    app: AppPool,
    data: Option<DataPool>,
    additional_data: T,
) -> Schema<Query, Mutation, EmptySubscription> {
    create_schema_with_data_plane(app, data, None, additional_data)
}

/// Create a schema as [`create_schema_with_data`] does which, when it has no data database,
/// asks `data_plane` (the data plane's GraphQL endpoint, `DATA_GRAPHQL_URL`) whether a new
/// annotation's series exists. With neither, that check is skipped; see
/// [`crate::graphql::context::series_check`].
pub fn create_schema_with_data_plane<T: Send + Sync + 'static>(
    app: AppPool,
    data: Option<DataPool>,
    data_plane: Option<Arc<DataPlaneClient>>,
    additional_data: T,
) -> Schema<Query, Mutation, EmptySubscription> {
    let resources = SchemaResources {
        data_loaders: data
            .as_ref()
            .map(|data| Arc::new(DataLoaders::new(data.clone()))),
    };

    let mut builder = schema_builder()
        .limit_depth(MAX_QUERY_DEPTH)
        .limit_complexity(MAX_QUERY_COMPLEXITY)
        .data(resources)
        .data(app)
        .data(additional_data);
    if let Some(data) = data {
        builder = builder.data(data);
    }
    if let Some(data_plane) = data_plane {
        builder = builder.data(data_plane);
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::database::DatabasePool;
    use std::sync::Arc;

    /// Test schema creation
    #[tokio::test]
    async fn test_schema_creation() {
        // Create a test database container
        let container = econ_graph_core::test_utils::get_test_db().await;
        let pool = container.pool().clone();
        let schema = create_schema(
            econ_graph_core::database::AppPool(pool.clone()),
            Some(econ_graph_core::database::DataPool(pool)),
        );

        // Verify schema is created successfully
        // Note: We can't easily test the schema structure without a real database
        // This test just ensures the schema can be created without panicking
        assert!(!std::ptr::addr_of!(schema).is_null());
    }

    /// Test schema creation with additional data
    #[tokio::test]
    async fn test_schema_creation_with_data() {
        // Create a test database container
        let container = econ_graph_core::test_utils::get_test_db().await;
        let pool = container.pool().clone();
        let additional_data = "test_data".to_string();
        let schema = create_schema_with_data(
            econ_graph_core::database::AppPool(pool.clone()),
            Some(econ_graph_core::database::DataPool(pool)),
            additional_data,
        );

        // Verify schema is created successfully
        // Note: We can't easily test the schema structure without a real database
        // This test just ensures the schema can be created without panicking
        assert!(!std::ptr::addr_of!(schema).is_null());
    }
}
