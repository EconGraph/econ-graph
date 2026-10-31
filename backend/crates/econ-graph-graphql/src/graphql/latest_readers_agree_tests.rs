//! Every reader of a series' latest values agrees with `dataPoints` with `latestRevisionOnly`
//! on a date that carries a synthetic legacy row (ECO-807): a real original release published
//! before the date (a projection), a synthetic row on the date (`revision_date = date`, tagged as
//! the original release, from a pre-vintage crawl) and no later vintage. The synthetic row never
//! counts, so the latest value is the projection's.
//!
//! The readers: `EconomicSeries.dataPoints` with `latestRevisionOnly`, `seriesData`
//! (`series_service::get_series_data`), `recentDataPoints`, `latestObservation`, the data-point
//! dataloader, and `crossSection` without `asOf`.
//!
//! A source scan also fails when code outside tests reads with the plain `revision_filter`,
//! which lets synthetic rows count.
//!
//! The DB test needs `DATABASE_URL` and is skipped without it. It creates its own data source
//! (unique name), so it can share the database.
use crate::graphql::schema::create_schema;
use crate::imports::*;
use chrono::NaiveDate;
use econ_graph_core::models::dataset::{
    ComponentType, Dataset, DatasetComponent, NewDataset, SeriesDimensions,
};
use econ_graph_core::models::{
    DataPoint, DataQueryParams, EconomicSeries, NewDataPoint, NewDataSource, NewEconomicSeries,
};
use std::path::Path;
use std::str::FromStr;

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// A series with an ordinary date and the ECO-807 date, in a dataset of its own with one
/// dimension `area`. Returns the dataset and series ids.
async fn seed(pool: &DatabasePool) -> (Uuid, Uuid) {
    let source = DataSource::create(
        pool,
        NewDataSource {
            name: format!("Latest readers test source {}", Uuid::new_v4()),
            base_url: "https://example.test".into(),
            rate_limit_per_minute: 60,
            ..NewDataSource::default()
        },
    )
    .await
    .expect("create data source");
    let area = DatasetComponent::new("area", "Area", ComponentType::String);
    let dataset = Dataset::create(
        pool,
        &NewDataset::long(source.id, "eco807", "ECO-807 dataset", vec![area]),
    )
    .await
    .expect("create dataset");
    let series = EconomicSeries::create(
        pool,
        &NewEconomicSeries {
            source_id: source.id,
            dataset_id: dataset.id,
            external_id: "ECO807.XXX".into(),
            title: "Series ECO807".into(),
            frequency: "Annual".into(),
            dimensions: [("area", "XXX")].into_iter().collect::<SeriesDimensions>(),
            // As the crawler sets it: `series` treats a series without one as having no data.
            end_date: Some(day("2018-01-01")),
            ..Default::default()
        },
    )
    .await
    .expect("create series");
    // (date, value, revision_date, is_original_release)
    for (date, value, revision_date, original) in [
        // An ordinary date, published once.
        ("2017-01-01", "10", "2017-03-01", true),
        // The ECO-807 date: a projection published before it, then a synthetic row, and no
        // later vintage.
        ("2018-01-01", "1", "2017-12-01", true),
        ("2018-01-01", "2", "2018-01-01", true),
    ] {
        DataPoint::create(
            pool,
            &NewDataPoint {
                series_id: series.id,
                date: day(date),
                value: Some(BigDecimal::from_str(value).unwrap()),
                revision_date: day(revision_date),
                is_original_release: original,
            },
        )
        .await
        .expect("create data point");
    }
    (dataset.id, series.id)
}

/// (date, value, revision_date) strings from a GraphQL point object.
fn point(p: &serde_json::Value) -> (String, String, String) {
    let field = |k: &str| {
        p[k].as_str()
            .map(str::to_string)
            .unwrap_or_else(|| p[k].to_string())
    };
    (field("date"), field("value"), field("revisionDate"))
}

fn expected(points: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
    points
        .iter()
        .map(|(d, v, r)| (d.to_string(), v.to_string(), r.to_string()))
        .collect()
}

#[tokio::test]
async fn every_latest_reader_agrees_with_data_points_on_a_synthetic_row_date() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed latest-readers test");
        return;
    };
    crate::graphql::test_db::migrate_once(&url).await;
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    let (dataset_id, series_id) = seed(&pool).await;
    let latest = expected(&[
        ("2017-01-01", "10", "2017-03-01"),
        ("2018-01-01", "1", "2017-12-01"),
    ]);

    let schema = create_schema(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );
    let query = format!(
        r#"{{
          series(id: "{series_id}") {{
            dataPoints(filter: {{ latestRevisionOnly: true }}) {{ date value revisionDate }}
            recentDataPoints {{ date value revisionDate }}
            latestObservation {{ date value revisionDate }}
          }}
          crossSection(datasetId: "{dataset_id}", across: "area", latest: true) {{ key date value }}
        }}"#
    );
    let response = schema.execute(async_graphql::Request::new(query)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let sorted = |field: &str| {
        let mut points: Vec<_> = data["series"][field]
            .as_array()
            .unwrap_or_else(|| panic!("{field}: {data}"))
            .iter()
            .map(point)
            .collect();
        points.sort();
        points
    };

    // The reference read, as #755 left it.
    assert_eq!(
        sorted("dataPoints"),
        latest,
        "dataPoints latestRevisionOnly"
    );
    assert_eq!(sorted("recentDataPoints"), latest, "recentDataPoints");
    assert_eq!(
        point(&data["series"]["latestObservation"]),
        latest[1],
        "latestObservation"
    );
    assert_eq!(
        data["crossSection"],
        serde_json::json!([{"key": "XXX", "date": "2018-01-01", "value": "1"}]),
        "crossSection without asOf"
    );

    // seriesData's service.
    let page = econ_graph_services::services::series_service::get_series_data(
        &pool,
        DataQueryParams {
            series_id,
            start_date: None,
            end_date: None,
            original_only: Some(false),
            latest_revision_only: Some(true),
            as_of: None,
            limit: None,
            offset: None,
        },
        None,
    )
    .await
    .expect("get_series_data");
    let mut service: Vec<_> = page
        .points
        .iter()
        .map(|p| {
            (
                p.date.to_string(),
                p.value.as_ref().unwrap().normalized().to_string(),
                p.revision_date.to_string(),
            )
        })
        .collect();
    service.sort();
    assert_eq!(service, latest, "seriesData");

    // The data-point dataloader.
    use crate::graphql::dataloaders::DataPointsBySeriesBatcher;
    use dataloader::BatchFn;
    let loaded = BatchFn::load(
        &mut DataPointsBySeriesBatcher { pool: pool.clone() },
        &[series_id],
    )
    .await;
    let mut loaded: Vec<_> = loaded[&series_id]
        .iter()
        .map(|p| {
            (
                p.date.to_string(),
                p.value.as_ref().unwrap().normalized().to_string(),
                p.revision_date.to_string(),
            )
        })
        .collect();
    loaded.sort();
    assert_eq!(loaded, latest, "data-point dataloader");
}

/// Code outside tests that resolves revisions must not let synthetic legacy rows count: it
/// uses `revision_filter_excluding_synthetic` with `exclude_synthetic_legacy_rows`, or
/// `synthetic_legacy_row` in raw SQL. This fails on a call of the plain `revision_filter`
/// anywhere in the workspace's crates outside test files and the filter's own definition.
#[test]
fn no_reader_uses_the_plain_revision_filter() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut offenders = Vec::new();
    let mut pending = vec![crates.clone()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if path.is_dir() {
                if name != "tests" && name != "target" && !name.starts_with('.') {
                    pending.push(path);
                }
                continue;
            }
            let is_test =
                name == "tests.rs" || name.ends_with("_tests.rs") || name.ends_with("_test.rs");
            if !name.ends_with(".rs") || is_test || path.ends_with("models/data_point.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for (n, line) in source.lines().enumerate() {
                let code = line.split("//").next().unwrap();
                if code.contains("revision_filter(") {
                    offenders.push(format!(
                        "{}:{}",
                        path.strip_prefix(&crates).unwrap().display(),
                        n + 1
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these read with the plain revision_filter, which lets synthetic legacy rows count; use \
         revision_filter_excluding_synthetic with exclude_synthetic_legacy_rows: {offenders:?}"
    );
}
