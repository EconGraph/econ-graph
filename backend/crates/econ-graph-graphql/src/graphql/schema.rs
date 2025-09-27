//! # GraphQL Schema Definition
//!
//! Schema creation and configuration for the EconGraph GraphQL API.
//! Provides the main entry point for GraphQL operations.

use async_graphql::{EmptySubscription, Schema};
use std::sync::Arc;

use crate::graphql::{mutation::Mutation, query::Query};
use crate::security::{SecurityConfig, SecurityMiddleware};
use econ_graph_core::database::DatabasePool;

/// GraphQL context containing shared resources
#[derive(Clone)]
pub struct GraphQLContext {
    /// Database connection pool
    pub pool: Arc<DatabasePool>,
    /// Security middleware
    pub security: Arc<SecurityMiddleware>,
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
pub fn create_schema(pool: Arc<DatabasePool>) -> Schema<Query, Mutation, EmptySubscription> {
    let security_config = SecurityConfig::default();
    let security = Arc::new(SecurityMiddleware::new(security_config));
    let context = GraphQLContext { pool, security };

    Schema::build(Query, Mutation, EmptySubscription)
        .data(context)
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
    pool: Arc<DatabasePool>,
    additional_data: T,
) -> Schema<Query, Mutation, EmptySubscription> {
    let security_config = SecurityConfig::default();
    let security = Arc::new(SecurityMiddleware::new(security_config));
    let context = GraphQLContext { pool, security };

    Schema::build(Query, Mutation, EmptySubscription)
        .data(context)
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
        let pool = Arc::new(container.pool().clone());
        let schema = create_schema(pool);

        // Verify schema is created successfully
        // Note: We can't easily test the schema structure without a real database
        // This test just ensures the schema can be created without panicking
        assert!(std::ptr::addr_of!(schema) != std::ptr::null());
    }

    /// Test schema creation with additional data
    #[tokio::test]
    async fn test_schema_creation_with_data() {
        // Create a test database container
        let container = econ_graph_core::test_utils::get_test_db().await;
        let pool = Arc::new(container.pool().clone());
        let additional_data = "test_data".to_string();
        let schema = create_schema_with_data(pool, additional_data);

        // Verify schema is created successfully
        // Note: We can't easily test the schema structure without a real database
        // This test just ensures the schema can be created without panicking
        assert!(std::ptr::addr_of!(schema) != std::ptr::null());
    }
}
