//! DB-backed tests for `DataPoint.revisedFromOriginal` (ECO-826): whether a point's value
//! differs from its date's original release, which the series page shows as "Revised".
//!
//! Each read path is checked: `seriesData` (what the series page reads, with
//! `latestRevisionOnly`, `asOf` and a transformation) and `EconomicSeries.dataPoints`.
//!
//! They need `DATABASE_URL` and are skipped without it. Each test creates its own data source
//! (unique name), so it can share the database.
use crate::graphql::schema::create_schema;
use crate::imports::*;
use chrono::NaiveDate;
use econ_graph_core::models::{
    DataPoint, EconomicSeries, NewDataPoint, NewDataSource, NewEconomicSeries,
};
use std::str::FromStr;

async fn db() -> Option<DatabasePool> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed revisedFromOriginal test");
        return None;
    };
    crate::graphql::test_db::migrate_once(&url).await;
    Some(
        econ_graph_core::database::create_pool(&url)
            .await
            .expect("pool"),
    )
}

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// A series under a fresh data source, with these points: (date, value or None,
/// revision_date, is_original_release).
async fn series_with(pool: &DatabasePool, points: &[(&str, Option<&str>, &str, bool)]) -> Uuid {
    let source = DataSource::create(
        pool,
        NewDataSource {
            name: format!("Test source {}", Uuid::new_v4()),
            base_url: "https://example.test".into(),
            rate_limit_per_minute: 60,
            ..NewDataSource::default()
        },
    )
    .await
    .expect("create data source");
    let dataset_id =
        econ_graph_core::test_utils::test_dataset_id(&mut pool.get().await.unwrap(), source.id)
            .await;
    let series = EconomicSeries::create(
        pool,
        &NewEconomicSeries {
            source_id: source.id,
            dataset_id,
            external_id: "ECO826".into(),
            title: "Series ECO826".into(),
            // As the crawler sets it: `series` treats a series without one as having no data.
            end_date: Some(day("2019-01-01")),
            ..Default::default()
        },
    )
    .await
    .expect("create series");
    for (date, value, revision_date, is_original_release) in points {
        DataPoint::create(
            pool,
            &NewDataPoint {
                series_id: series.id,
                date: day(date),
                value: value.map(|v| BigDecimal::from_str(v).unwrap()),
                revision_date: day(revision_date),
                is_original_release: *is_original_release,
            },
        )
        .await
        .expect("create data point");
    }
    series.id
}

/// Runs `query` and returns its data.
async fn run(pool: &DatabasePool, query: String) -> serde_json::Value {
    let schema = create_schema(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );
    let response = schema
        .execute(async_graphql::Request::new(query.clone()))
        .await;
    assert!(response.errors.is_empty(), "{query}: {:?}", response.errors);
    response.data.into_json().unwrap()
}

/// (date, revisionDate, isOriginalRelease, revisedFromOriginal) of each node.
fn flags(nodes: &serde_json::Value) -> Vec<(String, String, bool, Option<bool>)> {
    nodes
        .as_array()
        .unwrap_or_else(|| panic!("not a list: {nodes}"))
        .iter()
        .map(|p| {
            (
                p["date"].as_str().unwrap().to_string(),
                p["revisionDate"].as_str().unwrap().to_string(),
                p["isOriginalRelease"].as_bool().unwrap(),
                p["revisedFromOriginal"].as_bool(),
            )
        })
        .collect()
}

/// `seriesData` nodes for `args` (inside the call's parentheses, after `seriesId`).
async fn series_data(
    pool: &DatabasePool,
    series: Uuid,
    args: &str,
) -> Vec<(String, String, bool, Option<bool>)> {
    let data = run(
        pool,
        format!(
            r#"{{ seriesData(seriesId: "{series}" {args}) {{
                nodes {{ date revisionDate isOriginalRelease revisedFromOriginal }} }} }}"#
        ),
    )
    .await;
    flags(&data["seriesData"]["nodes"])
}

fn row(
    date: &str,
    revision: &str,
    original: bool,
    revised: Option<bool>,
) -> (String, String, bool, Option<bool>) {
    (date.into(), revision.into(), original, revised)
}

/// World Bank dates after the first-release relabel (ECO-738): a later release that repeated
/// the first one's value is a revision but not revised; one with another value is revised.
/// Values compare as numbers, and a missing value differs from any number.
#[tokio::test]
async fn a_later_release_is_revised_only_when_its_value_changed() {
    let Some(pool) = db().await else { return };
    let series = series_with(
        &pool,
        &[
            // The same value released again.
            ("2000-01-01", Some("3.0"), "2021-07-01", true),
            ("2000-01-01", Some("3.0"), "2022-07-01", false),
            // A changed value.
            ("2001-01-01", Some("3.0"), "2021-07-01", true),
            ("2001-01-01", Some("3.5"), "2022-07-01", false),
            // Only the original release.
            ("2002-01-01", Some("4.0"), "2021-07-01", true),
            // The same number with another scale.
            ("2003-01-01", Some("5.10"), "2021-07-01", true),
            ("2003-01-01", Some("5.1000"), "2022-07-01", false),
            // A value where the original had none.
            ("2004-01-01", None, "2021-07-01", true),
            ("2004-01-01", Some("1"), "2022-07-01", false),
        ],
    )
    .await;

    let latest = [
        row("2000-01-01", "2022-07-01", false, Some(false)),
        row("2001-01-01", "2022-07-01", false, Some(true)),
        row("2002-01-01", "2021-07-01", true, Some(false)),
        row("2003-01-01", "2022-07-01", false, Some(false)),
        row("2004-01-01", "2022-07-01", false, Some(true)),
    ];
    assert_eq!(
        series_data(&pool, series, "filter: { latestRevisionOnly: true }").await,
        latest
    );
    // Computed from the stored values, so a transformation doesn't change it.
    let transformed = series_data(
        &pool,
        series,
        "filter: { latestRevisionOnly: true } transformation: YEAR_OVER_YEAR",
    )
    .await;
    assert_eq!(transformed, latest);
    // Paged: each page knows its own dates' originals.
    let mut paged = series_data(
        &pool,
        series,
        "filter: { latestRevisionOnly: true } first: 2",
    )
    .await;
    paged.extend(
        series_data(
            &pool,
            series,
            r#"filter: { latestRevisionOnly: true } first: 3 after: "2""#,
        )
        .await,
    );
    assert_eq!(paged, latest);

    // Every revision: each row against its date's original.
    let all = series_data(&pool, series, "").await;
    assert_eq!(
        all,
        [
            row("2000-01-01", "2021-07-01", true, Some(false)),
            row("2000-01-01", "2022-07-01", false, Some(false)),
            row("2001-01-01", "2021-07-01", true, Some(false)),
            row("2001-01-01", "2022-07-01", false, Some(true)),
            row("2002-01-01", "2021-07-01", true, Some(false)),
            row("2003-01-01", "2021-07-01", true, Some(false)),
            row("2003-01-01", "2022-07-01", false, Some(false)),
            row("2004-01-01", "2021-07-01", true, Some(false)),
            row("2004-01-01", "2022-07-01", false, Some(true)),
        ]
    );

    // `EconomicSeries.dataPoints` gives the same answer.
    let data = run(
        &pool,
        format!(
            r#"{{ series(id: "{series}") {{ dataPoints(filter: {{ latestRevisionOnly: true }}) {{
                date revisionDate isOriginalRelease revisedFromOriginal }} }} }}"#
        ),
    )
    .await;
    assert_eq!(flags(&data["series"]["dataPoints"]), latest);
}

/// A FRED-style vintage history: a revision is revised while its value differs from the first
/// release, including as of a past day, and not once a later vintage restores that value.
#[tokio::test]
async fn a_vintage_revised_back_to_its_first_value_is_not_revised() {
    let Some(pool) = db().await else { return };
    let series = series_with(
        &pool,
        &[
            ("2020-01-01", Some("100.0"), "2020-01-30", true),
            ("2020-01-01", Some("100.5"), "2020-02-28", false),
            ("2020-01-01", Some("100.0"), "2020-07-30", false),
            ("2020-02-01", Some("101.0"), "2020-02-28", true),
            ("2020-02-01", Some("101.2"), "2020-07-30", false),
        ],
    )
    .await;
    assert_eq!(
        series_data(&pool, series, "filter: { latestRevisionOnly: true }").await,
        [
            row("2020-01-01", "2020-07-30", false, Some(false)),
            row("2020-02-01", "2020-07-30", false, Some(true)),
        ]
    );
    assert_eq!(
        series_data(&pool, series, r#"filter: { asOf: "2020-03-01" }"#).await,
        [
            row("2020-01-01", "2020-02-28", false, Some(true)),
            row("2020-02-01", "2020-02-28", true, Some(false)),
        ]
    );
}

/// A synthetic legacy row (`revision_date = date`, tagged original, beside a real original) is
/// never the original release a value is compared with (ECO-805), even when it is the earliest
/// row; a date with no original row has no known original, so the field is null.
#[tokio::test]
async fn synthetic_rows_are_not_originals_and_a_date_without_one_is_unknown() {
    let Some(pool) = db().await else { return };
    let series = series_with(
        &pool,
        &[
            // A synthetic row older than the real original: compared with the real one (5.0),
            // not the synthetic 9.9.
            ("2010-01-01", Some("9.9"), "2010-01-01", true),
            ("2010-01-01", Some("5.0"), "2021-07-01", true),
            ("2010-01-01", Some("5.0"), "2022-07-01", false),
            // A projection published before the date, and a newer synthetic row.
            ("2011-01-01", Some("4.0"), "2010-10-01", true),
            ("2011-01-01", Some("3.9"), "2011-01-01", true),
            // A legacy row alone (no other original): it is the date's original release.
            ("2012-01-01", Some("7.0"), "2012-01-01", true),
            ("2012-01-01", Some("7.5"), "2021-07-01", false),
            // No original release stored at all.
            ("2013-01-01", Some("8.0"), "2021-07-01", false),
        ],
    )
    .await;
    assert_eq!(
        series_data(&pool, series, "filter: { latestRevisionOnly: true }").await,
        [
            row("2010-01-01", "2022-07-01", false, Some(false)),
            row("2011-01-01", "2010-10-01", true, Some(false)),
            row("2012-01-01", "2021-07-01", false, Some(true)),
            row("2013-01-01", "2021-07-01", false, None),
        ]
    );
}
