use crate::database::Database;
use async_graphql::{EmptySubscription, Schema};

pub mod mutation;
pub mod query;

pub use mutation::Mutation;
pub use query::Query;

pub async fn create_schema(
    database: Database,
) -> anyhow::Result<Schema<Query, Mutation, EmptySubscription>> {
    let schema = Schema::new(Query, Mutation, EmptySubscription);
    Ok(schema)
}
