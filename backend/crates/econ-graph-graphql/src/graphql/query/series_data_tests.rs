//! `seriesData` paging and transformations. The DB-backed tests need `DATABASE_URL` (a migrated
//! or migratable database) and are skipped when it is unset. Each creates its own data source and
//! series and deletes them afterwards.

use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use econ_graph_core::database::DatabasePool;
use serde_json::Value;
use uuid::Uuid;

use super::{apply_data_transformation, log_difference};
use crate::types::DataTransformationType;

fn dec(s: &str) -> BigDecimal {
    BigDecimal::from_str(s).unwrap()
}

fn point(date: NaiveDate, value: Option<&str>) -> econ_graph_core::models::DataPoint {
    let now = chrono::Utc::now();
    econ_graph_core::models::DataPoint {
        id: Uuid::new_v4(),
        series_id: Uuid::nil(),
        date,
        value: value.map(dec),
        revision_date: date,
        is_original_release: true,
        created_at: now,
        updated_at: now,
    }
}

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn log_difference_is_the_natural_log_of_the_ratio() {
    let diff = log_difference(&dec("110"), &dec("100")).unwrap();
    assert_eq!(diff.round(4), dec("0.0953"));
    // ln(1.1) is 0.09531..., where the old `ratio - 1` gave 0.1.
    assert!((diff - dec("0.0953101798")).abs() < dec("0.0000000001"));
}

#[test]
fn log_difference_skips_non_positive_values() {
    assert_eq!(log_difference(&dec("0"), &dec("100")), None);
    assert_eq!(log_difference(&dec("100"), &dec("0")), None);
    assert_eq!(log_difference(&dec("-5"), &dec("100")), None);
}

#[tokio::test]
async fn log_difference_transformation_compares_consecutive_points() {
    let points = vec![
        point(day(2024, 1, 1), Some("100")),
        point(day(2024, 2, 1), Some("110")),
        point(day(2024, 3, 1), Some("0")),
        point(day(2024, 4, 1), Some("50")),
        point(day(2024, 5, 1), Some("100")),
    ];
    let out = apply_data_transformation(points, DataTransformationType::LogDifference)
        .await
        .unwrap();
    let values: Vec<_> = out
        .iter()
        .map(|p| p.value.clone().map(|v| v.round(4)))
        .collect();
    assert_eq!(
        values,
        vec![None, Some(dec("0.0953")), None, None, Some(dec("0.6931"))]
    );
}

#[tokio::test]
async fn percent_change_without_any_usable_base_keeps_every_point() {
    let points = vec![
        point(day(2024, 1, 1), None),
        point(day(2024, 2, 1), None),
        point(day(2024, 3, 1), Some("0")),
    ];
    let out = apply_data_transformation(points, DataTransformationType::PercentChange)
        .await
        .unwrap();
    assert_eq!(out.len(), 3);
    assert!(out.iter().all(|p| p.value.is_none()));
}

#[tokio::test]
async fn percent_change_skips_a_null_first_point_for_the_base() {
    let points = vec![
        point(day(2024, 1, 1), None),
        point(day(2024, 2, 1), Some("100")),
        point(day(2024, 3, 1), Some("110")),
    ];
    let out = apply_data_transformation(points, DataTransformationType::PercentChange)
        .await
        .unwrap();
    let values: Vec<_> = out.iter().map(|p| p.value.clone()).collect();
    assert_eq!(values, vec![None, Some(dec("0")), Some(dec("10"))]);
}

#[tokio::test]
async fn percent_change_skips_a_zero_first_point_for_the_base() {
    let points = vec![
        point(day(2024, 1, 1), Some("0")),
        point(day(2024, 2, 1), Some("50")),
        point(day(2024, 3, 1), Some("75")),
    ];
    let out = apply_data_transformation(points, DataTransformationType::PercentChange)
        .await
        .unwrap();
    let values: Vec<_> = out.iter().map(|p| p.value.clone()).collect();
    // The zero first point still gets a value (compared against the real base, 50), unlike a
    // null point which has nothing to transform.
    assert_eq!(
        values,
        vec![Some(dec("-100")), Some(dec("0")), Some(dec("50"))]
    );
}

fn value(node: &Value) -> BigDecimal {
    dec(node["value"].as_str().unwrap())
}

// ---------------------------------------------------------------------
// DB-backed
// ---------------------------------------------------------------------

static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Fixture {
    pool: DatabasePool,
    source_id: Uuid,
    _guard: tokio::sync::MutexGuard<'static, ()>,
}

impl Fixture {
    async fn new() -> Option<Self> {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            eprintln!("DATABASE_URL not set; skipping DB-backed seriesData test");
            return None;
        };
        let guard = DB_LOCK.lock().await;
        econ_graph_core::database::run_migrations(&url)
            .await
            .expect("migrations");
        let pool = econ_graph_core::database::create_pool(&url)
            .await
            .expect("pool");
        let source_id = Uuid::new_v4();
        let fixture = Self {
            pool,
            source_id,
            _guard: guard,
        };
        fixture
            .sql(&format!(
                "INSERT INTO data_sources (id, name, description, base_url) \
                 VALUES ('{source_id}', 'seriesData test {source_id}', 'test', 'https://example.test')"
            ))
            .await;
        fixture
            .sql(&format!(
                "INSERT INTO datasets (source_id, code, name) VALUES ('{source_id}', 'test', 'Test')"
            ))
            .await;
        Some(fixture)
    }

    /// Deletes the fixture's source; its series and points cascade.
    async fn finish(self) {
        self.sql(&format!(
            "DELETE FROM data_sources WHERE id = '{}'",
            self.source_id
        ))
        .await;
    }

    async fn sql(&self, sql: &str) {
        let mut conn = self.pool.get().await.unwrap();
        diesel_async::RunQueryDsl::execute(diesel::sql_query(sql), &mut conn)
            .await
            .unwrap();
    }

    /// A series whose points come from `points_sql`, a SELECT of (date, value, revision_date,
    /// is_original_release) that may use `$series`.
    async fn series(&self, frequency: &str, points_sql: &str) -> Uuid {
        let series_id = Uuid::new_v4();
        self.sql(&format!(
            "INSERT INTO economic_series (id, source_id, external_id, title, frequency, dataset_id) \
             SELECT '{series_id}', '{0}', 'seriesdata_{series_id}', 'seriesData test', \
                 '{frequency}', id FROM datasets WHERE source_id = '{0}' AND code = 'test'",
            self.source_id
        ))
        .await;
        self.sql(&format!(
            "INSERT INTO data_points (series_id, date, value, revision_date, is_original_release) \
             SELECT '{series_id}', p.* FROM ({points_sql}) p"
        ))
        .await;
        series_id
    }

    async fn query(&self, query: &str) -> Value {
        let schema = crate::graphql::schema::create_schema(self.pool.clone());
        let resp = schema.execute(query).await;
        assert!(resp.errors.is_empty(), "{query}: {:?}", resp.errors);
        resp.data.into_json().unwrap()
    }

    /// Every page of `seriesData`, `first` points at a time, following `endCursor`.
    async fn all_pages(&self, series_id: Uuid, args: &str, first: i32) -> (Vec<Value>, Vec<Value>) {
        let mut nodes = Vec::new();
        let mut pages = Vec::new();
        let mut after = String::new();
        loop {
            let data = self
                .query(&format!(
                    "{{ seriesData(seriesId: \"{series_id}\", first: {first}{after}{args}) {{ \
                       totalCount pageInfo {{ hasNextPage hasPreviousPage endCursor }} \
                       nodes {{ date value }} }} }}"
                ))
                .await;
            let conn = data["seriesData"].clone();
            nodes.extend(conn["nodes"].as_array().unwrap().iter().cloned());
            let has_next = conn["pageInfo"]["hasNextPage"].as_bool().unwrap();
            let end = conn["pageInfo"]["endCursor"].clone();
            pages.push(conn);
            if !has_next {
                break;
            }
            after = format!(", after: {end}");
            assert!(pages.len() < 100, "paging does not end");
        }
        (nodes, pages)
    }
}

/// 12,000 consecutive days from 1990-01-01, valued 1..=12000.
const DAILY_12000: &str =
    "SELECT DATE '1990-01-01' + i - 1, i::numeric, DATE '1990-01-01' + i - 1, true \
                           FROM generate_series(1, 12000) i";

#[tokio::test]
async fn long_series_reports_its_true_total_and_pages_to_the_end() {
    let Some(db) = Fixture::new().await else {
        return;
    };
    let series = db.series("Daily", DAILY_12000).await;

    // Without `first`, one full page (the 10,000 cap), and the client is told more remain.
    let data = db
        .query(&format!(
            "{{ seriesData(seriesId: \"{series}\") {{ totalCount \
               pageInfo {{ hasNextPage hasPreviousPage startCursor endCursor }} nodes {{ date }} }} }}"
        ))
        .await;
    let conn = &data["seriesData"];
    assert_eq!(conn["totalCount"], 12000);
    assert_eq!(conn["nodes"].as_array().unwrap().len(), 10000);
    assert_eq!(conn["pageInfo"]["hasNextPage"], true);
    assert_eq!(conn["pageInfo"]["hasPreviousPage"], false);
    assert_eq!(conn["pageInfo"]["startCursor"], "1");
    assert_eq!(conn["pageInfo"]["endCursor"], "10000");

    // Asking for more than the cap still returns the cap.
    let data = db
        .query(&format!(
            "{{ seriesData(seriesId: \"{series}\", first: 50000) {{ nodes {{ date }} }} }}"
        ))
        .await;
    assert_eq!(data["seriesData"]["nodes"].as_array().unwrap().len(), 10000);

    // Paging by endCursor reaches every point, in order, exactly once.
    let (nodes, pages) = db.all_pages(series, "", 5000).await;
    assert_eq!(pages.len(), 3);
    assert!(pages.iter().all(|p| p["totalCount"] == 12000));
    assert_eq!(pages[1]["pageInfo"]["hasPreviousPage"], true);
    assert_eq!(pages[2]["pageInfo"]["endCursor"], "12000");
    assert_eq!(nodes.len(), 12000);
    let dates: Vec<NaiveDate> = nodes
        .iter()
        .map(|n| n["date"].as_str().unwrap().parse().unwrap())
        .collect();
    let expected: Vec<NaiveDate> = (0..12000)
        .map(|i| day(1990, 1, 1) + chrono::Duration::days(i))
        .collect();
    assert_eq!(dates, expected);

    // A page past the end is empty; its endCursor stays where it was.
    let data = db
        .query(&format!(
            "{{ seriesData(seriesId: \"{series}\", after: \"12000\") {{ totalCount \
               pageInfo {{ hasNextPage startCursor endCursor }} nodes {{ date }} }} }}"
        ))
        .await;
    let conn = &data["seriesData"];
    assert_eq!(conn["totalCount"], 12000);
    assert_eq!(conn["nodes"].as_array().unwrap().len(), 0);
    assert_eq!(conn["pageInfo"]["hasNextPage"], false);
    assert_eq!(conn["pageInfo"]["startCursor"], Value::Null);
    assert_eq!(conn["pageInfo"]["endCursor"], "12000");

    // `first: 0` returns only the count, and its cursor doesn't send a paging client back to the
    // start.
    let data = db
        .query(&format!(
            "{{ seriesData(seriesId: \"{series}\", first: 0, after: \"5\") {{ totalCount \
               pageInfo {{ hasNextPage endCursor }} nodes {{ date }} }} }}"
        ))
        .await;
    let conn = &data["seriesData"];
    assert_eq!(conn["totalCount"], 12000);
    assert_eq!(conn["nodes"].as_array().unwrap().len(), 0);
    assert_eq!(conn["pageInfo"]["hasNextPage"], true);
    assert_eq!(conn["pageInfo"]["endCursor"], "5");
    db.finish().await;
}

#[tokio::test]
async fn count_is_of_latest_revisions() {
    let Some(db) = Fixture::new().await else {
        return;
    };
    // 3 dates with 3 revisions each; revision r is published r months after the date.
    let series = db
        .series(
            "Monthly",
            "SELECT (DATE '2020-01-01' + make_interval(months => d))::date, (d * 10 + r)::numeric, \
                    (DATE '2020-01-01' + make_interval(months => d + r))::date, r = 0 \
             FROM generate_series(0, 2) d, generate_series(0, 2) r",
        )
        .await;

    let (nodes, pages) = db
        .all_pages(series, ", filter: { latestRevisionOnly: true }", 2)
        .await;
    assert_eq!(pages.len(), 2);
    assert!(pages.iter().all(|p| p["totalCount"] == 3));
    let values: Vec<_> = nodes.iter().map(value).collect();
    assert_eq!(values, vec![dec("2"), dec("12"), dec("22")]);

    let (nodes, pages) = db.all_pages(series, "", 4).await;
    assert!(pages.iter().all(|p| p["totalCount"] == 9));
    assert_eq!(nodes.len(), 9);

    // As known on 2020-03-01: Jan's second revision, Feb's first, Mar's original.
    let (nodes, pages) = db
        .all_pages(series, ", filter: { asOf: \"2020-03-01\" }", 2)
        .await;
    assert!(pages.iter().all(|p| p["totalCount"] == 3));
    let values: Vec<_> = nodes.iter().map(value).collect();
    assert_eq!(values, vec![dec("2"), dec("11"), dec("20")]);
    db.finish().await;
}

#[tokio::test]
async fn transformed_pages_match_the_whole_series() {
    let Some(db) = Fixture::new().await else {
        return;
    };
    // 40 months of a wobbly, positive series.
    let series = db
        .series(
            "Monthly",
            "SELECT (DATE '2020-01-01' + make_interval(months => i))::date, \
                    (100 + i * 3 + (i % 5) * 7)::numeric, \
                    (DATE '2020-01-01' + make_interval(months => i))::date, true \
             FROM generate_series(0, 39) i",
        )
        .await;

    for transformation in [
        "LOG_DIFFERENCE",
        "YEAR_OVER_YEAR",
        "QUARTER_OVER_QUARTER",
        "MONTH_OVER_MONTH",
        "PERCENT_CHANGE",
    ] {
        let args = format!(", transformation: {transformation}");
        let (whole, _) = db.all_pages(series, &args, 1000).await;
        let (paged, pages) = db.all_pages(series, &args, 7).await;
        assert_eq!(pages.len(), 6, "{transformation}");
        assert_eq!(whole.len(), 40, "{transformation}");
        assert_eq!(paged, whole, "{transformation}");
    }
    db.finish().await;
}

#[tokio::test]
async fn transformed_pages_match_the_whole_series_across_revisions() {
    let Some(db) = Fixture::new().await else {
        return;
    };
    // 15 dates with 3 revisions each and `latestRevisionOnly: false`, so pages split a date's
    // revisions.
    // 15 months gives YoY (12 apart) and QoQ (3 apart) pairs, so both actually compute a value.
    let series = db
        .series(
            "Monthly",
            "SELECT (DATE '2020-01-01' + make_interval(months => d))::date, (d * 10 + r + 1)::numeric, \
                    (DATE '2020-01-01' + make_interval(months => d + r))::date, r = 0 \
             FROM generate_series(0, 14) d, generate_series(0, 2) r",
        )
        .await;

    for transformation in [
        "LOG_DIFFERENCE",
        "PERCENT_CHANGE",
        "MONTH_OVER_MONTH",
        "QUARTER_OVER_QUARTER",
        "YEAR_OVER_YEAR",
    ] {
        let args =
            format!(", filter: {{ latestRevisionOnly: false }}, transformation: {transformation}");
        let (whole, _) = db.all_pages(series, &args, 1000).await;
        assert_eq!(whole.len(), 45, "{transformation}");
        assert!(
            whole.iter().any(|n| n["value"] != Value::Null),
            "{transformation} should compute at least one value over 15 months"
        );
        for first in [1, 2, 4] {
            let (paged, _) = db.all_pages(series, &args, first).await;
            assert_eq!(paged, whole, "{transformation}, first: {first}");
        }
    }
    db.finish().await;
}

#[tokio::test]
async fn percent_change_paging_skips_leading_null_and_zero_points_consistently() {
    let Some(db) = Fixture::new().await else {
        return;
    };
    // Jan is null and Feb is zero, so Mar (10) is the real base; Apr..Oct keep climbing by 10.
    let series = db
        .series(
            "Monthly",
            "SELECT (DATE '2020-01-01' + make_interval(months => d))::date, \
                    CASE d WHEN 0 THEN NULL WHEN 1 THEN 0 ELSE (d - 1) * 10 END::numeric, \
                    (DATE '2020-01-01' + make_interval(months => d))::date, true \
             FROM generate_series(0, 7) d",
        )
        .await;

    let (whole, _) = db
        .all_pages(series, ", transformation: PERCENT_CHANGE", 1000)
        .await;
    assert_eq!(whole.len(), 8);
    assert_eq!(whole[0]["value"], Value::Null); // null point: nothing to transform
    assert_eq!(value(&whole[1]).round(0), dec("-100")); // zero vs. base 10
    assert_eq!(value(&whole[2]).round(0), dec("0")); // the base itself
    assert_eq!(value(&whole[7]).round(0), dec("500")); // (60-10)/10 * 100

    // Every page size that keeps Jan..Mar together on the first page gives the same values,
    // including pages whose context (loaded separately per page) must itself skip the leading
    // null/zero to find the real base.
    //
    // A page boundary that *splits* the leading null/zero run from the base it precedes (e.g.
    // `first: 1`) is a known gap this fix doesn't close: a page seen before the base is found
    // has no later point to borrow one from, so it renders those points as having no usable
    // base yet, unlike a single whole-series read. That needs look-ahead paging context, which
    // is out of scope here; see the PR description.
    for first in [3, 4, 8] {
        let (paged, _) = db
            .all_pages(series, ", transformation: PERCENT_CHANGE", first)
            .await;
        assert_eq!(paged, whole, "first: {first}");
    }
    db.finish().await;
}

#[tokio::test]
async fn transformations_default_to_the_latest_revisions() {
    let Some(db) = Fixture::new().await else {
        return;
    };
    // 15 months, each published 3 times (r months after the date); the latest revision of month d
    // is d * 10 + 3.
    let series = db
        .series(
            "Monthly",
            "SELECT (DATE '2020-01-01' + make_interval(months => d))::date, (d * 10 + r + 1)::numeric, \
                    (DATE '2020-01-01' + make_interval(months => d + r))::date, r = 0 \
             FROM generate_series(0, 14) d, generate_series(0, 2) r",
        )
        .await;

    for transformation in ["PERCENT_CHANGE", "MONTH_OVER_MONTH", "YEAR_OVER_YEAR"] {
        let explicit =
            format!(", filter: {{ latestRevisionOnly: true }}, transformation: {transformation}");
        let default = format!(", transformation: {transformation}");
        let (expected, _) = db.all_pages(series, &explicit, 1000).await;
        assert_eq!(expected.len(), 15, "{transformation}");
        let (got, _) = db.all_pages(series, &default, 1000).await;
        assert_eq!(got, expected, "{transformation}");
        // Paged the same way too.
        let (paged, _) = db.all_pages(series, &default, 4).await;
        assert_eq!(paged, expected, "{transformation}");
    }

    // The latest revisions are 3, 13, 23, ...: month-over-month is 13/3 - 1 = 333.33...%.
    let (got, _) = db
        .all_pages(series, ", transformation: MONTH_OVER_MONTH", 1000)
        .await;
    assert_eq!(got[0]["value"], Value::Null);
    assert_eq!(value(&got[1]).round(2), dec("333.33"));

    // `originalOnly: false` is no revision mode, so the default still applies.
    let (got, _) = db
        .all_pages(
            series,
            ", filter: { originalOnly: false }, transformation: MONTH_OVER_MONTH",
            1000,
        )
        .await;
    assert_eq!(got.len(), 15);
    assert_eq!(value(&got[1]).round(2), dec("333.33"));

    // `asOf` is a mode: as known on 2020-03-01, month d is its newest revision published by then
    // (month 0: r=2 -> 3, month 1: r=1 -> 12, month 2: r=0 -> 21), one point per known date.
    let (known, _) = db
        .all_pages(
            series,
            ", filter: { asOf: \"2020-03-01\" }, transformation: MONTH_OVER_MONTH",
            1000,
        )
        .await;
    assert_eq!(known.len(), 3);
    assert_eq!(value(&known[1]).round(2), dec("300"));
    assert_eq!(value(&known[2]).round(2), dec("75"));

    // The nested `dataPoints` field follows the same rule (`series` hides a series with no
    // `end_date`).
    db.sql(&format!(
        "UPDATE economic_series SET end_date = DATE '2021-03-01' WHERE id = '{series}'"
    ))
    .await;
    let data = db
        .query(&format!(
            "{{ series(id: \"{series}\") {{ dataPoints(transformation: MONTH_OVER_MONTH) {{ date value }} }} }}"
        ))
        .await;
    let nested = data["series"]["dataPoints"].as_array().unwrap();
    assert_eq!(nested.len(), 15);
    assert_eq!(value(&nested[1]).round(2), dec("333.33"));

    // An explicit revision mode is respected: originals only is not collapsed to the latest.
    let (originals, _) = db
        .all_pages(
            series,
            ", filter: { originalOnly: true }, transformation: PERCENT_CHANGE",
            1000,
        )
        .await;
    assert_eq!(originals.len(), 15);
    assert_eq!(value(&originals[1]).round(2), dec("1000"));
    db.finish().await;
}

#[tokio::test]
async fn rejects_bad_paging_arguments() {
    let Some(db) = Fixture::new().await else {
        return;
    };
    let series = db
        .series(
            "Daily",
            "SELECT DATE '2020-01-01', 1::numeric, DATE '2020-01-01', true",
        )
        .await;
    let schema = crate::graphql::schema::create_schema(db.pool.clone());
    for args in ["first: -1", "after: \"-3\"", "after: \"abc\""] {
        let resp = schema
            .execute(format!(
                "{{ seriesData(seriesId: \"{series}\", {args}) {{ totalCount }} }}"
            ))
            .await;
        assert!(!resp.errors.is_empty(), "{args} should be rejected");
    }
    db.finish().await;
}
