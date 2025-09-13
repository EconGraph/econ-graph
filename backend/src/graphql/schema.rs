use async_graphql::*;
use std::sync::Arc;

use super::{context::GraphQLContext, mutation::Mutation, query::Query};

/// Create the GraphQL schema with query and mutation roots
pub fn create_schema() -> Schema<Query, Mutation, EmptySubscription> {
    Schema::build(Query, Mutation, EmptySubscription).finish()
}

/// Create the schema with data context
pub fn create_schema_with_data(
    db_pool: crate::database::DatabasePool,
) -> Schema<Query, Mutation, EmptySubscription> {
    // Pool is already cloneable
    let data_loaders = crate::graphql::dataloaders::DataLoaders::new(db_pool.clone());

    Schema::build(Query, Mutation, EmptySubscription)
        .data(db_pool.clone())
        .data(data_loaders)
        .finish()
}

/// Create the schema with authentication context
pub fn create_schema_with_auth(
    db_pool: crate::database::DatabasePool,
    auth_context: Arc<GraphQLContext>,
) -> Schema<Query, Mutation, EmptySubscription> {
    let data_loaders = crate::graphql::dataloaders::DataLoaders::new(db_pool.clone());

    Schema::build(Query, Mutation, EmptySubscription)
        .data(db_pool)
        .data(data_loaders)
        .data(auth_context)
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_schema_creation() {
        // REQUIREMENT: GraphQL API should be properly structured with Query and Mutation roots
        // PURPOSE: Verify that the GraphQL schema is correctly constructed and introspectable
        // This ensures GraphQL clients can discover available operations and types

        let schema = create_schema();

        // Test introspection query - required for GraphQL tooling and IDE support
        let query = "
            query IntrospectionQuery {
                __schema {
                    queryType {
                        name
                    }
                    mutationType {
                        name
                    }
                }
            }
        ";

        let result = schema.execute(query).await;

        // Verify introspection query executes without errors - required for GraphQL compliance
        assert!(
            result.errors.is_empty(),
            "Introspection query should execute without errors"
        );

        let data = result.data.into_json().unwrap();
        // Verify Query root type is properly configured - required for read operations
        assert_eq!(
            data["__schema"]["queryType"]["name"], "Query",
            "Schema should have Query root type"
        );
        // Verify Mutation root type is properly configured - required for write operations
        assert_eq!(
            data["__schema"]["mutationType"]["name"], "Mutation",
            "Schema should have Mutation root type"
        );
    }
}
