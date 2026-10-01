//! DB-backed tests for `seriesByExternalId` and `EconomicSeries.latestObservation`.
//!
//! They need `DATABASE_URL` and are skipped without it. Each test creates its own data
//! source (unique name) so tests can share the database and run in parallel.

use crate::graphql::schema::create_schema;
use crate::imports::*;
use crate::types::LatestObservationType;
use chrono::NaiveDate;
use econ_graph_core::models::{
    DataPoint, EconomicSeries, NewDataPoint, NewDataSource, NewEconomicSeries,
};
use serde_json::{json, Value};
use std::str::FromStr;

// data_points.value is NUMERIC with scale 6, so values come back with six decimals.

async fn db() -> Option<DatabasePool> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed latestObservation test");
        return None;
    };
    crate::graphql::test_db::migrate_once(&url).await;
    Some(
        econ_graph_core::database::create_pool(&url)
            .await
            .expect("pool"),
    )
}

/// A fresh data source with a unique name, returned with that name.
async fn source(pool: &DatabasePool) -> (Uuid, String) {
    let name = format!("Test source {}", Uuid::new_v4());
    let created = DataSource::create(
        pool,
        NewDataSource {
            name: name.clone(),
            base_url: "https://example.test".into(),
            rate_limit_per_minute: 60,
            ..NewDataSource::default()
        },
    )
    .await
    .expect("create data source");
    (created.id, name)
}

async fn series(pool: &DatabasePool, source_id: Uuid, external_id: &str) -> EconomicSeries {
    let dataset_id =
        econ_graph_core::test_utils::test_dataset_id(&mut pool.get().await.unwrap(), source_id)
            .await;
    EconomicSeries::create(
        pool,
        &NewEconomicSeries {
            source_id,
            dataset_id,
            external_id: external_id.into(),
            title: format!("Series {external_id}"),
            ..Default::default()
        },
    )
    .await
    .expect("create series")
}

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

async fn point(
    pool: &DatabasePool,
    series_id: Uuid,
    date: &str,
    value: Option<&str>,
    revision_date: &str,
    is_original_release: bool,
) {
    DataPoint::create(
        pool,
        &NewDataPoint {
            series_id,
            date: day(date),
            value: value.map(|v| BigDecimal::from_str(v).unwrap()),
            revision_date: day(revision_date),
            is_original_release,
        },
    )
    .await
    .expect("create data point");
}

async fn run(pool: &DatabasePool, query: String) -> Value {
    let resp = create_schema(pool.clone()).execute(query).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    resp.data.into_json().unwrap()
}

fn latest_query(source_name: &str, external_id: &str) -> String {
    format!(
        r#"{{ seriesByExternalId(sourceName: {}, externalId: {}) {{
            externalId latestObservation {{ date value revisionDate }} }} }}"#,
        json!(source_name),
        json!(external_id)
    )
}

#[tokio::test]
async fn test_series_by_external_id_finds_series_within_its_source() {
    let Some(pool) = db().await else { return };
    let (source_a, name_a) = source(&pool).await;
    let (source_b, name_b) = source(&pool).await;
    // Same external id under two sources: the source name decides which one comes back.
    let a = series(&pool, source_a, "GDP").await;
    let b = series(&pool, source_b, "GDP").await;

    for (name, source_id, expected) in [(&name_a, source_a, &a), (&name_b, source_b, &b)] {
        let data = run(
            &pool,
            format!(
                r#"{{ seriesByExternalId(sourceName: {}, externalId: "GDP") {{ id sourceId externalId }} }}"#,
                json!(name)
            ),
        )
        .await;
        assert_eq!(
            data["seriesByExternalId"],
            json!({ "id": expected.id.to_string(), "sourceId": source_id.to_string(), "externalId": "GDP" })
        );
    }
}

#[tokio::test]
async fn test_series_by_external_id_unknown_returns_null() {
    let Some(pool) = db().await else { return };
    let (source_id, name) = source(&pool).await;
    series(&pool, source_id, "UNRATE").await;

    for (source_name, external_id) in [
        (name.as_str(), "NOPE"),
        ("No such source", "UNRATE"),
        // externalId matching is case-sensitive.
        (name.as_str(), "unrate"),
    ] {
        let data = run(&pool, latest_query(source_name, external_id)).await;
        assert_eq!(
            data["seriesByExternalId"],
            Value::Null,
            "{source_name}/{external_id}"
        );
    }
}

#[tokio::test]
async fn test_latest_observation_takes_newest_date_then_newest_revision() {
    let Some(pool) = db().await else { return };
    let (source_id, name) = source(&pool).await;
    let s = series(&pool, source_id, "CPI").await;
    // An older date with a later revision must not win over the newest date.
    point(&pool, s.id, "2026-06-01", Some("99.0"), "2026-09-20", false).await;
    point(&pool, s.id, "2026-07-01", Some("100.0"), "2026-08-10", true).await;
    point(
        &pool,
        s.id,
        "2026-07-01",
        Some("101.5"),
        "2026-09-10",
        false,
    )
    .await;
    point(
        &pool,
        s.id,
        "2026-07-01",
        Some("100.7"),
        "2026-08-25",
        false,
    )
    .await;

    let data = run(&pool, latest_query(&name, "CPI")).await;
    assert_eq!(
        data["seriesByExternalId"]["latestObservation"],
        json!({ "date": "2026-07-01", "value": "101.500000", "revisionDate": "2026-09-10" })
    );
}

#[tokio::test]
async fn test_latest_observation_same_day_revision_beats_original() {
    let Some(pool) = db().await else { return };
    let (source_id, name) = source(&pool).await;
    let s = series(&pool, source_id, "PAYEMS").await;
    point(&pool, s.id, "2026-08-01", Some("1.0"), "2026-09-05", true).await;
    point(&pool, s.id, "2026-08-01", Some("2.0"), "2026-09-05", false).await;

    let data = run(&pool, latest_query(&name, "PAYEMS")).await;
    assert_eq!(
        data["seriesByExternalId"]["latestObservation"]["value"],
        json!("2.000000")
    );
}

#[tokio::test]
async fn test_latest_observation_empty_series_returns_null() {
    let Some(pool) = db().await else { return };
    let (source_id, name) = source(&pool).await;
    series(&pool, source_id, "EMPTY").await;

    let data = run(&pool, latest_query(&name, "EMPTY")).await;
    assert_eq!(
        data["seriesByExternalId"],
        json!({ "externalId": "EMPTY", "latestObservation": null })
    );
}

#[tokio::test]
async fn test_latest_observation_keeps_null_value_of_newest_date() {
    let Some(pool) = db().await else { return };
    let (source_id, name) = source(&pool).await;
    let s = series(&pool, source_id, "GAPPY").await;
    point(&pool, s.id, "2026-07-01", Some("5.0"), "2026-08-01", true).await;
    point(&pool, s.id, "2026-08-01", None, "2026-09-01", true).await;

    let data = run(&pool, latest_query(&name, "GAPPY")).await;
    assert_eq!(
        data["seriesByExternalId"]["latestObservation"],
        json!({ "date": "2026-08-01", "value": null, "revisionDate": "2026-09-01" })
    );
}

#[tokio::test]
async fn test_latest_observation_batches_across_series_in_a_list() {
    let Some(pool) = db().await else { return };
    let (source_id, _name) = source(&pool).await;
    let with_data = series(&pool, source_id, "A").await;
    let empty = series(&pool, source_id, "B").await;
    point(
        &pool,
        with_data.id,
        "2026-01-01",
        Some("1"),
        "2026-02-01",
        true,
    )
    .await;
    point(
        &pool,
        with_data.id,
        "2026-02-01",
        Some("2"),
        "2026-03-01",
        true,
    )
    .await;

    // The loader answers every key in one batch, including series with no rows.
    let loaders = crate::graphql::dataloaders::DataLoaders::new(pool.clone());
    let got = loaders
        .latest_observation_loader
        .load_many(vec![with_data.id, empty.id])
        .await;
    assert_eq!(got.len(), 2);
    assert_eq!(got[&with_data.id].as_ref().unwrap().date, day("2026-02-01"));
    assert_eq!(got[&empty.id], None);
}

/// A batcher whose query failed: it answers no key.
struct FailingBatcher;

impl dataloader::BatchFn<Uuid, Option<LatestObservationType>> for FailingBatcher {
    async fn load(
        &mut self,
        _keys: &[Uuid],
    ) -> std::collections::HashMap<Uuid, Option<LatestObservationType>> {
        std::collections::HashMap::new()
    }
}

#[tokio::test]
async fn test_failed_batch_is_an_error_not_null() {
    // LatestObservationBatcher returns an empty map on a DB error; `try_load` must turn that
    // into an error so the field reports a failure instead of "no data".
    let loader = dataloader::non_cached::Loader::new(FailingBatcher);
    assert!(loader.try_load(Uuid::new_v4()).await.is_err());
}
