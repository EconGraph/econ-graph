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
//! which lets synthetic rows count, splits the pair of filters, or reads `data_points` in raw
//! SQL that isn't accounted for.
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
                attributes: None,
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

/// Raw SQL reading `data_points`, outside tests, by file: whether it resolves revisions, and so
/// must use `synthetic_legacy_row`. A new file must be added here, which forces that question.
const RAW_DATA_POINTS_SQL: &[(&str, bool)] = &[
    // The filters' own definitions.
    ("econ-graph-core/src/models/data_point.rs", true),
    // latestObservation.
    ("econ-graph-graphql/src/graphql/dataloaders.rs", true),
    (
        "econ-graph-services/src/services/cross_section_service.rs",
        true,
    ),
    // Whether a successor series has any data.
    ("econ-graph-crawler/src/lifecycle.rs", false),
    // The crawler's change detection on write.
    ("econ-graph-crawler/src/persist.rs", false),
    // Disposable fixture occupancy checks and deterministic IDs, without revision selection.
    ("econ-graph-crawler/src/bin/seed_fixtures.rs", false),
];

/// Lines of SOURCE outside `#[cfg(test)]` items, numbered from 1. The item an attribute marks
/// ends at its first `;` outside braces, or where its braces close.
fn without_test_items(source: &str) -> Vec<(usize, String)> {
    let mut kept = Vec::new();
    let mut lines = source.lines().enumerate().peekable();
    while let Some((n, line)) = lines.next() {
        if line.trim() != "#[cfg(test)]" {
            kept.push((n + 1, line.to_string()));
            continue;
        }
        while lines
            .peek()
            .is_some_and(|(_, l)| l.trim_start().starts_with("#["))
        {
            lines.next();
        }
        let mut depth = 0i32;
        for (_, item) in lines.by_ref() {
            let mut ended = false;
            for c in item.chars() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        ended |= depth == 0;
                    }
                    ';' if depth == 0 => ended = true,
                    _ => {}
                }
            }
            if ended {
                break;
            }
        }
    }
    kept
}

/// Each non-test source file of the workspace's crates, relative to `crates/`, with its code
/// lines: whole-line comments and `#[cfg(test)]` items dropped.
fn sources() -> Vec<(String, Vec<(usize, String)>)> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
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
            if !name.ends_with(".rs") || is_test {
                continue;
            }
            let lines = without_test_items(&std::fs::read_to_string(&path).unwrap())
                .into_iter()
                .filter(|(_, line)| !line.trim_start().starts_with("//"))
                .collect();
            let relative = path.strip_prefix(&crates).unwrap().display().to_string();
            files.push((relative, lines));
        }
    }
    files
}

/// Code outside tests that resolves revisions must not let synthetic legacy rows count.
///
/// - The plain `revision_filter` is called only where it is defined.
/// - `revision_filter_excluding_synthetic` comes with `exclude_synthetic_legacy_rows`: alone,
///   it keeps a synthetic row beside the projection, two rows for one date.
/// - Raw SQL on `data_points` is only in the files listed in [`RAW_DATA_POINTS_SQL`], and those
///   that resolve revisions use `synthetic_legacy_row`; each listed file still has some.
///
/// It reads source text, so it checks per file (a second reader in a file that already pairs
/// the filters isn't told apart) and can't follow aliases; review covers what it can't.
#[test]
fn no_reader_lets_synthetic_legacy_rows_count() {
    let mut problems = Vec::new();
    let mut listed_seen = Vec::new();
    for (file, lines) in sources() {
        let code = lines
            .iter()
            .map(|(_, l)| l.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        // Whitespace-insensitive, for SQL split across lines or continuation backslashes.
        let words = code
            .split(|c: char| c.is_whitespace() || c == '\\')
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>();
        let raw_sql = words
            .windows(2)
            .any(|w| (w[0] == "FROM" || w[0] == "JOIN") && w[1] == "data_points");
        let definition = file.ends_with("econ-graph-core/src/models/data_point.rs");
        for (n, line) in &lines {
            if line.contains("revision_filter(") && !definition {
                problems.push(format!("{file}:{n}: the plain revision_filter"));
            }
        }
        if code.contains("revision_filter_excluding_synthetic(")
            && !code.contains("exclude_synthetic_legacy_rows(")
            && !definition
        {
            problems.push(format!(
                "{file}: revision_filter_excluding_synthetic without exclude_synthetic_legacy_rows"
            ));
        }
        if raw_sql {
            let listed = RAW_DATA_POINTS_SQL.iter().find(|(f, _)| file.ends_with(f));
            if let Some((f, _)) = listed {
                listed_seen.push(*f);
            }
            match listed {
                None => problems.push(format!(
                    "{file}: raw SQL on data_points; add it to RAW_DATA_POINTS_SQL, using \
                     synthetic_legacy_row if it resolves revisions"
                )),
                Some((_, true)) if !code.contains("synthetic_legacy_row(") => problems.push(
                    format!("{file}: resolves revisions without synthetic_legacy_row"),
                ),
                _ => {}
            }
        }
    }
    for (f, _) in RAW_DATA_POINTS_SQL {
        if !listed_seen.contains(f) {
            problems.push(format!(
                "RAW_DATA_POINTS_SQL lists {f}, which has no raw SQL on data_points"
            ));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn test_items_are_skipped_up_to_their_end() {
    let source = "a\n#[cfg(test)]\nfn plan() {\n  x;\n}\nb\n#[cfg(test)]\n#[allow(x)]\nmod t;\nc\n\
                  #[cfg(test)]\n    let y = 1;\nd\n#[cfg(test)]\nmod tests {\n  fn f() {}\n}\ne";
    let kept: Vec<_> = without_test_items(source)
        .into_iter()
        .map(|(_, l)| l.trim().to_string())
        .collect();
    assert_eq!(kept, ["a", "b", "c", "d", "e"]);
}
