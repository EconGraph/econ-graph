//! DB-backed tests for `originalOnly`, `asOf` and latest reads of a date that carries a
//! synthetic legacy row (`revision_date = date`, tagged as the original release) beside a real
//! original release published before the date (ECO-805). The synthetic row is excluded from
//! these reads, and must not hide the real original either: the date stays, with that original.
//!
//! Both read paths are checked: `EconomicSeries.dataPoints` (crate::types) and
//! `series_service::get_series_data` (behind `seriesData`).
//!
//! They need `DATABASE_URL` and are skipped without it. The test creates its own data source
//! (unique name), so it can share the database.
use crate::graphql::schema::create_schema;
use crate::imports::*;
use chrono::NaiveDate;
use econ_graph_core::models::{
    DataPoint, DataQueryParams, EconomicSeries, NewDataPoint, NewDataSource, NewEconomicSeries,
};
use std::str::FromStr;

async fn db() -> Option<DatabasePool> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed originalOnly test");
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

/// A series under a fresh data source, with these points: (date, value, revision_date,
/// is_original_release).
async fn series_with(pool: &DatabasePool, points: &[(&str, &str, &str, bool)]) -> Uuid {
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
            external_id: "ECO805".into(),
            title: "Series ECO805".into(),
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
                value: Some(BigDecimal::from_str(value).unwrap()),
                revision_date: day(revision_date),
                is_original_release: *is_original_release,
            },
        )
        .await
        .expect("create data point");
    }
    series.id
}

/// (date, revision_date) of each point `EconomicSeries.dataPoints` returns for `filter`.
async fn graphql_points(pool: &DatabasePool, series: Uuid, filter: &str) -> Vec<(String, String)> {
    let schema = create_schema(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );
    let query = format!(
        r#"{{ series(id: "{series}") {{ dataPoints(filter: {{ {filter} }}) {{ date revisionDate }} }} }}"#
    );
    let response = schema
        .execute(async_graphql::Request::new(query.clone()))
        .await;
    assert!(response.errors.is_empty(), "{query}: {:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let mut points: Vec<(String, String)> = data["series"]["dataPoints"]
        .as_array()
        .unwrap_or_else(|| panic!("{query}: {data}"))
        .iter()
        .map(|p| {
            let field = |k: &str| {
                p[k].as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| p[k].to_string())
            };
            (field("date"), field("revisionDate"))
        })
        .collect::<Vec<_>>();
    points.sort();
    points
}

/// (date, revision_date) of each point `get_series_data` returns.
async fn service_points(
    pool: &DatabasePool,
    series: Uuid,
    original_only: bool,
    latest_revision_only: bool,
    as_of: Option<&str>,
) -> Vec<(String, String)> {
    let page = econ_graph_services::services::series_service::get_series_data(
        pool,
        DataQueryParams {
            series_id: series,
            start_date: None,
            end_date: None,
            original_only: Some(original_only),
            latest_revision_only: Some(latest_revision_only),
            as_of: as_of.map(day),
            limit: None,
            offset: None,
        },
        None,
    )
    .await
    .expect("get_series_data");
    let mut points = page
        .points
        .iter()
        .map(|p| (p.date.to_string(), p.revision_date.to_string()))
        .collect::<Vec<_>>();
    points.sort();
    points
}

/// A read: its GraphQL filter; original_only, latest_revision_only and as_of for
/// `get_series_data`; and the (date, revision_date) points it must return.
type Case = (
    &'static str,
    bool,
    bool,
    Option<&'static str>,
    Vec<(String, String)>,
);

fn expect(points: &[(&str, &str)]) -> Vec<(String, String)> {
    points
        .iter()
        .map(|(d, r)| (d.to_string(), r.to_string()))
        .collect()
}

#[tokio::test]
async fn a_synthetic_legacy_row_does_not_hide_a_real_original_published_before_its_date() {
    let Some(pool) = db().await else { return };
    let series = series_with(
        &pool,
        &[
            // The review's reproduction: real original releases at 2017-12-01 (before the
            // observation date, a projection) and 2019-05-01, and a synthetic row on the date.
            ("2018-01-01", "1", "2017-12-01", true),
            ("2018-01-01", "2", "2018-01-01", true),
            ("2018-01-01", "3", "2019-05-01", true),
            // After the World Bank relabel (#752): one real original before the date, the
            // synthetic row, and a later revision that is no longer tagged original.
            ("2019-01-01", "4", "2018-12-01", true),
            ("2019-01-01", "5", "2019-01-01", true),
            ("2019-01-01", "6", "2019-06-01", false),
        ],
    )
    .await;

    // (GraphQL filter, original_only, latest_revision_only, as_of, expected points)
    let cases: [Case; 6] = [
        // originalOnly alone: every real original release (one per date isn't picked without
        // asOf or latestRevisionOnly); never the synthetic rows.
        (
            "originalOnly: true",
            true,
            false,
            None,
            expect(&[
                ("2018-01-01", "2017-12-01"),
                ("2018-01-01", "2019-05-01"),
                ("2019-01-01", "2018-12-01"),
            ]),
        ),
        // originalOnly, latest: the newest real original of each date. On 2019-01-01 that is
        // the original before the date, which the synthetic row must not hide.
        (
            "originalOnly: true, latestRevisionOnly: true",
            true,
            true,
            None,
            expect(&[("2018-01-01", "2019-05-01"), ("2019-01-01", "2018-12-01")]),
        ),
        // originalOnly as of a day before the later releases: the original known then, on both
        // dates, though each date's synthetic row is newer than it.
        (
            r#"originalOnly: true, asOf: "2019-03-01""#,
            true,
            false,
            Some("2019-03-01"),
            expect(&[("2018-01-01", "2017-12-01"), ("2019-01-01", "2018-12-01")]),
        ),
        // asOf alone: the newest revision known then, which is the same real original.
        (
            r#"asOf: "2019-03-01""#,
            false,
            false,
            Some("2019-03-01"),
            expect(&[("2018-01-01", "2017-12-01"), ("2019-01-01", "2018-12-01")]),
        ),
        // Later: the newest revisions, as before.
        (
            r#"asOf: "2020-01-01""#,
            false,
            false,
            Some("2020-01-01"),
            expect(&[("2018-01-01", "2019-05-01"), ("2019-01-01", "2019-06-01")]),
        ),
        (
            "latestRevisionOnly: true",
            false,
            true,
            None,
            expect(&[("2018-01-01", "2019-05-01"), ("2019-01-01", "2019-06-01")]),
        ),
    ];
    for (filter, original_only, latest_revision_only, as_of, expected) in cases {
        assert_eq!(
            graphql_points(&pool, series, filter).await,
            expected,
            "dataPoints({filter})"
        );
        assert_eq!(
            service_points(&pool, series, original_only, latest_revision_only, as_of).await,
            expected,
            "get_series_data({filter})"
        );
    }
}
