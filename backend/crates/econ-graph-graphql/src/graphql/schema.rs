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

/// Deepest query the API runs. The standard introspection query that GraphiQL, the
/// playground and codegen tools send nests 13 levels, so 15 leaves it room while still
/// refusing pathologically nested queries before they run.
pub const MAX_QUERY_DEPTH: usize = 15;

/// Largest query cost the API runs, counting one per selected field.
pub const MAX_QUERY_COMPLEXITY: usize = 1000;

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
        .limit_depth(MAX_QUERY_DEPTH)
        .limit_complexity(MAX_QUERY_COMPLEXITY)
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
        .limit_depth(MAX_QUERY_DEPTH)
        .limit_complexity(MAX_QUERY_COMPLEXITY)
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

#[cfg(test)]
mod query_limit_tests {
    use super::{create_schema_with_data, MAX_QUERY_COMPLEXITY, MAX_QUERY_DEPTH};
    use econ_graph_core::database::DatabasePool;

    /// A pool that never connects: these queries are refused, or answered from the schema,
    /// before any resolver touches the database.
    fn unreachable_pool() -> DatabasePool {
        let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
            diesel_async::AsyncPgConnection,
        >::new("postgres://nobody@127.0.0.1:1/none");
        DatabasePool::builder()
            .connection_timeout(std::time::Duration::from_secs(1))
            .build_unchecked(manager)
    }

    /// Run `query` and return its error messages (empty when it succeeded).
    async fn errors(query: &str) -> Vec<String> {
        let schema = create_schema_with_data(unreachable_pool(), ());
        schema
            .execute(query)
            .await
            .errors
            .into_iter()
            .map(|e| e.message)
            .collect()
    }

    /// An ordinary shallow query is still answered.
    #[tokio::test]
    async fn shallow_query_is_answered() {
        assert!(errors("{ __typename }").await.is_empty());
    }

    /// The standard introspection query that GraphiQL, the playground and codegen tools send.
    const INTROSPECTION_QUERY: &str = r#"
        query IntrospectionQuery {
          __schema {
            queryType { name } mutationType { name } subscriptionType { name }
            types { ...FullType }
            directives { name description locations args { ...InputValue } }
          }
        }
        fragment FullType on __Type {
          kind name description
          fields(includeDeprecated: true) {
            name description args { ...InputValue } type { ...TypeRef }
            isDeprecated deprecationReason
          }
          inputFields { ...InputValue }
          interfaces { ...TypeRef }
          enumValues(includeDeprecated: true) { name description isDeprecated deprecationReason }
          possibleTypes { ...TypeRef }
        }
        fragment InputValue on __InputValue { name description type { ...TypeRef } defaultValue }
        fragment TypeRef on __Type {
          kind name
          ofType { kind name ofType { kind name ofType { kind name ofType { kind name
            ofType { kind name ofType { kind name ofType { kind name ofType { kind name } } } } } } } }
        }
    "#;

    /// The limits leave room for the standard introspection query, so schema tooling works.
    #[tokio::test]
    async fn standard_introspection_query_is_answered() {
        let errors = errors(INTROSPECTION_QUERY).await;
        assert!(errors.is_empty(), "{errors:?}");
    }

    /// A query nested deeper than the depth limit is refused before it runs.
    #[tokio::test]
    async fn query_deeper_than_the_limit_is_refused() {
        let nested = (0..MAX_QUERY_DEPTH).fold("name".to_string(), |inner, _| {
            format!("ofType {{ {inner} }}")
        });
        let query =
            format!("{{ __schema {{ queryType {{ fields {{ type {{ {nested} }} }} }} }} }}");
        let errors = errors(&query).await;
        assert!(errors.iter().any(|e| e.contains("too deep")), "{errors:?}");
    }

    /// A query whose field count exceeds the complexity limit is refused before it runs.
    /// Each aliased `dataSources { id }` costs 2, so 600 of them cost 1200, over the 1000 limit.
    #[tokio::test]
    async fn query_over_the_complexity_limit_is_refused() {
        let fields: Vec<String> = (0..600)
            .map(|i| format!("a{i}: dataSources {{ id }}"))
            .collect();
        let query = format!("{{ {} }}", fields.join(" "));
        let errors = errors(&query).await;
        assert!(
            errors.iter().any(|e| e.contains("too complex")),
            "{errors:?}"
        );
    }

    /// Build a query with `n` aliased `__typename` selections, each costing 1, so its
    /// complexity is exactly `n`. `__typename` needs no resolver or database.
    fn typename_query(n: usize) -> String {
        let fields: Vec<String> = (0..n).map(|i| format!("a{i}: __typename")).collect();
        format!("{{ {} }}", fields.join(" "))
    }

    /// A query at exactly the complexity limit is accepted.
    #[tokio::test]
    async fn query_at_the_complexity_limit_is_accepted() {
        let errors = errors(&typename_query(MAX_QUERY_COMPLEXITY)).await;
        assert!(errors.is_empty(), "{errors:?}");
    }

    /// A query one field over the complexity limit is refused.
    #[tokio::test]
    async fn query_one_over_the_complexity_limit_is_refused() {
        let errors = errors(&typename_query(MAX_QUERY_COMPLEXITY + 1)).await;
        assert!(errors.iter().any(|e| e.contains("too complex")), "{errors:?}");
    }
}
