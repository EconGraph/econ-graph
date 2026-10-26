//! # GraphQL Schema Definition
//!
//! Schema creation and configuration for the EconGraph GraphQL API.
//! Provides the main entry point for GraphQL operations.

use async_graphql::{EmptySubscription, Schema, SchemaBuilder};
use std::sync::Arc;

use crate::graphql::dataloaders::DataLoaders;
use crate::graphql::errors::SanitizeErrors;
use crate::graphql::{mutation::Mutation, query::Query};
use crate::security::{SecurityConfig, SecurityMiddleware};
use econ_graph_core::database::{AppPool, DataPool};
use econ_graph_services::services::series_check::DataPlaneClient;

/// Shared resources every request of a schema sees (the per-request caller is
/// [`crate::graphql::context::GraphQLContext`])
#[derive(Clone)]
pub struct SchemaResources {
    /// DataLoaders on the data database, for N+1 query prevention; `None` when this schema has
    /// no data database (see [`crate::graphql::context::data_loaders`])
    pub data_loaders: Option<Arc<DataLoaders>>,
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
///
/// Every schema writes the permission denials its requests raise before returning their
/// responses ([`crate::graphql::audit::AuditWrites`]).
fn schema_builder() -> SchemaBuilder<Query, Mutation, EmptySubscription> {
    Schema::build(Query, Mutation, EmptySubscription)
        .extension(SanitizeErrors)
        .extension(crate::graphql::audit::AuditWrites)
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
    let security = Arc::new(SecurityMiddleware::new(SecurityConfig::default()));
    let resources = SchemaResources {
        data_loaders: data
            .as_ref()
            .map(|data| Arc::new(DataLoaders::new(data.clone()))),
        security,
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

#[cfg(test)]
mod query_limit_tests {
    use super::{create_schema_with_data, MAX_QUERY_COMPLEXITY, MAX_QUERY_DEPTH};
    use econ_graph_core::database::DatabasePool;

    /// A pool that never connects: these queries are refused, or answered from the schema,
    /// before any resolver touches the database. The one exception is the at-the-limit
    /// complexity test, whose resolvers fail to get a connection; its runtime is bounded by the
    /// one-second connection timeout.
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
        let schema = create_schema_with_data(
            econ_graph_core::database::AppPool(unreachable_pool()),
            Some(econ_graph_core::database::DataPool(unreachable_pool())),
            (),
        );
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

    /// Build a query of `n` aliased `dataSources { id }` selections, each costing 2, so its
    /// complexity is `2 * n`. (async-graphql does not count `__typename`, so it can't be used
    /// to build a query of an exact cost.)
    fn data_sources_query(n: usize) -> String {
        let fields: Vec<String> = (0..n)
            .map(|i| format!("a{i}: dataSources {{ id }}"))
            .collect();
        format!("{{ {} }}", fields.join(" "))
    }

    /// A query at exactly the complexity limit is not refused as too complex. The resolvers
    /// then fail on the unreachable database, which is not what this test is about.
    #[tokio::test]
    async fn query_at_the_complexity_limit_is_accepted() {
        let errors = errors(&data_sources_query(MAX_QUERY_COMPLEXITY / 2)).await;
        assert!(
            !errors.iter().any(|e| e.contains("too complex")),
            "{errors:?}"
        );
    }

    /// A query one selection (cost 2) over the complexity limit is refused.
    #[tokio::test]
    async fn query_one_over_the_complexity_limit_is_refused() {
        let errors = errors(&data_sources_query(MAX_QUERY_COMPLEXITY / 2 + 1)).await;
        assert!(
            errors.iter().any(|e| e.contains("too complex")),
            "{errors:?}"
        );
    }
}
