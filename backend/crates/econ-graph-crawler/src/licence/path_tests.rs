//! The licence gate on every crawl path: the worker's claim, `crawler discover` and `enqueue`
//! (and `fetch`, `refetch`), and the scheduler. Each gated source is refused while its flag is off
//! and allowed once unlocked (as a build with its flag on would be); ungated sources are never
//! affected. The DB-backed tests need `DATABASE_URL` and are skipped without it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDate;
use clap::Parser;
use econ_graph_core::models::{CrawlQueueItem, JobKind};
use econ_graph_core::DatabasePool;

use super::{testing, LICENCE_GATES};
use crate::adapter::{AdapterRegistry, CrawlCtx, DiscoveredSeries, FetchedSeries, SourceAdapter};
use crate::cli::{enqueue_all, format_sources, new_job, Cli, CATALOG_SERIES_ID};
use crate::error::CrawlError;
use crate::scheduler::{RefreshScheduler, SchedulerConfig};
use crate::source::SourceId;
use crate::testkit::{lazy_pool, test_ctx, test_keys};
use crate::worker::{JobOutcome, Worker, WorkerConfig};

fn gated() -> Vec<SourceId> {
    LICENCE_GATES.iter().map(|g| g.source).collect()
}

fn ungated() -> Vec<SourceId> {
    SourceId::ALL
        .into_iter()
        .filter(|s| !gated().contains(s))
        .collect()
}

/// Unlocks every gated source until the guards drop.
fn unlock_all() -> Vec<testing::Unlock> {
    gated().into_iter().map(testing::unlock).collect()
}

fn assert_licence_refusal(source: SourceId, msg: &str) {
    let g = super::gate(source).unwrap();
    assert!(
        msg.contains(&format!("disabled: licence ({})", g.ticket)),
        "{source}: {msg}"
    );
    assert!(msg.contains(&g.ticket_url()), "{source}: {msg}");
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

fn scheduler(sources: Vec<SourceId>) -> RefreshScheduler {
    RefreshScheduler::new(lazy_pool(), sources, SchedulerConfig::default())
}

#[test]
fn scheduler_leaves_out_gated_sources() {
    let s = scheduler(SourceId::ALL.to_vec());
    for source in gated() {
        assert!(!s.discovery_sources().contains(&source), "{source}");
        assert!(!s.refreshable_sources().contains(&source), "{source}");
    }
}

#[test]
fn scheduler_schedules_unlocked_gated_sources() {
    let _unlocked = unlock_all();
    let s = scheduler(SourceId::ALL.to_vec());
    for source in gated() {
        assert!(s.discovery_sources().contains(&source), "{source}");
    }
    // IMF has no static catalog, so it is refreshed as well as discovered once unlocked.
    assert!(s.refreshable_sources().contains(&SourceId::Imf));
}

#[test]
fn scheduler_keeps_every_ungated_source() {
    let s = scheduler(SourceId::ALL.to_vec());
    let discovery = s.discovery_sources();
    for source in ungated().into_iter().filter(|&s| s != SourceId::Sec) {
        assert!(discovery.contains(&source), "{source}");
    }
}

// ---------------------------------------------------------------------------
// CLI: enqueue, discover, fetch, refetch, sources, status
// ---------------------------------------------------------------------------

/// Runs `args`. The refusal comes before the database connection, so the URL is never used; a
/// command that gets past the gate times out connecting to it and fails the test.
async fn run_cli(args: &[&str]) -> anyhow::Result<String> {
    let mut full = vec!["crawler", "--database-url", "postgres://127.0.0.1:1/none"];
    full.extend_from_slice(args);
    let run = Cli::try_parse_from(full).unwrap().execute();
    tokio::time::timeout(Duration::from_secs(5), run)
        .await
        .unwrap_or_else(|_| panic!("{args:?} got past the licence gate"))
}

fn cli_commands(source: &str) -> Vec<Vec<String>> {
    [
        vec!["enqueue", "--source", source, "--series", "X"],
        vec!["discover", "--source", source],
        vec!["fetch", "--source", source, "--series", "X"],
        vec!["refetch", "--source", source],
    ]
    .into_iter()
    .map(|c| c.into_iter().map(str::to_string).collect())
    .collect()
}

#[tokio::test]
async fn cli_refuses_gated_sources() {
    for source in gated() {
        for cmd in cli_commands(source.as_str()) {
            let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
            let err = run_cli(&args).await.unwrap_err().to_string();
            assert_licence_refusal(source, &err);
        }
    }
}

#[tokio::test]
async fn enqueue_all_refuses_a_gated_job_and_inserts_nothing() {
    for source in gated() {
        let jobs = [
            new_job(SourceId::Fred, "X", JobKind::FetchSeries, 5),
            new_job(source, CATALOG_SERIES_ID, JobKind::DiscoverCatalog, 5),
        ];
        // The refusal comes before any query, so the pool is never used.
        let err = enqueue_all(&lazy_pool(), &jobs).await.unwrap_err();
        assert_licence_refusal(source, &err.to_string());
    }
}

#[test]
fn sources_lists_gated_sources_as_disabled_by_licence() {
    let out = format_sources(&AdapterRegistry::new(), &test_keys());
    for line in out.lines().skip(1) {
        let source: SourceId = line.split_whitespace().next().unwrap().parse().unwrap();
        match super::gate(source) {
            Some(g) => assert!(
                line.contains(&format!("disabled: licence ({})", g.ticket)),
                "{line}"
            ),
            None => assert!(
                line.contains("enabled") && !line.contains("licence"),
                "{line}"
            ),
        }
    }
    let _unlocked = unlock_all();
    let out = format_sources(&AdapterRegistry::new(), &test_keys());
    assert!(!out.contains("licence"), "{out}");
}

// ---------------------------------------------------------------------------
// Worker claim
// ---------------------------------------------------------------------------

/// An adapter that counts every upstream call and fails each fetch, so a test can tell whether the
/// worker claimed and dispatched a job.
struct Counting {
    id: SourceId,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl SourceAdapter for Counting {
    fn id(&self) -> SourceId {
        self.id
    }

    fn datasets(&self) -> &[&str] {
        &[]
    }

    async fn discover(&self, _ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(CrawlError::Permanent("counting adapter".into()))
    }

    async fn fetch_series(
        &self,
        _ctx: &CrawlCtx,
        _external_id: &str,
        _since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(CrawlError::Permanent("counting adapter".into()))
    }
}

fn counting_worker(pool: &DatabasePool, calls: &Arc<AtomicUsize>) -> Worker {
    let mut registry = AdapterRegistry::new();
    for id in SourceId::ALL {
        registry.register(Arc::new(Counting {
            id,
            calls: calls.clone(),
        }));
    }
    let mut ctx = test_ctx();
    ctx.pool = pool.clone();
    Worker::new(
        ctx,
        registry,
        WorkerConfig {
            worker_id: "licence-worker".into(),
            concurrency: 1,
            poll_interval: Duration::from_millis(50),
            stuck_after: Duration::from_secs(3600),
            source_filter: None,
            pause_after_consecutive: 0,
            pause_for: Duration::from_secs(60),
            queue_retention: None,
        },
    )
}

async fn queue_db() -> Option<(DatabasePool, tokio::sync::MutexGuard<'static, ()>)> {
    use diesel_async::RunQueryDsl;
    let (url, guard) = crate::testkit::lock_test_db("licence").await?;
    let pool = econ_graph_core::create_pool(&url).await.expect("pool");
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query("DELETE FROM crawl_queue")
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    Some((pool, guard))
}

async fn enqueue(pool: &DatabasePool, source: SourceId, kind: JobKind) -> uuid::Uuid {
    let series = if kind == JobKind::DiscoverCatalog {
        CATALOG_SERIES_ID
    } else {
        "licence_x"
    };
    CrawlQueueItem::enqueue(pool, &new_job(source, series, kind, 10))
        .await
        .unwrap()
        .expect("enqueued")
        .id
}

async fn reload(pool: &DatabasePool, id: uuid::Uuid) -> CrawlQueueItem {
    use diesel::{QueryDsl, SelectableHelper};
    use diesel_async::RunQueryDsl;
    use econ_graph_core::schema::crawl_queue;
    let mut conn = pool.get().await.unwrap();
    crawl_queue::table
        .find(id)
        .select(CrawlQueueItem::as_select())
        .first(&mut conn)
        .await
        .unwrap()
}

/// Jobs queued for a gated source are skipped: never claimed or fetched, never failed or deleted.
/// Once the source is unlocked the same jobs are claimed and fetched.
#[tokio::test]
async fn worker_skips_gated_jobs_until_unlocked() {
    let Some((pool, _guard)) = queue_db().await else {
        return;
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let w = counting_worker(&pool, &calls);
    let mut queued = Vec::new();
    for source in gated() {
        for kind in [JobKind::DiscoverCatalog, JobKind::FetchSeries] {
            queued.push(enqueue(&pool, source, kind).await);
        }
    }

    for _ in 0..3 {
        assert!(w.run_once().await.is_none(), "a gated job was claimed");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for id in &queued {
        let item = reload(&pool, *id).await;
        assert_eq!(item.status, "pending", "{}", item.source);
        assert_eq!(item.retry_count, 0, "{}", item.source);
        assert_eq!(item.locked_by, None, "{}", item.source);
        assert_eq!(item.error_message, None, "{}", item.source);
    }

    // An ungated job behind them is claimed.
    let fred = enqueue(&pool, SourceId::Fred, JobKind::FetchSeries).await;
    assert!(matches!(
        w.run_once().await,
        Some(JobOutcome::Failed { .. })
    ));
    assert_eq!(reload(&pool, fred).await.status, "failed");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    for source in gated() {
        let _unlocked = testing::unlock(source);
        let before = calls.load(Ordering::SeqCst);
        for _ in 0..2 {
            let outcome = w.run_once().await;
            assert!(outcome.is_some(), "{source}: unlocked job not claimed");
        }
        assert!(
            w.run_once().await.is_none(),
            "{source}: another gated source's job was claimed"
        );
        assert_eq!(calls.load(Ordering::SeqCst), before + 2, "{source}");
    }
}

/// With the flags on, `crawler enqueue` and `discover` queue a gated source's jobs as usual, and
/// ungated sources are queued whatever the flags.
#[tokio::test]
async fn cli_enqueues_unlocked_gated_sources() {
    let Some((pool, _guard)) = queue_db().await else {
        return;
    };
    let url = std::env::var("DATABASE_URL").unwrap();
    let run = |args: &[&str]| {
        let mut full = vec!["crawler", "--database-url", url.as_str()];
        full.extend_from_slice(args);
        Cli::try_parse_from(full).unwrap().execute()
    };
    for source in gated() {
        let s = source.as_str();
        let refused = run(&["discover", "--source", s]).await.unwrap_err();
        assert_licence_refusal(source, &refused.to_string());
        let _unlocked = testing::unlock(source);
        let out = run(&["discover", "--source", s]).await.unwrap();
        assert_eq!(out, format!("enqueued discover_catalog for {s}\n"));
        let out = run(&["enqueue", "--source", s, "--series", "A,B"])
            .await
            .unwrap();
        assert_eq!(
            out,
            format!("enqueued 2 of 2 fetch_series job(s) for {s}\n")
        );
    }
    // Ungated sources are never refused.
    for source in ungated() {
        let s = source.as_str();
        let out = run(&["discover", "--source", s]).await.unwrap();
        assert_eq!(out, format!("enqueued discover_catalog for {s}\n"));
    }
    use diesel_async::RunQueryDsl;
    let mut conn = pool.get().await.unwrap();
    #[derive(diesel::QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        n: i64,
    }
    let c: Count = diesel::sql_query("SELECT COUNT(*) AS n FROM crawl_queue")
        .get_result(&mut conn)
        .await
        .unwrap();
    assert_eq!(c.n, (3 * gated().len() + ungated().len()) as i64);
}
