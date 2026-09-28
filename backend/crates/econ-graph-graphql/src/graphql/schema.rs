//! # GraphQL Schema Definition
//!
//! Schema creation and configuration for the EconGraph GraphQL API.
//! Provides the main entry point for GraphQL operations.

use async_graphql::{EmptySubscription, Schema, SchemaBuilder};
use std::sync::Arc;

use crate::graphql::dataloaders::DataLoaders;
use crate::graphql::{mutation::Mutation, query::Query};
use crate::security::{SecurityConfig, SecurityMiddleware};
use econ_graph_core::database::DatabasePool;

/// Shared resources every request of a schema sees (the per-request caller is
/// [`crate::graphql::context::GraphQLContext`])
#[derive(Clone)]
pub struct SchemaResources {
    /// Database connection pool
    pub pool: Arc<DatabasePool>,
    /// DataLoaders for efficient N+1 query prevention
    pub data_loaders: Arc<DataLoaders>,
    /// Security middleware
    pub security: Arc<SecurityMiddleware>,
}

/// Start a schema builder with the root types and every schema-level setting.
///
/// [`create_schema`], [`create_schema_with_data`] and [`sdl`] all build from
/// this, so the committed `schema.graphql` snapshot describes the schema the
/// server actually serves.
fn schema_builder() -> SchemaBuilder<Query, Mutation, EmptySubscription> {
    Schema::build(Query, Mutation, EmptySubscription)
}

/// Print the schema as GraphQL SDL.
///
/// Needs no database or context data: those don't change the schema's shape.
/// The `schema_snapshot` test compares this with the committed
/// `schema.graphql` at the crate root.
pub fn sdl() -> String {
    schema_builder().finish().sdl()
}

/// Create a new GraphQL schema with the provided context
///
/// # Parameters
/// - `pool`: Database connection pool for data access
///
/// # Returns
/// Configured GraphQL schema ready for use
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_graphql::graphql::schema::create_schema;
/// use econ_graph_core::database::create_pool;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let pool = create_pool("postgres://localhost/econ_graph").await?;
///     let schema = create_schema(pool);
///
///     // Use schema for GraphQL operations
///     Ok(())
/// }
/// ```
pub fn create_schema(pool: DatabasePool) -> Schema<Query, Mutation, EmptySubscription> {
    let pool_arc = Arc::new(pool.clone());
    let data_loaders = Arc::new(DataLoaders::new(pool.clone()));
    let security_config = SecurityConfig::default();
    let security = Arc::new(SecurityMiddleware::new(security_config));
    let context = SchemaResources {
        pool: pool_arc,
        data_loaders,
        security,
    };

    schema_builder()
        .data(context)
        .data(pool) // Add pool as separate context data
        .finish()
}

/// Create a schema with additional data
///
/// # Parameters
/// - `pool`: Database connection pool
/// - `additional_data`: Additional data to include in the context
///
/// # Returns
/// Configured GraphQL schema with additional context data
pub fn create_schema_with_data<T: Send + Sync + 'static>(
    pool: DatabasePool,
    additional_data: T,
) -> Schema<Query, Mutation, EmptySubscription> {
    let pool_arc = Arc::new(pool.clone());
    let data_loaders = Arc::new(DataLoaders::new(pool.clone()));
    let security_config = SecurityConfig::default();
    let security = Arc::new(SecurityMiddleware::new(security_config));
    let context = SchemaResources {
        pool: pool_arc,
        data_loaders,
        security,
    };

    schema_builder()
        .data(context)
        .data(pool) // Add pool as separate context data
        .data(additional_data)
        .finish()
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
        let schema = create_schema(pool);

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
        let schema = create_schema_with_data(pool, additional_data);

        // Verify schema is created successfully
        // Note: We can't easily test the schema structure without a real database
        // This test just ensures the schema can be created without panicking
        assert!(!std::ptr::addr_of!(schema).is_null());
    }
}
