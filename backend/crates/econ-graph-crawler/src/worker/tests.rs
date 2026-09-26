//! DB-backed worker tests. They need `DATABASE_URL` (a migrated or migratable database) and are
//! skipped when it is unset. They serialise on a shared lock, so they are safe with or without
//! `--test-threads=1`. Every row they create uses external ids prefixed `t5_`; `crawl_queue` is
//! emptied before each test.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::models::{CrawlQueueItem, JobKind, NewCrawlQueueItem};
use econ_graph_core::schema::{
    crawl_attempts, crawl_queue, data_points, economic_series, series_metadata,
};
use econ_graph_core::DatabasePool;
use serde::Deserialize;
use uuid::Uuid;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, ResponseTemplate};

use super::*;
use crate::adapter::{
    AdapterRegistry, CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries,
    NewSeriesMetadataLite, SourceAdapter,
};
use crate::persist;
use crate::policy::SourcePolicy;
use crate::testkit::{fast_policy, test_ctx_with, MockSource, Reply, Route};

static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

/// Sources the test adapter is registered under (not FRED/BLS: those have real adapters).
const SRC: SourceId = SourceId::Bea;
const OTHER: SourceId = SourceId::Census;

struct Db {
    pool: DatabasePool,
    _guard: tokio::sync::MutexGuard<'static, ()>,
}

async fn db() -> Option<Db> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed worker test");
        return None;
    };
    let guard = DB_LOCK.lock().await;
    MIGRATED
        .get_or_init(|| async {
            econ_graph_core::run_migrations(&url)
                .await
                .expect("running migrations");
        })
        .await;
    let pool = econ_graph_core::create_pool(&url).await.expect("pool");
    {
        let mut conn = pool.get().await.unwrap();
        for sql in [
            "DELETE FROM crawl_queue",
            "DELETE FROM series_metadata WHERE external_id LIKE 't5\\_%'",
            "DELETE FROM economic_series WHERE external_id LIKE 't5\\_%'",
        ] {
            diesel::sql_query(sql).execute(&mut conn).await.unwrap();
        }
    }
    Some(Db {
        pool,
        _guard: guard,
    })
}

// ---------------------------------------------------------------------------
// Test adapter: GET {base}/series/{id}[?since=YYYY-MM-DD], GET {base}/catalog
// ---------------------------------------------------------------------------

struct TestAdapter {
    id: SourceId,
    base_url: String,
}

#[derive(Deserialize)]
struct SeriesBody {
    title: String,
    #[serde(default)]
    units: Option<String>,
    points: Vec<(NaiveDate, Option<String>)>,
}

#[derive(Deserialize)]
struct CatalogBody {
    series: Vec<(String, String)>,
}

#[async_trait]
impl SourceAdapter for TestAdapter {
    fn id(&self) -> SourceId {
        self.id
    }

    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let body: CatalogBody = ctx
            .http
            .get_json(self.id, &format!("{}/catalog", self.base_url), &[])
            .await?;
        Ok(body
            .series
            .into_iter()
            .map(|(id, title)| DiscoveredSeries {
                data_url: Some(format!("{}/series/{id}", self.base_url)),
                external_id: id,
                title,
                description: None,
                units: Some("Index".into()),
                frequency: Some("Monthly".into()),
            })
            .collect())
    }

    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        if external_id.contains("panic") {
            panic!("test adapter asked to panic");
        }
        let url = format!("{}/series/{external_id}", self.base_url);
        let since_s = since.map(|d| d.to_string());
        let query: Vec<(&str, &str)> = since_s.iter().map(|s| ("since", s.as_str())).collect();
        let body: SeriesBody = ctx.http.get_json(self.id, &url, &query).await?;
        let points = body
            .points
            .into_iter()
            .map(|(date, v)| FetchedPoint {
                date,
                value: v.map(|v| BigDecimal::from_str(&v).unwrap()),
                revision_date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                is_original_release: true,
            })
            .collect();
        Ok(FetchedSeries {
            metadata: Some(NewSeriesMetadataLite {
                title: body.title,
                units: body.units,
                frequency: Some("Monthly".into()),
                ..Default::default()
            }),
            points,
        })
    }
}

fn slow_backoff(source: SourceId) -> SourcePolicy {
    SourcePolicy {
        base_backoff: Duration::from_secs(60),
        max_backoff: Duration::from_secs(3600),
        ..fast_policy(source)
    }
}

fn ctx(pool: &DatabasePool) -> CrawlCtx {
    let mut ctx = test_ctx_with(HashMap::from([
        (SRC, slow_backoff(SRC)),
        (OTHER, slow_backoff(OTHER)),
    ]));
    ctx.pool = pool.clone();
    ctx
}

fn registry(mock: &MockSource) -> AdapterRegistry {
    let mut r = AdapterRegistry::new();
    for id in [SRC, OTHER] {
        r.register(Arc::new(TestAdapter {
            id,
            base_url: mock.base_url(),
        }));
    }
    r
}

fn config(worker_id: &str) -> WorkerConfig {
    WorkerConfig {
        worker_id: worker_id.to_string(),
        concurrency: 1,
        poll_interval: Duration::from_millis(50),
        stuck_after: Duration::from_secs(3600),
        source_filter: None,
        pause_after_consecutive: 0,
        pause_for: Duration::from_secs(60),
        queue_retention: None,
    }
}

fn worker(pool: &DatabasePool, mock: &MockSource) -> Worker {
    Worker::new(ctx(pool), registry(mock), config("t5-worker"))
}

async fn enqueue(
    pool: &DatabasePool,
    source: &str,
    series: &str,
    kind: JobKind,
    priority: i32,
) -> Uuid {
    CrawlQueueItem::enqueue(
        pool,
        &NewCrawlQueueItem {
            source: source.to_string(),
            series_id: series.to_string(),
            priority,
            kind: kind.as_str().to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .expect("enqueued")
    .id
}

async fn item(pool: &DatabasePool, id: Uuid) -> CrawlQueueItem {
    let mut conn = pool.get().await.unwrap();
    crawl_queue::table
        .find(id)
        .select(CrawlQueueItem::as_select())
        .first(&mut conn)
        .await
        .unwrap()
}

async fn series_row(
    pool: &DatabasePool,
    external_id: &str,
) -> Option<(
    Uuid,
    Uuid,
    String,
    Option<String>,
    Option<NaiveDate>,
    Option<NaiveDate>,
    Option<String>,
)> {
    let mut conn = pool.get().await.unwrap();
    economic_series::table
        .filter(economic_series::external_id.eq(external_id))
        .select((
            economic_series::id,
            economic_series::source_id,
            economic_series::title,
            economic_series::units,
            economic_series::start_date,
            economic_series::end_date,
            economic_series::crawl_status,
        ))
        .first(&mut conn)
        .await
        .optional()
        .unwrap()
}

async fn point_count(pool: &DatabasePool, series_id: Uuid) -> i64 {
    let mut conn = pool.get().await.unwrap();
    data_points::table
        .filter(data_points::series_id.eq(series_id))
        .count()
        .get_result(&mut conn)
        .await
        .unwrap()
}

async fn attempts(
    pool: &DatabasePool,
    series_id: Uuid,
) -> Vec<(bool, Option<String>, Option<i32>)> {
    let mut conn = pool.get().await.unwrap();
    crawl_attempts::table
        .filter(crawl_attempts::series_id.eq(series_id))
        .order(crawl_attempts::created_at.asc())
        .select((
            crawl_attempts::success,
            crawl_attempts::error_type,
            crawl_attempts::new_data_points,
        ))
        .load(&mut conn)
        .await
        .unwrap()
}

fn series_json(title: &str, points: &[(&str, Option<&str>)]) -> Reply {
    Reply::json(serde_json::json!({
        "title": title,
        "units": "Percent",
        "points": points.iter().map(|(d, v)| serde_json::json!([d, v])).collect::<Vec<_>>(),
    }))
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::from_str(s).unwrap()
}

// ---------------------------------------------------------------------------

#[tokio::test]
async fn success_persists_series_points_attempt_and_completes() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/series/t5_gdp"),
        series_json(
            "Test GDP",
            &[
                ("2024-01-01", Some("1.5")),
                ("2024-02-01", None),
                ("2024-03-01", Some("-2")),
            ],
        ),
    )
    .await;
    let w = worker(&db.pool, &mock);
    let id = enqueue(&db.pool, SRC.as_str(), "t5_gdp", JobKind::FetchSeries, 5).await;

    let outcome = w.run_once().await.expect("claimed");
    let JobOutcome::Completed(stats) = outcome else {
        panic!("expected Completed, got {outcome:?}")
    };
    assert_eq!(stats.points_written, 3);
    assert_eq!(stats.new_points, 3);
    assert_eq!(stats.latest_date, Some(d("2024-03-01")));

    let it = item(&db.pool, id).await;
    assert_eq!(it.status, "completed");
    assert!(it.locked_by.is_none() && it.locked_at.is_none());
    assert!(it.finished_at.is_some());

    let (series_id, source_id, title, units, start, end, status) =
        series_row(&db.pool, "t5_gdp").await.expect("series row");
    assert_eq!(Some(series_id), stats.series_id);
    assert_eq!(title, "Test GDP");
    assert_eq!(units.as_deref(), Some("Percent"));
    assert_eq!((start, end), (Some(d("2024-01-01")), Some(d("2024-03-01"))));
    assert_eq!(status.as_deref(), Some("success"));
    assert_eq!(point_count(&db.pool, series_id).await, 3);

    // Mapped onto the existing seeded data_sources row, not a duplicate.
    let template = persist::data_source_template(SRC);
    let mut conn = db.pool.get().await.unwrap();
    let ids: Vec<Uuid> = econ_graph_core::schema::data_sources::table
        .filter(econ_graph_core::schema::data_sources::name.eq(&template.name))
        .select(econ_graph_core::schema::data_sources::id)
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(ids, vec![source_id]);

    assert_eq!(
        attempts(&db.pool, series_id).await,
        vec![(true, None, Some(3))]
    );
    assert!(w.run_once().await.is_none(), "queue should be empty");
}

#[tokio::test]
async fn second_fetch_passes_since_latest_stored_date_minus_lookback() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/series/t5_cpi"),
        series_json(
            "CPI",
            &[("2024-01-01", Some("1")), ("2024-02-01", Some("2"))],
        ),
    )
    .await;
    let w = worker(&db.pool, &mock);
    enqueue(&db.pool, SRC.as_str(), "t5_cpi", JobKind::FetchSeries, 5).await;
    assert!(matches!(w.run_once().await, Some(JobOutcome::Completed(_))));
    let first = mock.received_requests().await;
    assert_eq!(first.len(), 1);
    assert!(first[0].url.query_pairs().all(|(k, _)| k != "since"));

    // Finished rows don't block re-enqueueing. The source revises Feb and adds Mar. The worker
    // asks for everything since latest stored date (Feb) minus the source's revision lookback.
    let lookback = w.ctx.http.policy(SRC).revision_lookback;
    assert_eq!(lookback, SourcePolicy::default_for(SRC).revision_lookback);
    assert!(lookback > Duration::ZERO);
    let expected_since =
        (d("2024-02-01") - chrono::Duration::from_std(lookback).unwrap()).to_string();
    assert_eq!(expected_since, "2023-02-01", "365-day default lookback");
    mock.reset().await;
    mock.mount(
        &Route::get("/series/t5_cpi").query("since", &expected_since),
        series_json(
            "CPI",
            &[("2024-02-01", Some("2.5")), ("2024-03-01", Some("3"))],
        ),
    )
    .await;
    let id = enqueue(&db.pool, SRC.as_str(), "t5_cpi", JobKind::FetchSeries, 5).await;
    let Some(JobOutcome::Completed(stats)) = w.run_once().await else {
        panic!("second fetch should complete (was `since` sent?)")
    };
    let reqs = mock.received_requests().await;
    assert_eq!(reqs.len(), 1);
    let since: Vec<_> = reqs[0]
        .url
        .query_pairs()
        .filter(|(k, _)| k == "since")
        .map(|(_, v)| v.into_owned())
        .collect();
    assert_eq!(since, vec![expected_since]);
    assert_eq!(stats.points_written, 2);
    assert_eq!(stats.new_points, 1);
    assert_eq!(item(&db.pool, id).await.status, "completed");

    let series_id = stats.series_id.unwrap();
    assert_eq!(point_count(&db.pool, series_id).await, 3);
    let mut conn = db.pool.get().await.unwrap();
    let feb: Option<BigDecimal> = data_points::table
        .filter(data_points::series_id.eq(series_id))
        .filter(data_points::date.eq(d("2024-02-01")))
        .select(data_points::value)
        .first(&mut conn)
        .await
        .unwrap();
    assert_eq!(feb, Some(BigDecimal::from_str("2.5").unwrap()));
    assert_eq!(attempts(&db.pool, series_id).await.len(), 2);
}

#[tokio::test]
async fn refetching_unchanged_points_writes_nothing() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    let points = [("2024-01-01", Some("1")), ("2024-02-01", None)];
    mock.mount(&Route::get("/series/t5_same"), series_json("Same", &points))
        .await;
    let w = worker(&db.pool, &mock);
    enqueue(&db.pool, SRC.as_str(), "t5_same", JobKind::FetchSeries, 5).await;
    let Some(JobOutcome::Completed(first)) = w.run_once().await else {
        panic!("first fetch should complete")
    };
    assert_eq!((first.points_written, first.new_points), (2, 2));
    let series_id = first.series_id.unwrap();
    let updated_at = |pool: DatabasePool| async move {
        let mut conn = pool.get().await.unwrap();
        data_points::table
            .filter(data_points::series_id.eq(series_id))
            .select(data_points::updated_at)
            .order(data_points::date)
            .load::<chrono::DateTime<Utc>>(&mut conn)
            .await
            .unwrap()
    };
    let before = updated_at(db.pool.clone()).await;

    // Same values again (including the NULL): nothing is rewritten or counted.
    mock.reset().await;
    mock.mount(&Route::get("/series/t5_same"), series_json("Same", &points))
        .await;
    enqueue(&db.pool, SRC.as_str(), "t5_same", JobKind::FetchSeries, 5).await;
    let Some(JobOutcome::Completed(second)) = w.run_once().await else {
        panic!("second fetch should complete")
    };
    assert_eq!((second.points_written, second.new_points), (0, 0));
    assert_eq!(updated_at(db.pool.clone()).await, before);
    assert_eq!(point_count(&db.pool, series_id).await, 2);
}

fn vintage(value: &str, revision: &str) -> FetchedPoint {
    FetchedPoint {
        date: d("2024-01-01"),
        value: Some(BigDecimal::from_str(value).unwrap()),
        revision_date: d(revision),
        is_original_release: false,
    }
}

/// `(revision_date, superseded_on, value)` for every stored vintage of 2024-01-01.
async fn vintages(
    pool: &DatabasePool,
    series_id: Uuid,
) -> Vec<(NaiveDate, Option<NaiveDate>, String)> {
    let mut conn = pool.get().await.unwrap();
    data_points::table
        .filter(data_points::series_id.eq(series_id))
        .filter(data_points::date.eq(d("2024-01-01")))
        .order(data_points::revision_date)
        .select((
            data_points::revision_date,
            data_points::superseded_on,
            data_points::value,
        ))
        .load::<(NaiveDate, Option<NaiveDate>, Option<BigDecimal>)>(&mut conn)
        .await
        .unwrap()
        .into_iter()
        .map(|(r, s, v)| (r, s, v.unwrap().normalized().to_string()))
        .collect()
}

#[tokio::test]
async fn revisions_are_linked_into_vintages_in_any_order() {
    let Some(db) = db().await else { return };
    let fetched = |points| FetchedSeries {
        metadata: None,
        points,
    };
    let write = persist::persist_series(
        &db.pool,
        SRC,
        "t5_vintage",
        &fetched(vec![vintage("1", "2024-02-01"), vintage("3", "2024-04-01")]),
    )
    .await
    .unwrap();
    let series_id = write.series_id;

    // A revision between two stored ones arrives later: the database splits the gap.
    persist::persist_series(
        &db.pool,
        SRC,
        "t5_vintage",
        &fetched(vec![vintage("2", "2024-03-01")]),
    )
    .await
    .unwrap();
    assert_eq!(
        vintages(&db.pool, series_id).await,
        vec![
            (d("2024-02-01"), Some(d("2024-03-01")), "1".to_string()),
            (d("2024-03-01"), Some(d("2024-04-01")), "2".to_string()),
            (d("2024-04-01"), None, "3".to_string()),
        ]
    );

    // "As known on 2024-03-15" picks exactly the vintage in effect that day.
    let mut conn = db.pool.get().await.unwrap();
    let as_of = d("2024-03-15");
    let known: Vec<Option<BigDecimal>> = data_points::table
        .filter(data_points::series_id.eq(series_id))
        .filter(data_points::revision_date.le(as_of))
        .filter(
            data_points::superseded_on
                .is_null()
                .or(data_points::superseded_on.gt(as_of)),
        )
        .select(data_points::value)
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(known, vec![Some(BigDecimal::from(2))]);

    // Deleting a vintage extends the previous one over the gap.
    diesel::delete(
        data_points::table
            .filter(data_points::series_id.eq(series_id))
            .filter(data_points::revision_date.eq(d("2024-03-01"))),
    )
    .execute(&mut conn)
    .await
    .unwrap();
    assert_eq!(
        vintages(&db.pool, series_id).await,
        vec![
            (d("2024-02-01"), Some(d("2024-04-01")), "1".to_string()),
            (d("2024-04-01"), None, "3".to_string()),
        ]
    );

    // Overlapping vintages are impossible even for writes that bypass the trigger's linking.
    let overlap = diesel::update(
        data_points::table
            .filter(data_points::series_id.eq(series_id))
            .filter(data_points::revision_date.eq(d("2024-02-01"))),
    )
    .set(data_points::superseded_on.eq(None::<NaiveDate>))
    .execute(&mut conn)
    .await;
    assert!(
        overlap.is_err(),
        "open-ended old vintage must overlap the current one"
    );
}

#[tokio::test]
async fn server_error_retries_with_counted_attempt_and_backoff() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    mock.mount(&Route::get("/series/t5_err"), Reply::status(500))
        .await;
    // Pre-existing series, so the failed attempt is recorded against it.
    let write = persist::persist_series(&db.pool, SRC, "t5_err", &FetchedSeries::default())
        .await
        .unwrap();
    let w = worker(&db.pool, &mock);
    let id = enqueue(&db.pool, SRC.as_str(), "t5_err", JobKind::FetchSeries, 5).await;

    let before = Utc::now();
    let outcome = w.run_once().await.expect("claimed");
    let JobOutcome::Retrying { error, at } = outcome else {
        panic!("expected Retrying, got {outcome:?}")
    };
    assert_eq!(error.kind(), "transient");
    let it = item(&db.pool, id).await;
    assert_eq!(it.status, "retrying");
    assert_eq!(it.retry_count, 1);
    assert!(it.locked_by.is_none());
    let scheduled = it.scheduled_for.expect("scheduled_for");
    assert_eq!(scheduled, at);
    // policy.backoff(0) = base_backoff = 60s
    assert!(
        scheduled > before + chrono::Duration::seconds(50),
        "{scheduled} vs {before}"
    );
    assert!(scheduled < Utc::now() + chrono::Duration::seconds(70));
    assert!(it.error_message.unwrap().contains("500"));
    // Not due yet, so not claimable.
    assert!(w.run_once().await.is_none());

    assert_eq!(
        attempts(&db.pool, write.series_id).await,
        vec![(false, Some("transient".into()), Some(0))]
    );
    let (.., status) = series_row(&db.pool, "t5_err").await.unwrap();
    assert_eq!(status.as_deref(), Some("failed"));
}

#[tokio::test]
async fn rate_limited_retries_after_retry_after_without_counting() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/series/t5_busy"),
        Reply::status(429).retry_after(120),
    )
    .await;
    let w = worker(&db.pool, &mock);
    let id = enqueue(&db.pool, SRC.as_str(), "t5_busy", JobKind::FetchSeries, 5).await;
    let before = Utc::now();
    let outcome = w.run_once().await.expect("claimed");
    assert!(
        matches!(&outcome, JobOutcome::Retrying { error: CrawlError::RateLimited { retry_after: Some(r) }, .. } if r.as_secs() == 120),
        "{outcome:?}"
    );
    let it = item(&db.pool, id).await;
    assert_eq!(it.status, "retrying");
    assert_eq!(it.retry_count, 0);
    let scheduled = it.scheduled_for.unwrap();
    assert!(scheduled >= before + chrono::Duration::seconds(115));
    assert!(scheduled <= Utc::now() + chrono::Duration::seconds(125));
    // Long Retry-After is not waited out in-process: exactly one request.
    assert_eq!(mock.received_requests().await.len(), 1);
}

#[tokio::test]
async fn not_found_fails_permanently() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await; // nothing mounted: 404
    let w = worker(&db.pool, &mock);
    let id = enqueue(
        &db.pool,
        SRC.as_str(),
        "t5_missing",
        JobKind::FetchSeries,
        5,
    )
    .await;
    let outcome = w.run_once().await.expect("claimed");
    assert!(matches!(outcome, JobOutcome::Failed { .. }), "{outcome:?}");
    let it = item(&db.pool, id).await;
    assert_eq!(it.status, "failed");
    assert_eq!(it.retry_count, 0);
    assert!(it.locked_by.is_none());
    assert!(it.finished_at.is_some());
    assert!(it.error_message.unwrap().contains("not found"));
    assert!(series_row(&db.pool, "t5_missing").await.is_none());
}

#[tokio::test]
async fn unknown_source_and_missing_adapter_fail() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    let w = worker(&db.pool, &mock);
    let bogus = enqueue(&db.pool, "NOT_A_SOURCE", "t5_x", JobKind::FetchSeries, 9).await;
    let unregistered = enqueue(
        &db.pool,
        SourceId::Oecd.as_str(),
        "t5_y",
        JobKind::FetchSeries,
        5,
    )
    .await;

    assert!(matches!(
        w.run_once().await,
        Some(JobOutcome::Failed { .. })
    ));
    let it = item(&db.pool, bogus).await;
    assert_eq!(it.status, "failed");
    assert!(it.error_message.unwrap().contains("unknown source"));

    assert!(matches!(
        w.run_once().await,
        Some(JobOutcome::Failed { .. })
    ));
    let it = item(&db.pool, unregistered).await;
    assert_eq!(it.status, "failed");
    assert!(it.error_message.unwrap().contains("no adapter"));
    assert!(mock.received_requests().await.is_empty());
}

#[tokio::test]
async fn discover_job_upserts_series_metadata() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/catalog"),
        Reply::json(serde_json::json!({"series": [["t5_a", "Series A"], ["t5_b", "Series B"], ["t5_a", "Series A v2"]]})),
    )
    .await;
    let w = worker(&db.pool, &mock);
    let id = enqueue(
        &db.pool,
        SRC.as_str(),
        "catalog",
        JobKind::DiscoverCatalog,
        5,
    )
    .await;
    let outcome = w.run_once().await.expect("claimed");
    let JobOutcome::Completed(stats) = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(stats.metadata_written, 2);
    assert_eq!(item(&db.pool, id).await.status, "completed");

    let source_id = persist::data_source_id(&db.pool, SRC).await.unwrap();
    let mut conn = db.pool.get().await.unwrap();
    let rows: Vec<(String, String, Option<String>, bool, Uuid)> = series_metadata::table
        .filter(series_metadata::external_id.like("t5\\_%"))
        .order(series_metadata::external_id.asc())
        .select((
            series_metadata::external_id,
            series_metadata::title,
            series_metadata::frequency,
            series_metadata::is_active,
            series_metadata::source_id,
        ))
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "t5_a");
    assert_eq!(rows[0].1, "Series A v2");
    assert_eq!(rows[1].1, "Series B");
    assert!(rows
        .iter()
        .all(|r| r.2.as_deref() == Some("Monthly") && r.3 && r.4 == source_id));

    // Re-discovery updates in place.
    mock.reset().await;
    mock.mount(
        &Route::get("/catalog"),
        Reply::json(serde_json::json!({"series": [["t5_b", "Series B renamed"]]})),
    )
    .await;
    enqueue(
        &db.pool,
        SRC.as_str(),
        "catalog",
        JobKind::DiscoverCatalog,
        5,
    )
    .await;
    assert!(matches!(w.run_once().await, Some(JobOutcome::Completed(_))));
    let titles: Vec<String> = series_metadata::table
        .filter(series_metadata::external_id.like("t5\\_%"))
        .order(series_metadata::external_id.asc())
        .select(series_metadata::title)
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(titles, vec!["Series A v2", "Series B renamed"]);
}

async fn wait_until_completed(pool: &DatabasePool, n: i64) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let mut conn = pool.get().await.unwrap();
        let done: i64 = crawl_queue::table
            .filter(crawl_queue::status.eq("completed"))
            .count()
            .get_result(&mut conn)
            .await
            .unwrap();
        drop(conn);
        if done >= n {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out: {done}/{n} completed"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn two_workers_drain_twenty_items_exactly_once() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    Mock::given(method("GET"))
        .and(path_regex("^/series/t5_"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(
                    serde_json::json!({"title": "Many", "points": [["2024-01-01", "1"]]}),
                )
                .set_delay(Duration::from_millis(20)),
        )
        .mount(mock.server())
        .await;
    for i in 0..20 {
        let src = if i % 2 == 0 { SRC } else { OTHER };
        enqueue(
            &db.pool,
            src.as_str(),
            &format!("t5_many_{i:02}"),
            JobKind::FetchSeries,
            5,
        )
        .await;
    }
    let mk = |id: &str| {
        Worker::new(
            ctx(&db.pool),
            registry(&mock),
            WorkerConfig {
                concurrency: 2,
                ..config(id)
            },
        )
    };
    let (w1, w2) = (mk("t5-w1"), mk("t5-w2"));
    tokio::time::timeout(
        Duration::from_secs(90),
        futures::future::join(
            w1.run(wait_until_completed(&db.pool, 20)),
            w2.run(wait_until_completed(&db.pool, 20)),
        ),
    )
    .await
    .expect("workers did not finish");

    let reqs = mock.received_requests().await;
    assert_eq!(reqs.len(), 20, "each item fetched exactly once");
    let paths: HashSet<String> = reqs.iter().map(|r| r.url.path().to_string()).collect();
    assert_eq!(paths.len(), 20);

    let mut conn = db.pool.get().await.unwrap();
    let rows: Vec<(String, Option<String>)> = crawl_queue::table
        .select((crawl_queue::status, crawl_queue::locked_by))
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(rows.len(), 20);
    assert!(rows.iter().all(|(s, l)| s == "completed" && l.is_none()));
    let n_attempts: i64 = crawl_attempts::table
        .inner_join(economic_series::table)
        .filter(economic_series::external_id.like("t5\\_many\\_%"))
        .count()
        .get_result(&mut conn)
        .await
        .unwrap();
    assert_eq!(n_attempts, 20);
}

#[tokio::test]
async fn circuit_breaker_pauses_throttled_source_only() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    // SRC is throttled (long Retry-After, so no in-process waiting); OTHER works.
    for i in 0..3 {
        mock.mount(
            &Route::get(format!("/series/t5_rl_{i}")),
            Reply::status(429).retry_after(60),
        )
        .await;
    }
    for i in 0..2 {
        mock.mount(
            &Route::get(format!("/series/t5_ok_{i}")),
            series_json("ok", &[("2024-01-01", Some("1"))]),
        )
        .await;
    }
    let mut rl = Vec::new();
    for i in 0..3 {
        rl.push(
            enqueue(
                &db.pool,
                SRC.as_str(),
                &format!("t5_rl_{i}"),
                JobKind::FetchSeries,
                10 - i,
            )
            .await,
        );
    }
    for i in 0..2 {
        enqueue(
            &db.pool,
            OTHER.as_str(),
            &format!("t5_ok_{i}"),
            JobKind::FetchSeries,
            1,
        )
        .await;
    }
    let w = Worker::new(
        ctx(&db.pool),
        registry(&mock),
        WorkerConfig {
            pause_after_consecutive: 2,
            pause_for: Duration::from_secs(60),
            ..config("t5-breaker")
        },
    );

    // Two 429s in a row trip the breaker for SRC.
    for _ in 0..2 {
        assert!(matches!(
            w.run_once().await,
            Some(JobOutcome::Retrying { .. })
        ));
    }
    assert_eq!(w.paused_sources(), vec![SRC]);
    // The third SRC item has the highest remaining priority but is skipped; OTHER proceeds.
    for _ in 0..2 {
        assert!(matches!(w.run_once().await, Some(JobOutcome::Completed(_))));
    }
    assert!(
        w.run_once().await.is_none(),
        "paused source must not be claimed"
    );
    let third = item(&db.pool, rl[2]).await;
    assert_eq!(third.status, "pending");
    assert_eq!(mock.received_requests().await.len(), 4);
}

#[tokio::test]
async fn adapter_panic_is_retried_and_worker_survives() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/series/t5_fine_after"),
        series_json("fine", &[("2024-01-01", Some("1"))]),
    )
    .await;
    let panicking = enqueue(&db.pool, SRC.as_str(), "t5_panic", JobKind::FetchSeries, 9).await;
    let fine = enqueue(
        &db.pool,
        SRC.as_str(),
        "t5_fine_after",
        JobKind::FetchSeries,
        1,
    )
    .await;
    let w = worker(&db.pool, &mock);
    tokio::time::timeout(
        Duration::from_secs(30),
        w.run(wait_until_completed(&db.pool, 1)),
    )
    .await
    .expect("worker stalled after panic");

    let it = item(&db.pool, panicking).await;
    assert_eq!(it.status, "retrying");
    assert_eq!(it.retry_count, 1);
    assert!(it.error_message.unwrap().contains("panicked"));
    assert_eq!(item(&db.pool, fine).await.status, "completed");
}

struct FilingHandler(AtomicUsize);

#[async_trait]
impl JobHandler for FilingHandler {
    async fn handle(
        &self,
        _ctx: &CrawlCtx,
        item: &CrawlQueueItem,
    ) -> Result<JobOutcome, CrawlError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        if item.series_id.ends_with("bad") {
            return Err(CrawlError::Parse("bad filing".into()));
        }
        Ok(JobOutcome::Completed(JobStats::default()))
    }
}

#[tokio::test]
async fn extension_handlers_dispatch_by_source_and_kind() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    let handler = Arc::new(FilingHandler(AtomicUsize::new(0)));
    let w =
        worker(&db.pool, &mock).with_handler(SourceId::Sec, JobKind::FetchFiling, handler.clone());

    let ok = enqueue(
        &db.pool,
        "SEC",
        "t5_0000320193-24-000123",
        JobKind::FetchFiling,
        9,
    )
    .await;
    let bad = enqueue(&db.pool, "SEC", "t5_bad", JobKind::FetchFiling, 8).await;
    let unhandled = enqueue(&db.pool, SRC.as_str(), "t5_filing", JobKind::FetchFiling, 7).await;

    assert!(matches!(w.run_once().await, Some(JobOutcome::Completed(_))));
    assert!(matches!(
        w.run_once().await,
        Some(JobOutcome::Failed { .. })
    ));
    assert!(matches!(
        w.run_once().await,
        Some(JobOutcome::Failed { .. })
    ));
    assert_eq!(AtomicUsize::load(&handler.0, Ordering::SeqCst), 2);
    assert_eq!(item(&db.pool, ok).await.status, "completed");
    assert!(item(&db.pool, bad)
        .await
        .error_message
        .unwrap()
        .contains("bad filing"));
    let it = item(&db.pool, unhandled).await;
    assert_eq!(it.status, "failed");
    assert!(it.error_message.unwrap().contains("no handler"));
}

#[tokio::test]
async fn zero_revision_lookback_fetches_since_latest_date() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/series/t5_zero"),
        series_json("Z", &[("2024-05-01", Some("1"))]),
    )
    .await;
    let mut ctx = test_ctx_with(HashMap::from([(
        SRC,
        SourcePolicy {
            revision_lookback: Duration::ZERO,
            ..slow_backoff(SRC)
        },
    )]));
    ctx.pool = db.pool.clone();
    let w = Worker::new(ctx, registry(&mock), config("t5-zero"));
    enqueue(&db.pool, SRC.as_str(), "t5_zero", JobKind::FetchSeries, 5).await;
    assert!(matches!(w.run_once().await, Some(JobOutcome::Completed(_))));
    mock.reset().await;
    mock.mount(
        &Route::get("/series/t5_zero").query("since", "2024-05-01"),
        series_json("Z", &[("2024-05-01", Some("1"))]),
    )
    .await;
    enqueue(&db.pool, SRC.as_str(), "t5_zero", JobKind::FetchSeries, 5).await;
    assert!(
        matches!(w.run_once().await, Some(JobOutcome::Completed(_))),
        "zero lookback should send since = latest stored date"
    );
}

/// Simulates a job that outlives its lease: while it "runs", `release_stuck` takes the item
/// away and another worker claims it.
struct SlowHandler {
    fail: bool,
}

#[async_trait]
impl JobHandler for SlowHandler {
    async fn handle(
        &self,
        ctx: &CrawlCtx,
        item: &CrawlQueueItem,
    ) -> Result<JobOutcome, CrawlError> {
        tokio::time::sleep(Duration::from_millis(20)).await;
        CrawlQueueItem::release_stuck(&ctx.pool, Duration::ZERO)
            .await
            .unwrap();
        CrawlQueueItem::claim_by_id(&ctx.pool, item.id, "t5-other-worker")
            .await
            .unwrap()
            .expect("the released item is claimable by another worker");
        if self.fail {
            Err(CrawlError::Transient("slow and failed".into()))
        } else {
            Ok(JobOutcome::Completed(JobStats::default()))
        }
    }
}

#[tokio::test]
async fn lost_lease_is_reported_and_leaves_new_owner_alone() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    for fail in [false, true] {
        let w = worker(&db.pool, &mock).with_handler(
            SourceId::Sec,
            JobKind::FetchFiling,
            Arc::new(SlowHandler { fail }),
        );
        let id = enqueue(&db.pool, "SEC", "t5_slow", JobKind::FetchFiling, 5).await;
        let outcome = w.run_once().await.expect("claimed");
        match (&outcome, fail) {
            (JobOutcome::LeaseLost { error: None }, false) => {}
            (JobOutcome::LeaseLost { error: Some(e) }, true) => assert!(e.contains("slow")),
            _ => panic!("expected LeaseLost, got {outcome:?}"),
        }
        let it = item(&db.pool, id).await;
        assert_eq!(it.status, "processing", "the new owner's claim stands");
        assert_eq!(it.locked_by.as_deref(), Some("t5-other-worker"));
        assert_eq!(it.retry_count, 1, "only the release counted");
        assert!(it.finished_at.is_none());

        // The new owner finishes normally.
        assert_eq!(
            CrawlQueueItem::complete(&db.pool, id, "t5-other-worker")
                .await
                .unwrap(),
            econ_graph_core::models::LeaseOutcome::Applied
        );
        assert_eq!(item(&db.pool, id).await.status, "completed");
    }
}

#[tokio::test]
async fn maintenance_loop_purges_old_finished_items() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    let old = enqueue(&db.pool, SRC.as_str(), "t5_old", JobKind::FetchSeries, 5).await;
    let recent = enqueue(&db.pool, SRC.as_str(), "t5_recent", JobKind::FetchSeries, 5).await;
    let pending = enqueue(
        &db.pool,
        OTHER.as_str(),
        "t5_pending",
        JobKind::FetchSeries,
        5,
    )
    .await;
    CrawlQueueItem::force_complete(&db.pool, old).await.unwrap();
    CrawlQueueItem::force_fail(&db.pool, recent, "x")
        .await
        .unwrap();
    {
        let mut conn = db.pool.get().await.unwrap();
        diesel::sql_query(
            "UPDATE crawl_queue SET finished_at = NOW() - INTERVAL '20 days', \
                                    created_at = NOW() - INTERVAL '20 days' \
             WHERE id = $1",
        )
        .bind::<diesel::sql_types::Uuid, _>(old)
        .execute(&mut conn)
        .await
        .unwrap();
        // Keep the pending item from being claimed: schedule it in the future.
        diesel::update(crawl_queue::table.find(pending))
            .set(crawl_queue::scheduled_for.eq(Some(Utc::now() + chrono::Duration::hours(1))))
            .execute(&mut conn)
            .await
            .unwrap();
    }
    let w = Worker::new(
        ctx(&db.pool),
        registry(&mock),
        WorkerConfig {
            queue_retention: Some(Duration::from_secs(14 * 86_400)),
            ..config("t5-purger")
        },
    );
    w.run(tokio::time::sleep(Duration::from_millis(300))).await;

    let mut conn = db.pool.get().await.unwrap();
    let left: HashSet<Uuid> = crawl_queue::table
        .select(crawl_queue::id)
        .load::<Uuid>(&mut conn)
        .await
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(left, HashSet::from([recent, pending]));
}

#[test]
fn default_queue_retention_is_14_days() {
    assert_eq!(
        WorkerConfig::default().queue_retention,
        Some(Duration::from_secs(14 * 24 * 3600))
    );
}

#[tokio::test]
async fn release_stuck_runs_on_start() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/series/t5_stuck"),
        series_json("s", &[("2024-01-01", Some("1"))]),
    )
    .await;
    let id = enqueue(&db.pool, SRC.as_str(), "t5_stuck", JobKind::FetchSeries, 5).await;
    // A crashed worker claimed it an hour ago.
    CrawlQueueItem::claim_next(&db.pool, "dead-worker", None)
        .await
        .unwrap()
        .unwrap();
    let mut conn = db.pool.get().await.unwrap();
    diesel::sql_query("UPDATE crawl_queue SET locked_at = NOW() - INTERVAL '2 hours'")
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let w = Worker::new(
        ctx(&db.pool),
        registry(&mock),
        WorkerConfig {
            stuck_after: Duration::from_secs(600),
            ..config("t5-rescuer")
        },
    );
    tokio::time::timeout(
        Duration::from_secs(30),
        w.run(wait_until_completed(&db.pool, 1)),
    )
    .await
    .expect("stuck item was not released");
    assert_eq!(item(&db.pool, id).await.status, "completed");
}
