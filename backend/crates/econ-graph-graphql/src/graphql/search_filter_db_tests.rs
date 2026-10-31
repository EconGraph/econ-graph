//! DB-backed tests for `searchSeries`'s `totalCount` (ECO-508) and for rejecting a malformed
//! source id on `searchSeries`, `seriesList` and `datasets` rather than dropping the filter
//! (ECO-509).
//!
//! They need `DATABASE_URL` naming a test database. Every test seeds its own sources, with a
//! title word no other test uses, so rows from other tests can't change the counts. Rows an
//! earlier run left behind are deleted first.

use crate::graphql::schema::create_schema;
use crate::graphql::test_db::migrate_once;
use async_graphql::{Request, Response};
use econ_graph_core::database::{AppPool, DataPool, DatabasePool};
use econ_graph_core::models::{DataSource, EconomicSeries, NewDataSource, NewEconomicSeries};
use econ_graph_core::test_utils::test_dataset_id;
use serde_json::Value;
use serial_test::serial;
use std::collections::HashSet;
use uuid::Uuid;

/// Prefix of the names of the data sources these tests create.
const SOURCE_PREFIX: &str = "search-filter-test-";

/// A pool on the migrated test database, without any of these tests' earlier rows. The pool
/// is the test's own, since a pool can't outlive the runtime of the test that made it.
async fn pool() -> DatabasePool {
    use diesel_async::RunQueryDsl;

    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL names a test database");
    migrate_once(&url).await;
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    let mut conn = pool.get().await.expect("connection");
    for table in ["economic_series", "datasets"] {
        diesel::sql_query(format!(
            "DELETE FROM {table} WHERE source_id IN
                 (SELECT id FROM data_sources WHERE starts_with(name, $1))"
        ))
        .bind::<diesel::sql_types::Text, _>(SOURCE_PREFIX)
        .execute(&mut conn)
        .await
        .expect("deleting earlier rows");
    }
    diesel::sql_query("DELETE FROM data_sources WHERE starts_with(name, $1)")
        .bind::<diesel::sql_types::Text, _>(SOURCE_PREFIX)
        .execute(&mut conn)
        .await
        .expect("deleting earlier sources");
    drop(conn);
    pool
}

/// Runs `query` against a schema on `pool` (with the production error sanitizing).
async fn run(pool: &DatabasePool, query: &str) -> Response {
    let schema = create_schema(AppPool(pool.clone()), Some(DataPool(pool.clone())));
    schema.execute(Request::new(query)).await
}

/// Runs `query`, asserts it succeeded, and returns its data.
async fn data(pool: &DatabasePool, query: &str) -> Value {
    let response = run(pool, query).await;
    assert!(response.errors.is_empty(), "{query}: {:?}", response.errors);
    response.data.into_json().unwrap()
}

/// A new data source.
async fn new_source(pool: &DatabasePool) -> DataSource {
    DataSource::create(
        pool,
        NewDataSource {
            name: format!("{SOURCE_PREFIX}{}", Uuid::new_v4()),
            description: None,
            base_url: "https://example.test".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 60,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: None,
            api_key_name: None,
        },
    )
    .await
    .expect("create data source")
}

/// Creates `count` series under `source` titled `"{word} {frequency} {i}"`, with data (an end
/// date) when `has_data`.
async fn add_series(
    pool: &DatabasePool,
    source: &DataSource,
    word: &str,
    frequency: &str,
    count: usize,
    has_data: bool,
) {
    let dataset_id = test_dataset_id(&mut pool.get().await.expect("connection"), source.id).await;
    for i in 0..count {
        let title = format!("{word} {frequency} {i:02}");
        EconomicSeries::create(
            pool,
            &NewEconomicSeries {
                source_id: source.id,
                external_id: format!("{title} {}", Uuid::new_v4()),
                title,
                description: None,
                units: None,
                frequency: frequency.to_string(),
                seasonal_adjustment: None,
                start_date: chrono::NaiveDate::from_ymd_opt(2020, 1, 1),
                end_date: has_data.then(|| chrono::NaiveDate::from_ymd_opt(2020, 12, 1).unwrap()),
                is_active: true,
                first_discovered_at: None,
                last_crawled_at: None,
                first_missing_date: None,
                crawl_status: None,
                crawl_error_message: None,
                dataset_id,
                dimensions: Default::default(),
                default_measure: None,
                units_source: None,
            },
        )
        .await
        .expect("create series");
    }
}

/// `searchSeries(query: word, <args>)`'s total count and the ids on its page.
async fn search(pool: &DatabasePool, word: &str, args: &str) -> (i64, Vec<String>) {
    let data = data(
        pool,
        &format!(r#"{{ searchSeries(query: "{word}"{args}) {{ totalCount series {{ id }} }} }}"#),
    )
    .await;
    let result = &data["searchSeries"];
    let ids = result["series"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap().to_string())
        .collect();
    (result["totalCount"].as_i64().unwrap(), ids)
}

/// `totalCount` is every matching series, whatever the page size or offset, and counts with
/// the same filters and rules (no series without data) as the page.
#[tokio::test]
#[serial]
async fn search_total_count_counts_every_match() {
    let pool = pool().await;
    let word = "Qwzvbindex";
    let monthly = new_source(&pool).await;
    let quarterly = new_source(&pool).await;
    add_series(&pool, &monthly, word, "Monthly", 7, true).await;
    // Never shown, so never counted.
    add_series(&pool, &monthly, word, "Monthly", 2, false).await;
    add_series(&pool, &quarterly, word, "Quarterly", 3, true).await;

    for (args, expected_len) in [
        (", first: 1", 1),
        (", first: 3", 3),
        (", first: 100", 10),
        (r#", first: 3, after: "4""#, 3),
        (r#", first: 3, after: "9""#, 1),
        // Past the end: an empty page, still with the total.
        (r#", first: 3, after: "50""#, 0),
    ] {
        let (total, ids) = search(&pool, word, args).await;
        assert_eq!(total, 10, "{args}");
        assert_eq!(ids.len(), expected_len, "{args}");
    }

    // Paging through by the total reaches every match exactly once.
    let mut seen = HashSet::new();
    let mut offset = 0;
    loop {
        let (total, ids) = search(&pool, word, &format!(r#", first: 3, after: "{offset}""#)).await;
        assert_eq!(total, 10);
        offset += ids.len();
        seen.extend(ids);
        if offset as i64 >= total {
            break;
        }
    }
    assert_eq!(seen.len(), 10);

    // Filters narrow the total as they narrow the rows.
    let by_source = format!(r#", source: "{}", first: 2"#, monthly.id);
    let (total, ids) = search(&pool, word, &by_source).await;
    assert_eq!((total, ids.len()), (7, 2));
    let (total, ids) = search(&pool, word, ", frequency: QUARTERLY, first: 2").await;
    assert_eq!((total, ids.len()), (3, 2));
    let both = format!(r#", source: "{}", frequency: QUARTERLY"#, monthly.id);
    assert_eq!(search(&pool, word, &both).await, (0, vec![]));

    // No matches at all.
    assert_eq!(search(&pool, "Xjqvnomatchword", "").await, (0, vec![]));
}

/// A source id that isn't a UUID (or is empty) is a `BAD_USER_INPUT` error naming the
/// argument, on `searchSeries`, `seriesList` and `datasets`, and never a result set without
/// the filter. So is a malformed `dataset(id:)`.
#[tokio::test]
#[serial]
async fn malformed_source_id_is_rejected() {
    let pool = pool().await;
    let word = "Plokvquintile";
    let source = new_source(&pool).await;
    add_series(&pool, &source, word, "Monthly", 2, true).await;

    for (query, argument) in [
        (
            format!(
                r#"{{ searchSeries(query: "{word}", source: "not-a-uuid") {{ totalCount }} }}"#
            ),
            "`source`",
        ),
        (
            format!(r#"{{ searchSeries(query: "{word}", source: "") {{ totalCount }} }}"#),
            "`source`",
        ),
        (
            format!(
                r#"{{ searchSeries(query: "{word}", source: "not-a-uuid", frequency: MONTHLY) {{ totalCount }} }}"#
            ),
            "`source`",
        ),
        (
            r#"{ seriesList(filter: { sourceId: "not-a-uuid" }) { totalCount } }"#.to_string(),
            "`filter.sourceId`",
        ),
        (
            r#"{ seriesList(filter: { sourceId: "", frequency: MONTHLY }) { totalCount } }"#
                .to_string(),
            "`filter.sourceId`",
        ),
        (
            r#"{ datasets(sourceId: "not-a-uuid") { id } }"#.to_string(),
            "`sourceId`",
        ),
        (
            r#"{ datasets(sourceId: "") { id } }"#.to_string(),
            "`sourceId`",
        ),
        (
            r#"{ dataset(id: "not-a-uuid") { id } }"#.to_string(),
            "`id`",
        ),
    ] {
        let response = run(&pool, &query).await;
        assert_eq!(response.errors.len(), 1, "{query}: {:?}", response.errors);
        let error = &response.errors[0];
        assert!(
            error.message.contains(argument) && error.message.contains("not a valid id"),
            "{query}: {}",
            error.message
        );
        let code = error
            .extensions
            .as_ref()
            .and_then(|e| e.get("code").cloned());
        assert_eq!(code, Some("BAD_USER_INPUT".into()), "{query}");
        assert_eq!(response.data.into_json().unwrap(), Value::Null, "{query}");
    }
}

/// A well-formed source id filters both fields: an existing source to its own series, and an
/// unknown one to nothing.
#[tokio::test]
#[serial]
async fn valid_source_id_filters() {
    let pool = pool().await;
    let word = "Gruvnquotient";
    let source = new_source(&pool).await;
    let other = new_source(&pool).await;
    add_series(&pool, &source, word, "Monthly", 2, true).await;
    add_series(&pool, &other, word, "Monthly", 3, true).await;

    let (total, ids) = search(&pool, word, &format!(r#", source: "{}""#, source.id)).await;
    assert_eq!((total, ids.len()), (2, 2));
    let unknown = Uuid::new_v4();
    let (total, ids) = search(&pool, word, &format!(r#", source: "{unknown}""#)).await;
    assert_eq!((total, ids.len()), (0, 0));
    let (total, _) = search(&pool, word, "").await;
    assert_eq!(total, 5, "no source: every source's matches");

    let list = |id: String| {
        format!(
            r#"{{ seriesList(filter: {{ sourceId: "{id}" }}) {{ totalCount nodes {{ id }} }} }}"#
        )
    };
    let listed = data(&pool, &list(source.id.to_string())).await;
    assert_eq!(listed["seriesList"]["totalCount"], 2);
    assert_eq!(listed["seriesList"]["nodes"].as_array().unwrap().len(), 2);
    let listed = data(&pool, &list(unknown.to_string())).await;
    assert_eq!(listed["seriesList"]["totalCount"], 0);
    assert_eq!(listed["seriesList"]["nodes"].as_array().unwrap().len(), 0);
}

/// A source id narrows along with the other filters, never instead of them, on both fields;
/// with no source id, every source's matches are counted.
#[tokio::test]
#[serial]
async fn source_id_combines_with_other_filters() {
    let pool = pool().await;
    let word = "Vexmordial";
    let source = new_source(&pool).await;
    let other = new_source(&pool).await;
    add_series(&pool, &source, word, "Monthly", 2, true).await;
    add_series(&pool, &source, word, "Quarterly", 1, true).await;
    add_series(&pool, &other, word, "Monthly", 3, true).await;
    let unknown = Uuid::new_v4();

    for (args, expected) in [
        (String::new(), 6),
        (", frequency: MONTHLY".to_string(), 5),
        (format!(r#", source: "{}""#, source.id), 3),
        (
            format!(r#", source: "{}", frequency: MONTHLY"#, source.id),
            2,
        ),
        (
            format!(r#", source: "{}", frequency: QUARTERLY"#, other.id),
            0,
        ),
        (format!(r#", source: "{unknown}", frequency: MONTHLY"#), 0),
    ] {
        let (total, ids) = search(&pool, word, &args).await;
        assert_eq!(
            (total, ids.len()),
            (expected, expected as usize),
            "searchSeries{args}"
        );
    }

    for (filter, expected) in [
        (String::new(), 6),
        (", frequency: MONTHLY".to_string(), 5),
        (format!(r#", sourceId: "{}""#, source.id), 3),
        (
            format!(r#", sourceId: "{}", frequency: MONTHLY"#, source.id),
            2,
        ),
        (
            format!(r#", sourceId: "{}", frequency: QUARTERLY"#, other.id),
            0,
        ),
        (format!(r#", sourceId: "{unknown}", frequency: MONTHLY"#), 0),
    ] {
        let query = format!(
            r#"{{ seriesList(filter: {{ searchQuery: "{word}"{filter} }}) {{ totalCount nodes {{ id }} }} }}"#
        );
        let listed = data(&pool, &query).await;
        assert_eq!(listed["seriesList"]["totalCount"], expected, "{query}");
        assert_eq!(
            listed["seriesList"]["nodes"].as_array().unwrap().len(),
            expected as usize,
            "{query}"
        );
    }
}
