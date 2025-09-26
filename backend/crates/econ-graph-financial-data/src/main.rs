use anyhow::Result;
use tracing::info;

mod crawler;
mod database;
mod graphql;
mod models;

use crate::database::Database;
use crate::graphql::create_schema;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting EconGraph Financial Data Service");

    // Initialize database
    let database = Database::new().await?;
    info!("Database initialized successfully");

    // Create GraphQL schema
    let schema = create_schema(database.clone()).await?;
    info!("GraphQL schema created successfully");

    // Start the server
    start_server(schema, database).await?;

    Ok(())
}

async fn start_server(
    schema: async_graphql::Schema<
        graphql::Query,
        graphql::Mutation,
        async_graphql::EmptySubscription,
    >,
    database: Database,
) -> Result<()> {
    use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
    use axum::{extract::State, response::Html, routing::post, Router};

    async fn graphql_handler(
        State((schema, database)): State<(
            async_graphql::Schema<
                graphql::Query,
                graphql::Mutation,
                async_graphql::EmptySubscription,
            >,
            Database,
        )>,
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

    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/", axum::routing::get(graphql_playground))
        .with_state((schema, database));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    info!("Financial Data Service listening on http://0.0.0.0:3001");
    info!("GraphQL Playground available at http://0.0.0.0:3001");

    axum::serve(listener, app).await?;

    Ok(())
}
