//! ECO-410: a real Postgres error raised inside a resolver reaches the client as a generic
//! message, and its detail is logged.
//!
//! The pool connects with `search_path` pointing at a schema that doesn't exist, so every query
//! fails in Postgres with `relation "..." does not exist`, without touching the shared test
//! database's tables.

use crate::graphql::context::GraphQLContext;
use crate::graphql::errors::{capture_logs, SanitizeErrors, INTERNAL_ERROR_MESSAGE};
use crate::graphql::global_analysis::GlobalAnalysisQuery;
use crate::graphql::schema::create_schema_with_data;
use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use econ_graph_core::DatabasePool;
use std::sync::Arc;

/// A pool whose every query fails in Postgres, or `None` (with a skip notice) when
/// `DATABASE_URL` isn't set.
async fn broken_pool() -> Option<DatabasePool> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed error sanitizing tests");
        return None;
    };
    let separator = if url.contains('?') { '&' } else { '?' };
    let url = format!("{url}{separator}options=-c%20search_path%3Deco410_no_such_schema");
    Some(
        econ_graph_core::database::create_pool(&url)
            .await
            .expect("pool"),
    )
}

/// Asserts `response` failed with only the generic message, and that `logs` hold Postgres's
/// own text for it.
fn assert_hidden_and_logged(
    response: &async_graphql::Response,
    logs: &std::sync::Mutex<Vec<u8>>,
    relation: &str,
) {
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    let message = &response.errors[0].message;
    assert_eq!(message, INTERNAL_ERROR_MESSAGE);
    let logs = String::from_utf8(logs.lock().unwrap().clone()).unwrap();
    assert!(
        logs.contains(&format!("relation \"{relation}\" does not exist")),
        "the log should keep Postgres's message: {logs}"
    );
}

/// `searchSeries`: the search service's query error used to be wrapped as an external API
/// error, which the sanitizer passed through as "Query execution error: <pg text>".
#[tokio::test]
async fn search_series_db_error_is_generic_and_logged() {
    let Some(pool) = broken_pool().await else {
        return;
    };
    let schema =
        create_schema_with_data(pool, Arc::new(GraphQLContext::for_test_user(None, false)));
    let (_guard, logs) = capture_logs();
    let response = schema
        .execute(r#"{ searchSeries(query: "unemployment") { totalCount } }"#)
        .await;
    assert_hidden_and_logged(&response, &logs, "economic_series");
}

/// The global analysis resolvers used to format the service error into their own message,
/// which dropped its source and so slipped past the sanitizer.
#[tokio::test]
async fn global_analysis_db_errors_are_generic_and_logged() {
    let Some(pool) = broken_pool().await else {
        return;
    };
    let schema = Schema::build(GlobalAnalysisQuery, EmptyMutation, EmptySubscription)
        .data(pool)
        .extension(SanitizeErrors)
        .finish();
    for (query, relation) in [
        (
            "{ countriesWithEconomicData { country { id } } }",
            "countries",
        ),
        (
            "{ globalEventsWithImpacts { event { id } } }",
            "global_economic_events",
        ),
    ] {
        let (_guard, logs) = capture_logs();
        let response = schema.execute(query).await;
        assert_hidden_and_logged(&response, &logs, relation);
    }
}
