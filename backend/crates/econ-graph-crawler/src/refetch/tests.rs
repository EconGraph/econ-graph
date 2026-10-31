// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! DB-backed tests for forced re-fetches. They need `DATABASE_URL` and are skipped when it is
//! unset; each runs in its own fresh database.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use diesel_async::RunQueryDsl;
use econ_graph_core::models::{CrawlQueueItem, JobKind};
use uuid::Uuid;

use super::*;
use crate::adapter::{AdapterRegistry, DiscoveredSeries, FetchedPoint, FetchedSeries};
use crate::cli::{new_job, DEFAULT_PRIORITY};
use crate::dataset::{DatasetCatalog, SeriesDataset};
use crate::persist;
use crate::persist::stable_id_tests::{database_url, FreshDb};
use crate::sources::fred::FredAdapter;
use crate::sources::world_bank::WorldBankAdapter;
use crate::testkit::{test_ctx, MockSource, Reply, Route};
use crate::worker::{JobOutcome, Worker, WorkerConfig};

const INDICATOR: &str = "NY.GDP.MKTP.PP.CD";
const WLD: &str = "wdi/NY.GDP.MKTP.PP.CD.WLD";
const USA: &str = "wdi/NY.GDP.MKTP.PP.CD.USA";

/// A World Bank response for [`INDICATOR`], last updated 2026-07-01. The WLD values are of the
/// size of the real world aggregate (about 1.9e14) and, like the USA value, carry more digits
/// than DECIMAL(20,6) or an f64 keeps.
const WDI_BODY: &str = r#"[
  {"page": 1, "pages": 1, "per_page": 20000, "total": 4, "sourceid": "2",
   "lastupdated": "2026-07-01"},
  [
    {"indicator": {"id": "NY.GDP.MKTP.PP.CD", "value": "GDP, PPP (current international $)"},
     "country": {"id": "1W", "value": "World"}, "countryiso3code": "WLD", "date": "2023",
     "value": 189543211876543.2109375, "unit": "", "obs_status": "", "decimal": 0},
    {"indicator": {"id": "NY.GDP.MKTP.PP.CD", "value": "GDP, PPP (current international $)"},
     "country": {"id": "1W", "value": "World"}, "countryiso3code": "WLD", "date": "2022",
     "value": 175000123456789.987654321, "unit": "", "obs_status": "", "decimal": 0},
    {"indicator": {"id": "NY.GDP.MKTP.PP.CD", "value": "GDP, PPP (current international $)"},
     "country": {"id": "US", "value": "United States"}, "countryiso3code": "USA", "date": "2023",
     "value": 27360935000000.123456789, "unit": "", "obs_status": "", "decimal": 0},
    {"indicator": {"id": "NY.GDP.MKTP.PP.CD", "value": "GDP, PPP (current international $)"},
     "country": {"id": "US", "value": "United States"}, "countryiso3code": "USA", "date": "2022",
     "value": null, "unit": "", "obs_status": "", "decimal": 0}
  ]
]"#;

fn d(s: &str) -> NaiveDate {
    NaiveDate::from_str(s).unwrap()
}

fn bd(s: &str) -> BigDecimal {
    BigDecimal::from_str(s).unwrap()
}

fn config() -> WorkerConfig {
    WorkerConfig {
        worker_id: "refetch-test".to_string(),
        concurrency: 1,
        poll_interval: Duration::from_millis(50),
        stuck_after: Duration::from_secs(3600),
        source_filter: None,
        pause_after_consecutive: 0,
        pause_for: Duration::from_secs(60),
        queue_retention: None,
    }
}

/// Runs the worker until the queue has nothing due; returns every outcome.
async fn drain(worker: &Worker) -> Vec<JobOutcome> {
    let mut out = Vec::new();
    while let Some(outcomes) = worker.run_batch_once().await {
        out.extend(outcomes);
    }
    out
}

/// `(data_points.id, date, revision_date, value)` of `external_id`'s points, in key order.
async fn points(
    pool: &DatabasePool,
    source: SourceId,
    external_id: &str,
) -> Vec<(Uuid, NaiveDate, NaiveDate, Option<String>)> {
    #[derive(QueryableByName)]
    struct Row {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        id: Uuid,
        #[diesel(sql_type = diesel::sql_types::Date)]
        date: NaiveDate,
        #[diesel(sql_type = diesel::sql_types::Date)]
        revision_date: NaiveDate,
        #[diesel(sql_type = Nullable<Text>)]
        value: Option<String>,
    }
    let series = persist::find_series_id(pool, source, external_id)
        .await
        .unwrap()
        .expect("series exists");
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(
        "SELECT id, date, revision_date, value::text AS value FROM data_points \
         WHERE series_id = $1 ORDER BY date, revision_date, is_original_release",
    )
    .bind::<diesel::sql_types::Uuid, _>(series)
    .load::<Row>(&mut conn)
    .await
    .unwrap()
    .into_iter()
    .map(|r| (r.id, r.date, r.revision_date, r.value))
    .collect()
}

async fn execute(pool: &DatabasePool, sql: &str) {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(sql).execute(&mut conn).await.unwrap();
}

fn full_requests(reqs: &[wiremock::Request]) -> usize {
    reqs.iter()
        .filter(|r| {
            r.url
                .query_pairs()
                .any(|(k, v)| k == "per_page" && v == "20000")
        })
        .count()
}

/// The World Bank values that DECIMAL(20,6) overflowed or rounded are stored exactly; a stored
/// rounded value survives a normal fetch (the indicator's `lastupdated` hasn't moved, so the
/// validators skip it) and is corrected by a forced re-fetch, in place: same series ids, same
/// point rows, no duplicates, and an older World Bank release's value kept as stored.
#[tokio::test]
async fn refetch_rewrites_rounded_world_bank_values_that_validators_skip() {
    let Some(admin_url) = database_url() else {
        return;
    };
    let db = FreshDb::create(&admin_url, "econgraph_refetch_world_bank").await;
    let mock = MockSource::start().await;
    let path = format!("/country/all/indicator/{INDICATOR}");
    for per_page in ["20000", "1"] {
        mock.mount(
            &Route::get(path.as_str())
                .query("format", "json")
                .query("per_page", per_page)
                .query("page", "1"),
            Reply::json_str(WDI_BODY),
        )
        .await;
    }
    let adapter = Arc::new(WorldBankAdapter::new(mock.base_url()));
    let mut registry = AdapterRegistry::new();
    registry.register(adapter.clone());
    let mut datasets = DatasetCatalog::empty();
    datasets.load_adapter(&*adapter).unwrap();
    persist::sync_datasets(&db.pool, &datasets).await.unwrap();
    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();
    let worker = Worker::new(ctx, registry, config()).with_datasets(datasets);
    let enqueue_fetch = || async {
        for id in [WLD, USA] {
            let job = new_job(
                SourceId::WorldBank,
                id,
                JobKind::FetchSeries,
                DEFAULT_PRIORITY,
            );
            CrawlQueueItem::enqueue(&db.pool, &job).await.unwrap();
        }
    };
    let release = d("2026-07-01");

    // First fetch: every digit stored, including the aggregates DECIMAL(20,6) overflowed.
    enqueue_fetch().await;
    let outcomes = drain(&worker).await;
    assert_eq!(outcomes.len(), 2);
    assert!(
        outcomes
            .iter()
            .all(|o| matches!(o, JobOutcome::Completed(_))),
        "{outcomes:?}"
    );
    let exact_wld = vec![
        (
            d("2022-01-01"),
            release,
            Some("175000123456789.987654321".to_string()),
        ),
        (
            d("2023-01-01"),
            release,
            Some("189543211876543.2109375".to_string()),
        ),
    ];
    let strip = |rows: &[(Uuid, NaiveDate, NaiveDate, Option<String>)]| {
        rows.iter()
            .map(|(_, d, r, v)| (*d, *r, v.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        strip(&points(&db.pool, SourceId::WorldBank, WLD).await),
        exact_wld
    );
    assert_eq!(
        strip(&points(&db.pool, SourceId::WorldBank, USA).await),
        vec![(
            d("2023-01-01"),
            release,
            Some("27360935000000.123456789".to_string())
        )]
    );
    let wld_series = persist::find_series_id(&db.pool, SourceId::WorldBank, WLD)
        .await
        .unwrap();
    // One full request per batch (the test policy may fetch each series on its own).
    let full = full_requests(&mock.received_requests().await);
    assert!(full >= 1);

    // The state a DECIMAL(20,6) column left behind: values rounded to six places. Plus a value
    // from an older World Bank release, which the API no longer serves.
    execute(&db.pool, "UPDATE data_points SET value = round(value, 6)").await;
    execute(
        &db.pool,
        &format!(
            "INSERT INTO data_points (series_id, date, value, revision_date) \
             VALUES ('{}', '2023-01-01', 180000000000000.5, '2025-12-01')",
            wld_series.unwrap()
        ),
    )
    .await;
    let rounded_wld = points(&db.pool, SourceId::WorldBank, WLD).await;
    let rounded_usa = points(&db.pool, SourceId::WorldBank, USA).await;
    assert_eq!(rounded_usa[0].3.as_deref(), Some("27360935000000.123457"));

    // A normal fetch: the probe's lastupdated matches, so nothing is rewritten.
    enqueue_fetch().await;
    let outcomes = drain(&worker).await;
    assert!(outcomes.iter().all(|o| matches!(
        o,
        JobOutcome::Completed(s) if s.points_written == 0
    )));
    assert_eq!(full_requests(&mock.received_requests().await), full);
    assert_eq!(
        points(&db.pool, SourceId::WorldBank, WLD).await,
        rounded_wld
    );
    assert_eq!(
        points(&db.pool, SourceId::WorldBank, USA).await,
        rounded_usa
    );

    // The forced re-fetch reads the indicator in full and rewrites the rounded values in place.
    let options = RefetchOptions::default();
    let planned = plan(&db.pool, &[SourceId::WorldBank], &options)
        .await
        .unwrap();
    assert_eq!(planned[0].to_enqueue(), 2);
    let enqueued = enqueue(&db.pool, &[SourceId::WorldBank], &options)
        .await
        .unwrap();
    assert_eq!(enqueued[0].enqueued, 2);
    let outcomes = drain(&worker).await;
    assert_eq!(outcomes.len(), 2);
    assert!(
        outcomes
            .iter()
            .all(|o| matches!(o, JobOutcome::Completed(s) if s.new_points == 0)),
        "{outcomes:?}"
    );
    assert_eq!(full_requests(&mock.received_requests().await), 2 * full);

    let after_wld = points(&db.pool, SourceId::WorldBank, WLD).await;
    let after_usa = points(&db.pool, SourceId::WorldBank, USA).await;
    // Same rows (ids), corrected values; the older release's value is untouched.
    assert_eq!(
        after_wld.iter().map(|p| p.0).collect::<Vec<_>>(),
        rounded_wld.iter().map(|p| p.0).collect::<Vec<_>>()
    );
    assert_eq!(
        strip(&after_wld),
        vec![
            exact_wld[0].clone(),
            (
                d("2023-01-01"),
                d("2025-12-01"),
                Some("180000000000000.5".to_string())
            ),
            exact_wld[1].clone(),
        ]
    );
    assert_eq!(after_usa[0].0, rounded_usa[0].0);
    assert_eq!(after_usa[0].3.as_deref(), Some("27360935000000.123456789"));
    assert_eq!(
        persist::find_series_id(&db.pool, SourceId::WorldBank, WLD)
            .await
            .unwrap(),
        wld_series
    );

    let progress = progress(&db.pool, &[SourceId::WorldBank], None)
        .await
        .unwrap();
    assert_eq!(
        progress,
        vec![SourceProgress {
            source: "WORLD_BANK".into(),
            completed: 2,
            ..SourceProgress::default()
        }]
    );
    assert!(progress[0].finished());

    db.drop().await;
}

/// `(series_id, status, count)` of every queue row, as [`StatusRow`]s keyed by series id, in
/// claim (`created_at`) order.
async fn queued(conn: &mut diesel_async::AsyncPgConnection) -> Vec<StatusRow> {
    diesel::sql_query(
        "SELECT series_id AS source, status, count(*) AS n FROM crawl_queue \
         GROUP BY series_id, status ORDER BY min(created_at)",
    )
    .load::<StatusRow>(conn)
    .await
    .unwrap()
}

fn wdi_series(area: &str) -> FetchedSeries {
    FetchedSeries {
        metadata: None,
        points: vec![FetchedPoint {
            attributes: None,
            date: d("2023-01-01"),
            value: Some(bd("1.5")),
            revision_date: d("2026-07-01"),
            is_original_release: true,
        }],
        dataset: SeriesDataset::new("wdi", [("indicator", INDICATOR), ("area", area)]),
        validators: None,
    }
}

/// Enqueuing selects the scheduler's series (active ones, and discovered ones not fetched yet),
/// skips series with an active re-fetch, and with `completed_since` skips finished ones, so it
/// can be re-run to resume. Progress counts each series by its latest job.
#[tokio::test]
async fn enqueue_is_idempotent_and_resumable() {
    let Some(admin_url) = database_url() else {
        return;
    };
    let db = FreshDb::create(&admin_url, "econgraph_refetch_enqueue").await;
    let mut datasets = DatasetCatalog::empty();
    datasets.load_adapter(&WorldBankAdapter::default()).unwrap();
    datasets.load_adapter(&FredAdapter::default()).unwrap();
    persist::sync_datasets(&db.pool, &datasets).await.unwrap();
    for area in ["USA", "DEU", "FRA"] {
        let id = format!("wdi/{INDICATOR}.{area}");
        persist::persist_series(&db.pool, SourceId::WorldBank, &id, &wdi_series(area))
            .await
            .unwrap();
    }
    // FRA is retired; WLD is discovered but not fetched yet; a FRED series is another source.
    execute(
        &db.pool,
        &format!("UPDATE economic_series SET is_active = FALSE WHERE external_id = 'wdi/{INDICATOR}.FRA'"),
    )
    .await;
    persist::persist_discovered(
        &db.pool,
        SourceId::WorldBank,
        &[DiscoveredSeries {
            external_id: WLD.to_string(),
            title: "World".into(),
            description: None,
            units: None,
            frequency: Some("Annual".into()),
            data_url: None,
            dataset: SeriesDataset::new("wdi", [("indicator", INDICATOR), ("area", "WLD")]),
        }],
    )
    .await
    .unwrap();
    persist::persist_series(
        &db.pool,
        SourceId::Fred,
        "GDP",
        &FetchedSeries {
            metadata: None,
            points: Vec::new(),
            dataset: SeriesDataset::new("FRED", Vec::<(String, String)>::new()),
            validators: None,
        },
    )
    .await
    .unwrap();
    let wb = [SourceId::WorldBank];
    let options = RefetchOptions::default();

    let dry = plan(&db.pool, &wb, &options).await.unwrap();
    assert_eq!(
        dry,
        vec![SourcePlan {
            source: "WORLD_BANK".into(),
            series: 3,
            ..SourcePlan::default()
        }]
    );
    let mut conn = db.pool.get().await.unwrap();
    assert!(queued(&mut conn).await.is_empty(), "dry run writes nothing");

    let first = enqueue(&db.pool, &wb, &options).await.unwrap();
    assert_eq!(first[0].enqueued, 3);
    // In external-id order (claim order), all refetch_series at the re-fetch priority.
    let rows = queued(&mut conn).await;
    assert_eq!(
        rows.iter().map(|r| r.source.as_str()).collect::<Vec<_>>(),
        [
            "wdi/NY.GDP.MKTP.PP.CD.DEU",
            "wdi/NY.GDP.MKTP.PP.CD.USA",
            "wdi/NY.GDP.MKTP.PP.CD.WLD"
        ]
    );
    #[derive(QueryableByName)]
    struct KindPriority {
        #[diesel(sql_type = Text)]
        kind: String,
        #[diesel(sql_type = Integer)]
        priority: i32,
    }
    let kinds: Vec<KindPriority> =
        diesel::sql_query("SELECT DISTINCT kind, priority FROM crawl_queue")
            .load(&mut conn)
            .await
            .unwrap();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0].kind, "refetch_series");
    assert_eq!(kinds[0].priority, DEFAULT_REFETCH_PRIORITY);

    // Again: everything is already queued.
    let again = enqueue(&db.pool, &wb, &options).await.unwrap();
    assert_eq!((again[0].already_queued, again[0].enqueued), (3, 0));

    // DEU completes, USA fails; WLD is still pending.
    let started = chrono::Utc::now() - chrono::Duration::hours(1);
    execute(
        &db.pool,
        "UPDATE crawl_queue SET status = 'completed', finished_at = NOW() \
         WHERE series_id LIKE '%.DEU'",
    )
    .await;
    execute(
        &db.pool,
        "UPDATE crawl_queue SET status = 'failed', finished_at = NOW(), \
             error_message = 'not found' \
         WHERE series_id LIKE '%.USA'",
    )
    .await;
    let p = progress(&db.pool, &wb, Some(started)).await.unwrap();
    assert_eq!(
        (p[0].pending, p[0].completed, p[0].failed, p[0].finished()),
        (1, 1, 1, false)
    );
    let failed = failures(&db.pool, &wb, None, 10).await.unwrap();
    assert_eq!(
        failed,
        vec![RefetchFailure {
            source: "WORLD_BANK".into(),
            series_id: "wdi/NY.GDP.MKTP.PP.CD.USA".into(),
            error_message: Some("not found".into()),
        }]
    );

    // Resume: only the failed series is enqueued again; then it counts as pending.
    let resume = RefetchOptions {
        completed_since: Some(started),
        ..RefetchOptions::default()
    };
    let resumed = enqueue(&db.pool, &wb, &resume).await.unwrap();
    assert_eq!(
        resumed,
        vec![SourcePlan {
            source: "WORLD_BANK".into(),
            series: 3,
            already_queued: 1,
            already_completed: 1,
            enqueued: 1,
        }]
    );
    let p = progress(&db.pool, &wb, Some(started)).await.unwrap();
    assert_eq!((p[0].pending, p[0].completed, p[0].failed), (2, 1, 0));
    // Without completed_since the finished series is selected again.
    assert_eq!(
        plan(&db.pool, &wb, &options).await.unwrap()[0].to_enqueue(),
        1
    );

    // --include-inactive adds the retired series.
    let inactive = RefetchOptions {
        include_inactive: true,
        completed_since: Some(started),
        ..RefetchOptions::default()
    };
    let with_retired = plan(&db.pool, &wb, &inactive).await.unwrap();
    assert_eq!(
        (with_retired[0].series, with_retired[0].to_enqueue()),
        (4, 1)
    );

    // Bad priorities are refused.
    assert!(enqueue(
        &db.pool,
        &wb,
        &RefetchOptions {
            priority: 11,
            ..RefetchOptions::default()
        }
    )
    .await
    .is_err());

    drop(conn);
    db.drop().await;
}
