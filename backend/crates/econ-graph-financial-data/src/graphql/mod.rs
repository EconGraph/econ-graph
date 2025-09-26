use crate::database::Database;
use async_graphql::{EmptySubscription, Schema};

pub mod mutation;
pub mod query;

pub use mutation::Mutation;
pub use query::Query;

pub async fn create_schema(
    database: Database,
) -> anyhow::Result<Schema<Query, Mutation, EmptySubscription>> {
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(database)
        .finish();
    Ok(schema)
}
