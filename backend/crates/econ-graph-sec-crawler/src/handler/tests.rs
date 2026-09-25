//! Tests for [`SecFilingHandler`] against a mock EDGAR ([`MockSource`]).
//!
//! HTTP-only tests always run. DB-backed tests need `DATABASE_URL` (migrations are applied) and
//! are skipped when it is unset; they serialise on a lock and only touch CIK 0009999901 and
//! `crawl_queue` rows with source `SEC`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::models::{CrawlQueueItem, JobKind};
use econ_graph_crawler::testkit::{
    fast_policy, lazy_pool, test_http_config, test_keys, MockSource, Reply, Route,
};
use econ_graph_crawler::{
    AdapterRegistry, CrawlCtx, CrawlError, HttpConfig, HttpFetcher, JobHandler, JobOutcome,
    SourceId, SourcePolicy, Worker, WorkerConfig,
};
use uuid::Uuid;

use super::*;

const CIK: &str = "0009999901";
const SUBMISSIONS_PATH: &str = "/submissions/CIK0009999901.json";
const SUBMISSIONS: &str = include_str!("../../test_data/sec_mock/submissions_CIK0009999901.json");
const TENK_PATH: &str = "/Archives/edgar/data/9999901/000095017024000001/tstc-20231231_htm.xml";
const TENQ_PATH: &str = "/Archives/edgar/data/9999901/000999990124000002/tstc-20240331_htm.xml";
const XBRL: &str = include_str!("../../test_data/sample_10k.xml");

fn handler_for(mock: &MockSource) -> SecFilingHandler {
    SecFilingHandler::with_endpoints(
        SecFilingHandler::default_config(),
        SecEndpoints::new(mock.base_url(), mock.base_url()),
    )
}

fn ctx(config: HttpConfig, sec: SourcePolicy, pool: DatabasePool) -> CrawlCtx {
    let policies = SourceId::ALL
        .into_iter()
        .map(|id| (id, fast_policy(id)))
        .chain([(SourceId::Sec, sec)])
        .collect::<HashMap<_, _>>();
    CrawlCtx {
        http: HttpFetcher::new(config, policies).expect("fetcher"),
        pool,
        keys: test_keys(),
    }
}

fn fast_ctx(pool: DatabasePool) -> CrawlCtx {
    ctx(test_http_config(), fast_policy(SourceId::Sec), pool)
}

fn item(series_id: &str) -> CrawlQueueItem {
    CrawlQueueItem {
        id: Uuid::new_v4(),
        source: "SEC".into(),
        series_id: series_id.into(),
        priority: 5,
        status: "processing".into(),
        retry_count: 0,
        max_retries: 3,
        error_message: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        scheduled_for: None,
        locked_by: Some("test".into()),
        locked_at: Some(Utc::now()),
        kind: JobKind::FetchFiling.as_str().into(),
        started_at: Some(Utc::now()),
        finished_at: None,
    }
}

// ---------------------------------------------------------------------------
// HTTP-only (no database)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn requests_carry_sec_user_agent_and_404_is_not_found() {
    let mock = MockSource::start().await;
    mock.mount_expect(&Route::get(SUBMISSIONS_PATH), Reply::status(404), 1)
        .await;
    // Production HTTP settings: the default User-Agent includes a contact address.
    let ctx = ctx(
        HttpConfig::default(),
        fast_policy(SourceId::Sec),
        lazy_pool(),
    );

    // Unpadded CIK in the queue item is normalized in the URL.
    let err = handler_for(&mock)
        .handle(&ctx, &item("9999901"))
        .await
        .unwrap_err();
    assert!(matches!(err, CrawlError::NotFound(_)), "got {err:?}");

    let reqs = mock.received_requests().await;
    assert_eq!(reqs.len(), 1);
    let ua = reqs[0]
        .headers
        .get("user-agent")
        .expect("User-Agent header")
        .to_str()
        .unwrap();
    assert!(ua.starts_with("EconGraph/"), "unexpected User-Agent {ua}");
    assert!(ua.contains('@'), "SEC requires a contact address: {ua}");
}

#[tokio::test]
async fn http_429_is_rate_limited() {
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get(SUBMISSIONS_PATH),
        Reply::status(429).retry_after(60),
    )
    .await;
    let err = handler_for(&mock)
        .handle(&fast_ctx(lazy_pool()), &item(CIK))
        .await
        .unwrap_err();
    assert!(matches!(err, CrawlError::RateLimited { .. }), "got {err:?}");
    assert_eq!(err.retry_after(), Some(Duration::from_secs(60)));
}

#[tokio::test]
async fn http_500_is_transient_and_malformed_json_is_parse() {
    let mock = MockSource::start().await;
    mock.mount(&Route::get(SUBMISSIONS_PATH), Reply::status(500))
        .await;
    let err = handler_for(&mock)
        .handle(&fast_ctx(lazy_pool()), &item(CIK))
        .await
        .unwrap_err();
    assert!(matches!(err, CrawlError::Transient(_)), "got {err:?}");

    mock.reset().await;
    mock.mount(
        &Route::get(SUBMISSIONS_PATH),
        Reply::json_str("{\"nope\": 1}"),
    )
    .await;
    let err = handler_for(&mock)
        .handle(&fast_ctx(lazy_pool()), &item(CIK))
        .await
        .unwrap_err();
    assert!(matches!(err, CrawlError::Parse(_)), "got {err:?}");
}

#[tokio::test]
async fn invalid_cik_is_permanent_without_requests() {
    let mock = MockSource::start().await;
    for bad in ["", "not-a-cik", "0000000000", "12345678901"] {
        let err = handler_for(&mock)
            .handle(&fast_ctx(lazy_pool()), &item(bad))
            .await
            .unwrap_err();
        assert!(matches!(err, CrawlError::Permanent(_)), "{bad:?}: {err:?}");
    }
    assert!(mock.received_requests().await.is_empty());
}

#[tokio::test]
async fn requests_wait_on_the_shared_sec_rate_limiter() {
    let mock = MockSource::start().await;
    mock.mount(&Route::get(SUBMISSIONS_PATH), Reply::status(404))
        .await;
    // 4 req/s, burst 1: four requests need at least three 250 ms intervals.
    let slow = SourcePolicy {
        requests_per_second: 4.0,
        burst: 1,
        ..fast_policy(SourceId::Sec)
    };
    let ctx = ctx(test_http_config(), slow, lazy_pool());
    let handler = handler_for(&mock);
    let started = Instant::now();
    for _ in 0..4 {
        let err = handler.handle(&ctx, &item(CIK)).await.unwrap_err();
        assert!(matches!(err, CrawlError::NotFound(_)));
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(700),
        "4 requests at 4 req/s finished in {elapsed:?}: limiter not applied"
    );
    assert_eq!(mock.received_requests().await.len(), 4);
}

// ---------------------------------------------------------------------------
// DB-backed
// ---------------------------------------------------------------------------

static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

struct Db {
    pool: DatabasePool,
    _guard: tokio::sync::MutexGuard<'static, ()>,
}

async fn db() -> Option<Db> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed SEC handler test");
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
            "DELETE FROM crawl_queue WHERE source = 'SEC'",
            "DELETE FROM companies WHERE cik = '0009999901'",
        ] {
            diesel::sql_query(sql).execute(&mut conn).await.unwrap();
        }
    }
    Some(Db {
        pool,
        _guard: guard,
    })
}

async fn stored_filings(pool: &DatabasePool) -> Vec<(String, String)> {
    use econ_graph_core::schema::{companies, financial_statements as fs};
    let mut conn = pool.get().await.unwrap();
    let mut rows: Vec<(String, String)> = fs::table
        .inner_join(companies::table)
        .filter(companies::cik.eq(CIK))
        .select((fs::accession_number, fs::form_type))
        .load(&mut conn)
        .await
        .unwrap();
    rows.sort();
    rows
}

async fn mount_company(mock: &MockSource, xbrl_times: u64) {
    mock.mount(&Route::get(SUBMISSIONS_PATH), Reply::json_str(SUBMISSIONS))
        .await;
    mock.mount_expect(&Route::get(TENK_PATH), Reply::xml(XBRL), xbrl_times)
        .await;
    mock.mount_expect(&Route::get(TENQ_PATH), Reply::xml(XBRL), xbrl_times)
        .await;
}

async fn exec(pool: &DatabasePool, sql: &str) {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(sql).execute(&mut conn).await.unwrap();
}

/// Inserts the company under a stale name plus `financial_statements` rows for `accessions`
/// directly in SQL.
async fn prestore(pool: &DatabasePool, accessions: &[&str]) {
    exec(
        pool,
        "INSERT INTO companies (cik, name) VALUES ('0009999901', 'Old Name')",
    )
    .await;
    for acc in accessions {
        exec(
            pool,
            &format!(
                "INSERT INTO financial_statements (company_id, filing_type, form_type, \
                 accession_number, filing_date, period_end_date, fiscal_year, document_url) \
                 SELECT id, '10-K', '10-K', '{acc}', '2024-01-01', '2023-12-31', 2023, 'test' \
                 FROM companies WHERE cik = '0009999901'"
            ),
        )
        .await;
    }
}

async fn company_names(pool: &DatabasePool) -> Vec<String> {
    use econ_graph_core::schema::companies;
    let mut conn = pool.get().await.unwrap();
    companies::table
        .filter(companies::cik.eq(CIK))
        .select(companies::name)
        .load(&mut conn)
        .await
        .unwrap()
}

#[tokio::test]
async fn rerun_skips_stored_filings_without_downloading() {
    let Some(db) = db().await else { return };
    prestore(&db.pool, &["0000950170-24-000001", "0009999901-24-000002"]).await;
    let mock = MockSource::start().await;
    // Both filings are already stored: neither instance document may be requested.
    mount_company(&mock, 0).await;
    let ctx = fast_ctx(db.pool.clone());

    // Unpadded and padded CIKs address the same company row.
    for cik in ["9999901", CIK] {
        let out = handler_for(&mock)
            .handle(&ctx, &item(cik))
            .await
            .expect("run");
        assert!(
            matches!(out, JobOutcome::Completed(ref s) if s.points_written == 0),
            "got {out:?}"
        );
    }
    assert_eq!(stored_filings(&db.pool).await.len(), 2);
    // Upserted by CIK: one row, refreshed from the submissions document.
    assert_eq!(company_names(&db.pool).await, vec!["Testco Holdings Inc."]);
}

#[tokio::test]
async fn retryable_filing_error_fails_the_job() {
    let Some(db) = db().await else { return };
    prestore(&db.pool, &["0000950170-24-000001"]).await;
    let mock = MockSource::start().await;
    mock.mount(&Route::get(SUBMISSIONS_PATH), Reply::json_str(SUBMISSIONS))
        .await;
    mock.mount_expect(&Route::get(TENK_PATH), Reply::xml(XBRL), 0)
        .await;
    mock.mount(&Route::get(TENQ_PATH), Reply::status(500)).await;

    let err = handler_for(&mock)
        .handle(&fast_ctx(db.pool.clone()), &item(CIK))
        .await
        .unwrap_err();
    assert!(matches!(err, CrawlError::Transient(_)), "got {err:?}");
    assert_eq!(stored_filings(&db.pool).await.len(), 1);
}

#[tokio::test]
async fn non_retryable_filing_error_still_completes() {
    let Some(db) = db().await else { return };
    prestore(&db.pool, &["0000950170-24-000001"]).await;
    let mock = MockSource::start().await;
    mock.mount(&Route::get(SUBMISSIONS_PATH), Reply::json_str(SUBMISSIONS))
        .await;
    mock.mount(&Route::get(TENQ_PATH), Reply::status(404)).await;

    let out = handler_for(&mock)
        .handle(&fast_ctx(db.pool.clone()), &item(CIK))
        .await
        .expect("a missing instance document is not a job failure");
    assert!(
        matches!(out, JobOutcome::Completed(ref s) if s.points_written == 0),
        "got {out:?}"
    );
}

#[tokio::test]
async fn worker_runs_enqueued_fetch_filing_job() {
    let Some(db) = db().await else { return };
    prestore(&db.pool, &["0000950170-24-000001", "0009999901-24-000002"]).await;
    let mock = MockSource::start().await;
    mount_company(&mock, 0).await;

    let (enqueued, active) = enqueue_filings(&db.pool, &["9999901".to_string()], 5)
        .await
        .unwrap();
    assert_eq!((enqueued, active), (1, 0));
    // Same company, padded: already active, not enqueued twice.
    let (enqueued, active) = enqueue_filings(&db.pool, &[CIK.to_string()], 5)
        .await
        .unwrap();
    assert_eq!((enqueued, active), (0, 1));

    let worker = Worker::new(
        fast_ctx(db.pool.clone()),
        AdapterRegistry::new(),
        WorkerConfig {
            worker_id: "a11-test".into(),
            concurrency: 1,
            source_filter: Some(vec![SourceId::Sec]),
            ..WorkerConfig::default()
        },
    )
    .with_handler(
        SourceId::Sec,
        JobKind::FetchFiling,
        Arc::new(handler_for(&mock)),
    );

    let outcome = worker.run_once().await.expect("an item was due");
    assert!(
        matches!(outcome, JobOutcome::Completed(_)),
        "got {outcome:?}"
    );
    assert!(worker.run_once().await.is_none(), "queue drained");

    use econ_graph_core::schema::crawl_queue::dsl as q;
    let mut conn = db.pool.get().await.unwrap();
    let rows: Vec<(String, String, String, Option<String>)> = q::crawl_queue
        .filter(q::source.eq("SEC"))
        .select((q::series_id, q::kind, q::status, q::locked_by))
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![(
            CIK.to_string(),
            "fetch_filing".to_string(),
            "completed".to_string(),
            None
        )]
    );
    assert_eq!(mock.received_requests().await.len(), 1, "submissions only");
}

#[tokio::test]
async fn worker_retries_rate_limited_fetch_filing_job() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get(SUBMISSIONS_PATH),
        Reply::status(429).retry_after(60),
    )
    .await;
    enqueue_filings(&db.pool, &[CIK.to_string()], 5)
        .await
        .unwrap();
    let worker = Worker::new(
        fast_ctx(db.pool.clone()),
        AdapterRegistry::new(),
        WorkerConfig {
            worker_id: "a11-test".into(),
            source_filter: Some(vec![SourceId::Sec]),
            ..WorkerConfig::default()
        },
    )
    .with_handler(
        SourceId::Sec,
        JobKind::FetchFiling,
        Arc::new(handler_for(&mock)),
    );
    let outcome = worker.run_once().await.expect("an item was due");
    assert!(
        matches!(
            outcome,
            JobOutcome::Retrying {
                error: CrawlError::RateLimited { .. },
                ..
            }
        ),
        "got {outcome:?}"
    );
}

/// The full download-and-store path through `XbrlStorage::store_xbrl_file`.
#[tokio::test]
async fn crawls_company_and_stores_filings() {
    let Some(db) = db().await else { return };
    let mock = MockSource::start().await;
    mount_company(&mock, 1).await;
    let ctx = fast_ctx(db.pool.clone());
    let handler = handler_for(&mock);

    let first = handler
        .handle(&ctx, &item("9999901"))
        .await
        .expect("first run");
    let JobOutcome::Completed(stats) = first else {
        panic!("expected Completed, got {first:?}")
    };
    assert_eq!(stats.points_written, 2, "10-K and 10-Q stored, 8-K skipped");
    assert_eq!(stats.latest_date, NaiveDate::from_ymd_opt(2024, 5, 2));
    assert_eq!(
        stored_filings(&db.pool).await,
        vec![
            ("0000950170-24-000001".to_string(), "10-K".to_string()),
            ("0009999901-24-000002".to_string(), "10-Q".to_string()),
        ]
    );
    // Re-run: nothing downloaded (mount_expect(.., 1) above) or stored again.
    let second = handler.handle(&ctx, &item(CIK)).await.expect("second run");
    assert!(matches!(second, JobOutcome::Completed(ref s) if s.points_written == 0));
    assert_eq!(stored_filings(&db.pool).await.len(), 2);
}
