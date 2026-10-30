//! DB-backed tests of chunked catalog discovery, through the real worker and queue, with a test
//! adapter that opts in. Skipped when `DATABASE_URL` is unset; they hold the crate-wide DB test
//! lock. Every row they create uses external ids or dataset codes prefixed `t745_`; `crawl_queue`
//! and `discovery_runs` are emptied before each test.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDate;
use diesel::sql_types::{BigInt, Bool, Integer, Nullable, Text, Uuid as SqlUuid};
use diesel::QueryableByName;
use diesel_async::{AsyncConnection, RunQueryDsl};
use econ_graph_core::models::{CrawlQueueItem, JobKind, NewCrawlQueueItem};
use econ_graph_core::DatabasePool;
use uuid::Uuid;

use crate::adapter::{
    AdapterRegistry, CrawlCtx, DiscoveredSeries, Discovery, FetchedSeries, SourceAdapter,
};
use crate::dataset::{parse_dataset_file, DatasetCatalog, SeriesDataset};
use crate::discovery_run::{
    self, ChunkCommit, ChunkDiscovery, ChunkedDiscovery, DiscoveryUnit, Enumeration,
};
use crate::error::CrawlError;
use crate::http::Validators;
use crate::persist;
use crate::source::SourceId;
use crate::testkit::{fast_policy, test_ctx_with};
use crate::worker::{JobOutcome, Worker, WorkerConfig};

const SRC: SourceId = SourceId::Bea;
const FLAT: &str = "t745_flat";
const CATALOG_URL: &str = "https://example.test/t745/catalog";

struct Db {
    pool: DatabasePool,
    _guard: tokio::sync::MutexGuard<'static, ()>,
}

async fn exec(pool: &DatabasePool, sql: &str) {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(sql).execute(&mut conn).await.unwrap();
}

fn catalog() -> DatasetCatalog {
    let mut c = DatasetCatalog::empty();
    c.insert(
        SRC,
        &[FLAT],
        parse_dataset_file("[[dataset]]\ncode = \"t745_flat\"\nname = \"Chunked test\"\n").unwrap(),
    )
    .unwrap();
    c
}

async fn db() -> Option<Db> {
    let (url, guard) = crate::testkit::lock_test_db("chunked discovery").await?;
    let pool = econ_graph_core::create_pool(&url).await.expect("pool");
    for sql in [
        "DELETE FROM crawl_queue",
        "DELETE FROM discovery_runs",
        "DELETE FROM series_metadata WHERE external_id LIKE 't745\\_%'",
        "DELETE FROM reference_file_cache WHERE url LIKE '%/t745/%'",
    ] {
        exec(&pool, sql).await;
    }
    persist::sync_datasets(&pool, &catalog()).await.unwrap();
    Some(Db {
        pool,
        _guard: guard,
    })
}

// ---------------------------------------------------------------------------
// Test adapter
// ---------------------------------------------------------------------------

/// Lists `catalog` as one unit per id. Ids starting `t745_missing` are curated entries the
/// source no longer lists. `errors` queues errors per unit key: a chunk containing such a unit
/// fails with the next one. `gate`, when set, makes the next `discover_units` call wait for a
/// permit; `barrier`, when set, makes every call wait for the others.
struct ChunkedAdapter {
    catalog: Mutex<Vec<String>>,
    chunked: Mutex<Option<ChunkedDiscovery>>,
    errors: Mutex<HashMap<String, VecDeque<CrawlError>>>,
    calls: Mutex<Vec<Vec<String>>>,
    missing: Mutex<Vec<String>>,
    gate: Mutex<Option<Arc<tokio::sync::Semaphore>>>,
    barrier: Mutex<Option<Arc<tokio::sync::Barrier>>>,
    in_flight: AtomicUsize,
}

impl ChunkedAdapter {
    fn new(ids: &[&str], units_per_chunk: usize) -> Arc<Self> {
        Arc::new(Self {
            catalog: Mutex::new(ids.iter().map(|s| (*s).to_string()).collect()),
            chunked: Mutex::new(Some(ChunkedDiscovery {
                units_per_chunk,
                selection_fingerprint: Some("rules-v1".into()),
            })),
            errors: Mutex::new(HashMap::new()),
            calls: Mutex::new(Vec::new()),
            missing: Mutex::new(Vec::new()),
            gate: Mutex::new(None),
            barrier: Mutex::new(None),
            in_flight: AtomicUsize::new(0),
        })
    }

    fn fail_unit(&self, key: &str, errors: Vec<CrawlError>) {
        self.errors
            .lock()
            .unwrap()
            .insert(key.to_string(), errors.into());
    }

    /// How many `discover_units` calls included `key`.
    fn calls_with(&self, key: &str) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.iter().any(|k| k == key))
            .count()
    }

    fn units(&self) -> Vec<DiscoveryUnit> {
        self.catalog
            .lock()
            .unwrap()
            .iter()
            .map(|id| {
                DiscoveryUnit::with_listing(
                    id,
                    serde_json::json!({ "title": format!("Series {id}") }),
                )
            })
            .collect()
    }

    fn detail(units: &[DiscoveryUnit]) -> ChunkDiscovery {
        let mut out = ChunkDiscovery::default();
        for unit in units {
            if unit.key.starts_with("t745_missing") {
                out.curated_missing.push(unit.key.clone());
                continue;
            }
            let title = unit
                .listing
                .as_ref()
                .and_then(|l| l["title"].as_str())
                .unwrap_or_default()
                .to_string();
            out.found.push(DiscoveredSeries {
                external_id: unit.key.clone(),
                title,
                description: Some(format!("About {}", unit.key)),
                units: Some("Index".into()),
                frequency: Some("Monthly".into()),
                data_url: Some(format!("https://example.test/t745/{}", unit.key)),
                dataset: SeriesDataset::new(FLAT, Vec::<(String, String)>::new()),
            });
        }
        out
    }

    fn validator() -> (String, Validators) {
        (
            CATALOG_URL.to_string(),
            Validators {
                version: Some("catalog-v1".into()),
                ..Validators::default()
            },
        )
    }
}

#[async_trait]
impl SourceAdapter for ChunkedAdapter {
    fn id(&self) -> SourceId {
        SRC
    }

    fn datasets(&self) -> &[&str] {
        &[FLAT]
    }

    fn discovery_is_complete(&self) -> bool {
        true
    }

    fn retirement_scope_prefix(&self) -> Option<&str> {
        Some("t745_")
    }

    fn curated_missing(&self) -> Vec<String> {
        self.missing.lock().unwrap().clone()
    }

    fn chunked_discovery(&self) -> Option<ChunkedDiscovery> {
        self.chunked.lock().unwrap().clone()
    }

    async fn discover(&self, _: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let detail = Self::detail(&self.units());
        *self.missing.lock().unwrap() = detail.curated_missing;
        Ok(detail.found)
    }

    async fn discover_if_changed(&self, ctx: &CrawlCtx) -> Result<Discovery, CrawlError> {
        Ok(Discovery::Changed {
            found: self.discover(ctx).await?,
            validator: Some(Self::validator()),
        })
    }

    async fn enumerate_discovery(&self, _: &CrawlCtx) -> Result<Enumeration, CrawlError> {
        Ok(Enumeration::Units {
            units: self.units(),
            validator: Some(Self::validator()),
            curated_missing: Vec::new(),
        })
    }

    async fn discover_units(
        &self,
        _: &CrawlCtx,
        units: &[DiscoveryUnit],
    ) -> Result<ChunkDiscovery, CrawlError> {
        self.calls
            .lock()
            .unwrap()
            .push(units.iter().map(|u| u.key.clone()).collect());
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let gate = self.gate.lock().unwrap().take();
        if let Some(gate) = gate {
            let _permit = gate.acquire().await.unwrap();
        }
        let barrier = self.barrier.lock().unwrap().clone();
        if let Some(barrier) = barrier {
            barrier.wait().await;
        }
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        for unit in units {
            if let Some(e) = self
                .errors
                .lock()
                .unwrap()
                .get_mut(&unit.key)
                .and_then(VecDeque::pop_front)
            {
                return Err(e);
            }
        }
        Ok(Self::detail(units))
    }

    async fn fetch_series(
        &self,
        _: &CrawlCtx,
        _: &str,
        _: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        Err(CrawlError::Permanent("not used".into()))
    }
}

fn worker(pool: &DatabasePool, adapter: &Arc<ChunkedAdapter>, id: &str) -> Worker {
    let mut ctx = test_ctx_with(HashMap::from([(SRC, fast_policy(SRC))]));
    ctx.pool = pool.clone();
    let mut registry = AdapterRegistry::new();
    registry.register(adapter.clone());
    let config = WorkerConfig {
        worker_id: id.to_string(),
        concurrency: 1,
        poll_interval: Duration::from_millis(20),
        stuck_after: Duration::from_secs(3600),
        source_filter: None,
        pause_after_consecutive: 0,
        pause_for: Duration::from_secs(60),
        queue_retention: None,
    };
    Worker::new(ctx, registry, config).with_datasets(catalog())
}

async fn enqueue_discovery(pool: &DatabasePool, priority: i32) {
    CrawlQueueItem::enqueue(
        pool,
        &NewCrawlQueueItem {
            source: SRC.as_str().to_string(),
            series_id: "catalog".into(),
            priority,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::DiscoverCatalog.as_str().to_string(),
        },
    )
    .await
    .unwrap()
    .expect("no active discovery job");
}

/// Runs jobs until nothing is pending, waiting out short retry backoffs.
async fn drain(w: &Worker) -> Vec<JobOutcome> {
    let mut outcomes = Vec::new();
    for _ in 0..500 {
        if let Some(outcome) = w.run_once().await {
            outcomes.push(outcome);
            continue;
        }
        if count(
            &w.ctx.pool,
            "SELECT count(*) AS n FROM crawl_queue WHERE status IN ('pending', 'retrying')",
        )
        .await
            == 0
        {
            return outcomes;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("queue did not drain");
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    n: i64,
}

async fn count(pool: &DatabasePool, sql: &str) -> i64 {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(sql)
        .get_result::<Count>(&mut conn)
        .await
        .unwrap()
        .n
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct Run {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    state: String,
    #[diesel(sql_type = Integer)]
    chunks_total: i32,
    #[diesel(sql_type = Integer)]
    chunks_done: i32,
    #[diesel(sql_type = Integer)]
    chunks_failed: i32,
    #[diesel(sql_type = Bool)]
    catalog_unchanged: bool,
}

/// Runs, oldest first.
async fn runs(pool: &DatabasePool) -> Vec<Run> {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(
        "SELECT id, state::text AS state, chunks_total, chunks_done, chunks_failed, \
             catalog_unchanged \
         FROM discovery_runs ORDER BY started_at, id",
    )
    .load(&mut conn)
    .await
    .unwrap()
}

async fn finalize_jobs(pool: &DatabasePool, run: Uuid) -> i64 {
    count(
        pool,
        &format!(
            "SELECT count(*) AS n FROM crawl_queue WHERE kind = 'finalize_discovery' \
             AND series_id = '{run}'"
        ),
    )
    .await
}

/// The discovered catalog as stored: everything discovery writes except timestamps.
#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct MetadataRow {
    #[diesel(sql_type = Text)]
    external_id: String,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    units: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    frequency: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    data_url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    dataset: Option<String>,
    #[diesel(sql_type = Text)]
    dimensions: String,
    #[diesel(sql_type = Bool)]
    is_active: bool,
}

async fn metadata(pool: &DatabasePool) -> Vec<MetadataRow> {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(
        "SELECT sm.external_id, sm.title, sm.description, sm.units, sm.frequency, sm.data_url, \
             d.code AS dataset, COALESCE(sm.dimensions::text, '') AS dimensions, sm.is_active \
         FROM series_metadata sm LEFT JOIN datasets d ON d.id = sm.dataset_id \
         WHERE sm.external_id LIKE 't745\\_%' ORDER BY sm.external_id",
    )
    .load(&mut conn)
    .await
    .unwrap()
}

/// A previously discovered series the current catalog no longer lists.
async fn seed_stale(pool: &DatabasePool) {
    let source_id = persist::data_source_id(pool, SRC).await.unwrap();
    exec(
        pool,
        &format!(
            "INSERT INTO series_metadata (source_id, external_id, title, is_active) \
             VALUES ('{source_id}', 't745_stale', 'Stale', TRUE)"
        ),
    )
    .await;
}

async fn is_active(pool: &DatabasePool, external_id: &str) -> Option<bool> {
    metadata(pool)
        .await
        .into_iter()
        .find(|m| m.external_id == external_id)
        .map(|m| m.is_active)
}

async fn stored_validator(pool: &DatabasePool) -> Option<Validators> {
    persist::url_validators(pool, SRC, CATALOG_URL)
        .await
        .unwrap()
}

const IDS: [&str; 7] = [
    "t745_a", "t745_b", "t745_c", "t745_d", "t745_e", "t745_f", "t745_g",
];

// ---------------------------------------------------------------------------
// Acceptance cases
// ---------------------------------------------------------------------------

/// A run with several chunks finalizes once and stores exactly what single-job discovery stores
/// for the same catalog: the same metadata, the same validator, the same retirement. A second
/// finalize job for the completed run commits nothing.
#[tokio::test]
async fn chunked_run_finalizes_once_with_the_single_job_result() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let adapter = ChunkedAdapter::new(&IDS, 3);

    // Single-job discovery of the same catalog, for comparison.
    *adapter.chunked.lock().unwrap() = None;
    seed_stale(p).await;
    let w = worker(p, &adapter, "t745-w1");
    enqueue_discovery(p, 5).await;
    drain(&w).await;
    assert!(
        runs(p).await.is_empty(),
        "single-job discovery starts no run"
    );
    let single = metadata(p).await;
    assert_eq!(single.len(), IDS.len() + 1);
    assert_eq!(is_active(p, "t745_stale").await, Some(false));
    assert_eq!(
        stored_validator(p).await.and_then(|v| v.version).as_deref(),
        Some("catalog-v1")
    );

    // Start over and discover in chunks.
    exec(p, "DELETE FROM crawl_queue").await;
    exec(
        p,
        "DELETE FROM series_metadata WHERE external_id LIKE 't745\\_%'",
    )
    .await;
    exec(
        p,
        "DELETE FROM reference_file_cache WHERE url LIKE '%/t745/%'",
    )
    .await;
    seed_stale(p).await;
    *adapter.chunked.lock().unwrap() = Some(ChunkedDiscovery {
        units_per_chunk: 3,
        selection_fingerprint: Some("rules-v1".into()),
    });
    adapter.calls.lock().unwrap().clear();
    enqueue_discovery(p, 4).await;

    // Enumeration queues three chunk jobs at the discovery job's priority and writes nothing.
    assert!(matches!(w.run_once().await, Some(JobOutcome::Completed(_))));
    let run = runs(p).await.pop().unwrap();
    assert_eq!((run.state.as_str(), run.chunks_total), ("running", 3));
    assert_eq!(
        count(
            p,
            "SELECT count(*) AS n FROM crawl_queue WHERE kind = 'discover_chunk' \
             AND status = 'pending' AND priority = 4"
        )
        .await,
        3
    );
    assert_eq!(
        metadata(p).await.len(),
        1,
        "nothing written before finalize"
    );
    assert_eq!(
        stored_validator(p).await,
        None,
        "validator waits for finalize"
    );

    drain(&w).await;
    let run = runs(p).await.pop().unwrap();
    assert_eq!(
        (run.state.as_str(), run.chunks_done, run.chunks_failed),
        ("complete", 3, 0)
    );
    assert_eq!(finalize_jobs(p, run.id).await, 1);
    let mut sizes: Vec<usize> = adapter.calls.lock().unwrap().iter().map(Vec::len).collect();
    sizes.sort_unstable();
    assert_eq!(sizes, vec![1, 3, 3]);
    assert_eq!(metadata(p).await, single);
    assert_eq!(
        stored_validator(p).await.and_then(|v| v.version).as_deref(),
        Some("catalog-v1")
    );
    assert_eq!(
        count(p, "SELECT count(*) AS n FROM discovery_run_chunks").await,
        0,
        "chunk results are deleted at finalize"
    );

    // A stray second finalize job for the completed run is a no-op.
    exec(
        p,
        "UPDATE series_metadata SET title = 'edited' WHERE external_id = 't745_a'",
    )
    .await;
    exec(
        p,
        &format!(
            "INSERT INTO crawl_queue (source, series_id, kind) \
             VALUES ('{}', '{}', 'finalize_discovery')",
            SRC.as_str(),
            run.id
        ),
    )
    .await;
    assert!(matches!(w.run_once().await, Some(JobOutcome::Completed(_))));
    assert_eq!(
        metadata(p)
            .await
            .into_iter()
            .find(|m| m.external_id == "t745_a")
            .unwrap()
            .title,
        "edited"
    );
}

/// A transient failure retries only its own chunk; the run still completes.
#[tokio::test]
async fn transient_chunk_failure_retries_only_that_chunk() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let adapter = ChunkedAdapter::new(&IDS, 3);
    adapter.fail_unit("t745_d", vec![CrawlError::Transient("upstream 503".into())]);
    let w = worker(p, &adapter, "t745-w1");
    enqueue_discovery(p, 5).await;
    let outcomes = drain(&w).await;
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| matches!(o, JobOutcome::Retrying { .. }))
            .count(),
        1
    );
    assert_eq!(adapter.calls_with("t745_a"), 1);
    assert_eq!(
        adapter.calls_with("t745_d"),
        2,
        "the failed chunk ran twice"
    );
    assert_eq!(adapter.calls_with("t745_g"), 1);
    let run = runs(p).await.pop().unwrap();
    assert_eq!((run.state.as_str(), run.chunks_done), ("complete", 3));
    assert_eq!(metadata(p).await.len(), IDS.len());
}

/// A chunk that fails for good fails the run: nothing is committed, nothing retired, no
/// validator stored, and the run's other queued chunks are cancelled.
#[tokio::test]
async fn permanently_failed_chunk_fails_the_run_and_retires_nothing() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    seed_stale(p).await;
    let adapter = ChunkedAdapter::new(&IDS, 3);
    adapter.fail_unit("t745_a", vec![CrawlError::Permanent("gone".into())]);
    let w = worker(p, &adapter, "t745-w1");
    enqueue_discovery(p, 5).await;
    drain(&w).await;
    let run = runs(p).await.pop().unwrap();
    assert_eq!(run.state, "failed");
    assert_eq!(run.chunks_failed, 1);
    assert_eq!(finalize_jobs(p, run.id).await, 0);
    assert_eq!(metadata(p).await.len(), 1, "no discovered series written");
    assert_eq!(
        is_active(p, "t745_stale").await,
        Some(true),
        "nothing retired"
    );
    assert_eq!(stored_validator(p).await, None);
    assert_eq!(
        count(
            p,
            "SELECT count(*) AS n FROM crawl_queue WHERE kind = 'discover_chunk' \
             AND status IN ('pending', 'retrying', 'processing')"
        )
        .await,
        0
    );

    // Retries exhausted by transient errors fail the run the same way.
    adapter.fail_unit("t745_g", vec![CrawlError::Transient("503".into()); 10]);
    enqueue_discovery(p, 5).await;
    drain(&w).await;
    let run = runs(p).await.pop().unwrap();
    assert_eq!(run.state, "failed");
    assert_eq!(is_active(p, "t745_stale").await, Some(true));
}

/// A curated entry missing upstream in any chunk keeps retirement off, as for single-job
/// discovery; the run still completes and writes what it found.
#[tokio::test]
async fn curated_entry_missing_in_a_chunk_retires_nothing() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    seed_stale(p).await;
    let adapter = ChunkedAdapter::new(&["t745_a", "t745_b", "t745_missing_c", "t745_d"], 2);
    let w = worker(p, &adapter, "t745-w1");
    enqueue_discovery(p, 5).await;
    drain(&w).await;
    assert_eq!(runs(p).await.pop().unwrap().state, "complete");
    assert_eq!(is_active(p, "t745_a").await, Some(true));
    assert_eq!(is_active(p, "t745_stale").await, Some(true));
}

/// A newer run supersedes an older one: the older run's queued chunks are cancelled, a chunk of
/// it still in flight is discarded when it commits, and only the newer run finalizes.
#[tokio::test]
async fn a_newer_run_supersedes_the_older_one() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let adapter = ChunkedAdapter::new(&IDS, 3);
    let w1 = Arc::new(worker(p, &adapter, "t745-w1"));
    let w2 = worker(p, &adapter, "t745-w2");
    enqueue_discovery(p, 5).await;
    w2.run_once().await.unwrap();
    let older = runs(p).await.pop().unwrap();
    assert_eq!(older.state, "running");

    // One of the older run's chunks is in flight when the newer run starts.
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *adapter.gate.lock().unwrap() = Some(gate.clone());
    let in_flight = tokio::spawn({
        let w1 = w1.clone();
        async move { w1.run_once().await }
    });
    while AtomicUsize::load(&adapter.in_flight, Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // Higher priority, so it is claimed before the older run's remaining chunks.
    enqueue_discovery(p, 6).await;
    assert!(matches!(
        w2.run_once().await,
        Some(JobOutcome::Completed(_))
    ));
    let all = runs(p).await;
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].state, "superseded");
    assert_eq!(all[1].state, "running");
    let cancelled = count(
        p,
        &format!(
            "SELECT count(*) AS n FROM crawl_queue WHERE kind = 'discover_chunk' \
             AND series_id LIKE '{}/%' AND status = 'cancelled'",
            older.id
        ),
    )
    .await;
    assert_eq!(cancelled, 2, "the older run's queued chunks are cancelled");

    gate.add_permits(1);
    assert!(matches!(
        in_flight.await.unwrap(),
        Some(JobOutcome::Completed(_))
    ));
    let older_after = runs(p).await.remove(0);
    assert_eq!(
        (older_after.state.as_str(), older_after.chunks_done),
        ("superseded", older.chunks_done),
        "the late chunk was discarded"
    );
    assert_eq!(
        metadata(p).await.len(),
        0,
        "the older run committed nothing"
    );

    drain(&w2).await;
    let all = runs(p).await;
    assert_eq!(all[0].state, "superseded");
    assert_eq!(all[1].state, "complete");
    assert_eq!(finalize_jobs(p, all[0].id).await, 0);
    assert_eq!(finalize_jobs(p, all[1].id).await, 1);
    assert_eq!(metadata(p).await.len(), IDS.len());
}

/// Chunk jobs that finish at the same moment, on different workers, finalize the run once.
#[tokio::test]
async fn concurrent_last_chunk_completions_finalize_exactly_once() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let adapter = ChunkedAdapter::new(&IDS[..4], 1);
    let workers: Vec<Arc<Worker>> = (0..4)
        .map(|i| Arc::new(worker(p, &adapter, &format!("t745-w{i}"))))
        .collect();
    for round in 0..3 {
        enqueue_discovery(p, 5).await;
        workers[0].run_once().await.unwrap();
        *adapter.barrier.lock().unwrap() = Some(Arc::new(tokio::sync::Barrier::new(4)));
        let tasks: Vec<_> = workers
            .iter()
            .map(|w| {
                let w = w.clone();
                tokio::spawn(async move { w.run_once().await })
            })
            .collect();
        for t in tasks {
            assert!(matches!(t.await.unwrap(), Some(JobOutcome::Completed(_))));
        }
        *adapter.barrier.lock().unwrap() = None;
        let run = runs(p).await.pop().unwrap();
        assert_eq!(
            (run.state.as_str(), run.chunks_done),
            ("finalizing", 4),
            "round {round}"
        );
        assert_eq!(finalize_jobs(p, run.id).await, 1, "round {round}");
        drain(&workers[0]).await;
        assert_eq!(runs(p).await.pop().unwrap().state, "complete");
    }
}

/// The conditional transition alone: two transactions storing a run's last two chunks at once
/// queue exactly one finalize job.
#[tokio::test]
async fn racing_chunk_commits_begin_finalize_once() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let settings = ChunkedDiscovery {
        units_per_chunk: 1,
        selection_fingerprint: None,
    };
    for _ in 0..10 {
        let (run_id, _) = discovery_run::start_run(p, SRC, &settings, 5)
            .await
            .unwrap();
        let mut conn = p.get().await.unwrap();
        let units = vec![DiscoveryUnit::new("t745_a"), DiscoveryUnit::new("t745_b")];
        discovery_run::commit_enumeration_conn(&mut conn, run_id, &units, 1, None, &[], 3)
            .await
            .unwrap();
        drop(conn);
        let commit = |index: i32| {
            let pool = p.clone();
            tokio::spawn(async move {
                let mut conn = pool.get().await.unwrap();
                conn.transaction::<_, econ_graph_core::AppError, _>(async move |conn| {
                    discovery_run::commit_chunk_conn(
                        conn,
                        run_id,
                        index,
                        &ChunkDiscovery::default(),
                        3,
                    )
                    .await
                })
                .await
                .unwrap()
            })
        };
        let (a, b) = (commit(0), commit(1));
        let mut results = vec![a.await.unwrap(), b.await.unwrap()];
        results.sort_by_key(|r| *r == ChunkCommit::StoredLast);
        assert_eq!(results, vec![ChunkCommit::Stored, ChunkCommit::StoredLast]);
        assert_eq!(finalize_jobs(p, run_id).await, 1);
    }
}

/// A worker that dies mid-chunk: the reaper returns its chunk to the queue, another worker runs
/// it, the run finalizes once, and the first worker's late result is dropped as a lost lease.
#[tokio::test]
async fn a_worker_restart_mid_run_resumes() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let adapter = ChunkedAdapter::new(&IDS, 3);
    let stalled = Arc::new(worker(p, &adapter, "t745-stalled"));
    let fresh = worker(p, &adapter, "t745-fresh");
    enqueue_discovery(p, 5).await;
    fresh.run_once().await.unwrap();

    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *adapter.gate.lock().unwrap() = Some(gate.clone());
    let late = tokio::spawn({
        let stalled = stalled.clone();
        async move { stalled.run_once().await }
    });
    while AtomicUsize::load(&adapter.in_flight, Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // The stalled worker's lease expires; the reaper puts its chunk back in the queue.
    exec(
        p,
        "UPDATE crawl_queue SET locked_at = NOW() - INTERVAL '2 hours' WHERE status = 'processing'",
    )
    .await;
    assert_eq!(
        CrawlQueueItem::release_stuck(p, Duration::from_secs(3600))
            .await
            .unwrap(),
        1
    );
    drain(&fresh).await;
    let run = runs(p).await.pop().unwrap();
    assert_eq!((run.state.as_str(), run.chunks_done), ("complete", 3));
    assert_eq!(finalize_jobs(p, run.id).await, 1);

    gate.add_permits(1);
    assert!(matches!(
        late.await.unwrap(),
        Some(JobOutcome::LeaseLost { .. })
    ));
    let after = runs(p).await.pop().unwrap();
    assert_eq!((after.state.as_str(), after.chunks_done), ("complete", 3));
    assert_eq!(finalize_jobs(p, run.id).await, 1);
    assert_eq!(metadata(p).await.len(), IDS.len());
}

/// Jobs that end without the worker's failure hook (the reaper exhausting a chunk's retries)
/// fail their run at the next maintenance pass.
#[tokio::test]
async fn abandoned_runs_are_failed_by_maintenance() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let adapter = ChunkedAdapter::new(&IDS, 3);
    let w = worker(p, &adapter, "t745-w1");
    enqueue_discovery(p, 5).await;
    w.run_once().await.unwrap();
    exec(
        p,
        "UPDATE crawl_queue SET status = 'failed', finished_at = NOW() \
         WHERE id = (SELECT id FROM crawl_queue WHERE kind = 'discover_chunk' LIMIT 1)",
    )
    .await;
    // A run left enumerating by a discover_catalog job that is gone.
    exec(
        p,
        "INSERT INTO discovery_runs (source, state) VALUES ('CENSUS', 'enumerating')",
    )
    .await;
    let mut failed = discovery_run::fail_abandoned_runs(p).await.unwrap();
    failed.sort();
    assert_eq!(failed, vec!["BEA".to_string(), "CENSUS".to_string()]);
    assert!(runs(p).await.iter().all(|r| r.state == "failed"));
    assert_eq!(
        count(
            p,
            "SELECT count(*) AS n FROM crawl_queue WHERE kind = 'discover_chunk' \
             AND status = 'pending'"
        )
        .await,
        0
    );
    assert!(discovery_run::fail_abandoned_runs(p)
        .await
        .unwrap()
        .is_empty());
}

/// Unchanged catalogs complete the run with no chunks; a changed selection fails finalize.
#[tokio::test]
async fn unchanged_catalog_and_changed_selection() {
    struct Unchanged(Arc<ChunkedAdapter>);
    #[async_trait]
    impl SourceAdapter for Unchanged {
        fn id(&self) -> SourceId {
            SRC
        }
        fn datasets(&self) -> &[&str] {
            &[FLAT]
        }
        fn chunked_discovery(&self) -> Option<ChunkedDiscovery> {
            self.0.chunked_discovery()
        }
        async fn discover(&self, _: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
            Ok(Vec::new())
        }
        async fn enumerate_discovery(&self, _: &CrawlCtx) -> Result<Enumeration, CrawlError> {
            Ok(Enumeration::Unchanged)
        }
        async fn fetch_series(
            &self,
            _: &CrawlCtx,
            _: &str,
            _: Option<NaiveDate>,
        ) -> Result<FetchedSeries, CrawlError> {
            Err(CrawlError::Permanent("not used".into()))
        }
    }

    let Some(db) = db().await else { return };
    let p = &db.pool;
    seed_stale(p).await;
    let adapter = ChunkedAdapter::new(&IDS, 3);
    let mut ctx = test_ctx_with(HashMap::new());
    ctx.pool = p.clone();
    let mut registry = AdapterRegistry::new();
    registry.register(Arc::new(Unchanged(adapter.clone())));
    let w = Worker::new(ctx, registry, worker(p, &adapter, "x").config.clone())
        .with_datasets(catalog());
    enqueue_discovery(p, 5).await;
    drain(&w).await;
    let run = runs(p).await.pop().unwrap();
    assert_eq!((run.state.as_str(), run.chunks_total), ("complete", 0));
    assert!(run.catalog_unchanged);
    assert_eq!(is_active(p, "t745_stale").await, Some(true));

    // The selection changes between enumeration and finalize: the run fails.
    let w = worker(p, &adapter, "t745-w1");
    enqueue_discovery(p, 5).await;
    w.run_once().await.unwrap();
    *adapter.chunked.lock().unwrap() = Some(ChunkedDiscovery {
        units_per_chunk: 3,
        selection_fingerprint: Some("rules-v2".into()),
    });
    drain(&w).await;
    let run = runs(p).await.pop().unwrap();
    assert_eq!(run.state, "failed");
    assert_eq!(is_active(p, "t745_stale").await, Some(true));
    assert_eq!(metadata(p).await.len(), 1);
}

/// Purging keeps each source's newest complete run, and the oldest run of a source with none.
#[tokio::test]
async fn purge_keeps_the_runs_alerts_need() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    exec(
        p,
        "INSERT INTO discovery_runs (source, state, started_at, finished_at) VALUES \
         ('BEA', 'complete', NOW() - INTERVAL '40 days', NOW() - INTERVAL '40 days'), \
         ('BEA', 'complete', NOW() - INTERVAL '30 days', NOW() - INTERVAL '30 days'), \
         ('BEA', 'failed', NOW() - INTERVAL '20 days', NOW() - INTERVAL '20 days'), \
         ('BEA', 'superseded', NOW() - INTERVAL '1 day', NOW() - INTERVAL '1 day'), \
         ('CENSUS', 'failed', NOW() - INTERVAL '50 days', NOW() - INTERVAL '50 days'), \
         ('CENSUS', 'failed', NOW() - INTERVAL '45 days', NOW() - INTERVAL '45 days'), \
         ('CENSUS', 'running', NOW() - INTERVAL '44 days', NULL)",
    )
    .await;
    let deleted = discovery_run::purge_finished_runs(p, Duration::from_secs(14 * 86_400))
        .await
        .unwrap();
    assert_eq!(deleted, 3);
    #[derive(QueryableByName)]
    struct Kept {
        #[diesel(sql_type = Text)]
        kept: String,
    }
    let mut conn = p.get().await.unwrap();
    let kept: Vec<Kept> = diesel::sql_query(
        "SELECT source || ':' || state || ':' || extract(day FROM NOW() - started_at)::int AS kept \
         FROM discovery_runs ORDER BY source, started_at",
    )
    .load(&mut conn)
    .await
    .unwrap();
    let kept: Vec<String> = kept.into_iter().map(|k| k.kept).collect();
    assert_eq!(
        kept,
        vec![
            "BEA:complete:30",
            "BEA:superseded:1",
            "CENSUS:failed:50",
            "CENSUS:running:44",
        ]
    );
}
