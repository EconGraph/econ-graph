//! FRED on the chunked discovery path, through the real worker and queue against Postgres and a
//! mock FRED. Skipped when `DATABASE_URL` is unset; the database tests hold the crate-wide DB
//! test lock. Every series they create has an id starting `T756`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDate;
use diesel::sql_types::{BigInt, Bool, Integer, Nullable, Text};
use diesel::QueryableByName;
use diesel_async::RunQueryDsl;
use econ_graph_core::models::{CrawlQueueItem, JobKind, NewCrawlQueueItem};
use econ_graph_core::DatabasePool;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use super::*;
use crate::adapter::{AdapterRegistry, FetchedSeries};
use crate::dataset::DatasetCatalog;
use crate::testkit::{fast_policy, test_ctx, test_ctx_with, MockSource, Reply, Route};
use crate::worker::{JobOutcome, Worker, WorkerConfig};

const ERR_NO_SERIES: &str =
    r#"{"error_code":400,"error_message":"Bad Request.  The series does not exist."}"#;

/// Selection rules for the fixtures: GDP release rows by title, plus three include overrides:
/// one FRED has, one known-retired probe, one unexpectedly missing.
const RULES: &str = r#"
max_series = 100
max_catalog_series = 1000
include = ["T756INC", "T756PROBE", "T756LOST"]
expected_missing = ["T756PROBE"]
exclude = []

[[rules]]
release_id = 53
title_contains = "gross domestic product"
frequencies = ["Quarterly"]
"#;

/// GDP rows the rule selects, in series-id order.
fn gdp_ids() -> Vec<String> {
    (1..=20).map(|n| format!("T756GDP{n:02}")).collect()
}

/// Mounts the release catalog (the GDP rows, a copyright-restricted GDP row and an unrelated
/// row) and the include overrides' `/series` answers.
async fn mount_fred(mock: &MockSource, gdp: &[String]) {
    mount_fred_titled(mock, gdp, "").await;
}

/// [`mount_fred`], with `suffix` appended to every GDP row's title (a catalog that changed).
async fn mount_fred_titled(mock: &MockSource, gdp: &[String], suffix: &str) {
    let mut rows: Vec<serde_json::Value> = gdp
        .iter()
        .map(|id| {
            serde_json::json!({
                "id": id, "title": format!("Gross Domestic Product {id}{suffix}"),
                "frequency": "Quarterly", "units": "Billions of Dollars",
                "notes": "BEA account."
            })
        })
        .collect();
    rows.push(serde_json::json!({
        "id": "T756GDPR", "title": "Gross Domestic Product, licensed",
        "frequency": "Quarterly", "units": "Index",
        "notes": "Copyright 2026 Example. All rights reserved."
    }));
    rows.push(serde_json::json!({
        "id": "T756OTHER", "title": "An unrelated series", "frequency": "Monthly"
    }));
    rows.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    mock.mount(
        &Route::get("/release/series").query("release_id", "53"),
        Reply::json_str(
            serde_json::json!({
                "count": rows.len(), "offset": 0, "limit": CATALOG_PAGE_SIZE, "seriess": rows
            })
            .to_string(),
        ),
    )
    .await;
    mock.mount(
        &Route::get("/series").query("series_id", "T756INC"),
        Reply::json_str(
            serde_json::json!({"seriess": [{
                "title": "Included headline", "frequency": "Monthly", "units": "Index",
                "notes": "An include override."
            }]})
            .to_string(),
        ),
    )
    .await;
    for missing in ["T756PROBE", "T756LOST"] {
        mock.mount(
            &Route::get("/series").query("series_id", missing),
            Reply::json_str(ERR_NO_SERIES).with_status(400),
        )
        .await;
    }
}

/// FRED with chunked discovery turned off, for comparing against single-job discovery.
struct SingleJob(FredAdapter);

#[async_trait]
impl SourceAdapter for SingleJob {
    fn id(&self) -> SourceId {
        SourceId::Fred
    }
    fn datasets(&self) -> &[&str] {
        self.0.datasets()
    }
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        self.0.discover(ctx).await
    }
    fn discovery_is_complete(&self) -> bool {
        self.0.discovery_is_complete()
    }
    fn tracks_catalog_lifecycle(&self) -> bool {
        self.0.tracks_catalog_lifecycle()
    }
    fn catalog_scope(&self) -> Option<String> {
        self.0.catalog_scope()
    }
    fn catalog_lifecycle(&self) -> CatalogLifecycle {
        self.0.catalog_lifecycle()
    }
    fn curated_missing(&self) -> Vec<String> {
        self.0.curated_missing()
    }
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        self.0.fetch_series(ctx, external_id, since).await
    }
}

// ---------------------------------------------------------------------------
// Scope agreement (no database)
// ---------------------------------------------------------------------------

/// The scope chunked discovery computes from the rules, without a walk, is the scope a walk under
/// the same rules reports, on a cold adapter (never walked) and on a warm one (walked before),
/// and is never `None` for usable rules.
#[tokio::test]
async fn the_rules_scope_agrees_with_a_walk_on_cold_and_warm_adapters() {
    let mock = MockSource::start().await;
    mount_fred(&mock, &gdp_ids()).await;
    let adapter = FredAdapter::new(mock.base_url()).with_catalog_rules_text(RULES);
    let rules_scope = |a: &FredAdapter| a.chunked_discovery().unwrap().catalog_scope;

    // Cold: nothing walked, so the walk-time scope isn't known yet; the rules scope is.
    assert_eq!(adapter.catalog_scope(), None);
    let cold = rules_scope(&adapter);
    assert!(cold.is_some());
    adapter.discover(&test_ctx()).await.unwrap();
    assert_eq!(
        adapter.catalog_scope(),
        cold,
        "cold: rules scope == walk scope"
    );

    // Warm: walked once already; walk again under the same rules.
    let warm = rules_scope(&adapter);
    adapter.discover(&test_ctx()).await.unwrap();
    assert_eq!(warm, cold);
    assert_eq!(
        adapter.catalog_scope(),
        warm,
        "warm: rules scope == walk scope"
    );

    // The shipped rules file too.
    let shipped = FredAdapter::default();
    assert_eq!(
        rules_scope(&shipped),
        Some(load_catalog_rules().unwrap().fingerprint())
    );
}

/// Unusable rules leave the scope `None`, which the worker refuses (see
/// `a_fresh_worker_with_unusable_rules_refuses_and_changes_nothing`).
#[test]
fn unusable_rules_have_no_scope() {
    for text in ["", "max_series = 0", "not toml ["] {
        let adapter = FredAdapter::new("http://unused").with_catalog_rules_text(text);
        let settings = adapter.chunked_discovery().unwrap();
        assert_eq!(settings.catalog_scope, None, "{text:?}");
        assert_eq!(settings.units_per_chunk, UNITS_PER_CHUNK);
    }
}

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

struct Db {
    pool: DatabasePool,
    _guard: tokio::sync::MutexGuard<'static, ()>,
}

async fn exec(pool: &DatabasePool, sql: &str) {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(sql).execute(&mut conn).await.unwrap();
}

async fn db() -> Option<Db> {
    let (url, guard) = crate::testkit::lock_test_db("fred chunked discovery").await?;
    let pool = econ_graph_core::create_pool(&url).await.expect("pool");
    reset(&pool).await;
    Some(Db {
        pool,
        _guard: guard,
    })
}

async fn reset(pool: &DatabasePool) {
    let source_id = persist::data_source_id(pool, SourceId::Fred).await.unwrap();
    for sql in [
        "DELETE FROM crawl_queue".to_string(),
        "DELETE FROM discovery_runs".to_string(),
        "DELETE FROM series_metadata WHERE external_id LIKE 'T756%'".to_string(),
        "DELETE FROM series_lifecycle WHERE external_id LIKE 'T756%'".to_string(),
        format!("DELETE FROM catalog_lifecycle_scopes WHERE source_id = '{source_id}'"),
    ] {
        exec(pool, &sql).await;
    }
}

fn worker(pool: &DatabasePool, adapter: Arc<dyn SourceAdapter>) -> Worker {
    let mut ctx = test_ctx_with(HashMap::from([(
        SourceId::Fred,
        fast_policy(SourceId::Fred),
    )]));
    ctx.pool = pool.clone();
    let mut registry = AdapterRegistry::new();
    registry.register(adapter);
    let datasets = DatasetCatalog::load(&registry).unwrap();
    let config = WorkerConfig {
        worker_id: "t756-worker".into(),
        concurrency: 1,
        poll_interval: Duration::from_millis(20),
        stuck_after: Duration::from_secs(3600),
        source_filter: None,
        pause_after_consecutive: 0,
        pause_for: Duration::from_secs(60),
        queue_retention: None,
    };
    Worker::new(ctx, registry, config).with_datasets(datasets)
}

async fn sync(pool: &DatabasePool, w: &Worker) {
    persist::sync_datasets(pool, &w.datasets).await.unwrap();
}

async fn enqueue_discovery(pool: &DatabasePool) {
    CrawlQueueItem::enqueue(
        pool,
        &NewCrawlQueueItem {
            source: "FRED".into(),
            series_id: "catalog".into(),
            priority: 5,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::DiscoverCatalog.as_str().to_string(),
        },
    )
    .await
    .unwrap()
    .expect("no active discovery job");
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
    #[diesel(sql_type = Bool)]
    is_active: bool,
}

async fn metadata(pool: &DatabasePool) -> Vec<MetadataRow> {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(
        "SELECT external_id, title, description, units, frequency, data_url, is_active \
         FROM series_metadata WHERE external_id LIKE 'T756%' ORDER BY external_id",
    )
    .load(&mut conn)
    .await
    .unwrap()
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct LifecycleRow {
    #[diesel(sql_type = Text)]
    external_id: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Integer)]
    missing_refreshes: i32,
    #[diesel(sql_type = Bool)]
    in_accepted_catalog: bool,
    #[diesel(sql_type = Text)]
    scope_fingerprint: String,
}

async fn lifecycle(pool: &DatabasePool) -> Vec<LifecycleRow> {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(
        "SELECT external_id, status, missing_refreshes, in_accepted_catalog, scope_fingerprint \
         FROM series_lifecycle WHERE external_id LIKE 'T756%' ORDER BY external_id",
    )
    .load(&mut conn)
    .await
    .unwrap()
}

/// The accepted scope's fingerprint and whether a guarded snapshot is held.
async fn accepted_scope(pool: &DatabasePool) -> Option<(String, bool)> {
    #[derive(QueryableByName)]
    struct Row {
        #[diesel(sql_type = Text)]
        fingerprint: String,
        #[diesel(sql_type = Bool)]
        guarded: bool,
    }
    let source_id = persist::data_source_id(pool, SourceId::Fred).await.unwrap();
    let mut conn = pool.get().await.unwrap();
    let rows: Vec<Row> = diesel::sql_query(format!(
        "SELECT fingerprint, guarded_snapshot IS NOT NULL AS guarded \
         FROM catalog_lifecycle_scopes WHERE source_id = '{source_id}'"
    ))
    .load(&mut conn)
    .await
    .unwrap();
    rows.into_iter().next().map(|r| (r.fingerprint, r.guarded))
}

/// `(state, catalog_scope)` of every run, oldest first.
async fn runs(pool: &DatabasePool) -> Vec<(String, Option<String>)> {
    #[derive(QueryableByName)]
    struct Row {
        #[diesel(sql_type = Text)]
        state: String,
        #[diesel(sql_type = Nullable<Text>)]
        catalog_scope: Option<String>,
    }
    let mut conn = pool.get().await.unwrap();
    let rows: Vec<Row> = diesel::sql_query(
        "SELECT state::text AS state, catalog_scope FROM discovery_runs ORDER BY started_at, id",
    )
    .load(&mut conn)
    .await
    .unwrap();
    rows.into_iter()
        .map(|r| (r.state, r.catalog_scope))
        .collect()
}

fn rules_scope() -> String {
    FredCatalogRules::parse(RULES).unwrap().fingerprint()
}

/// Everything a discovery decides: metadata, lifecycle and the accepted scope.
type Outcome = (Vec<MetadataRow>, Vec<LifecycleRow>, Option<(String, bool)>);

async fn outcome(pool: &DatabasePool) -> Outcome {
    (
        metadata(pool).await,
        lifecycle(pool).await,
        accepted_scope(pool).await,
    )
}

/// Two discoveries, the second after `T756GDP07` left the release, on one path.
/// How a test worker discovers FRED.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// An adapter that can't chunk: single-job discovery, as before chunking existed.
    SingleJobAdapter,
    /// FRED's own chunked discovery.
    Chunked,
    /// FRED's adapter, with the worker's switch set to single-job discovery for FRED.
    SwitchedOff,
}

async fn two_discoveries(pool: &DatabasePool, mode: Mode) -> Outcome {
    reset(pool).await;
    let mock = MockSource::start().await;
    let mut gdp = gdp_ids();
    mount_fred(&mock, &gdp).await;
    let fred = FredAdapter::new(mock.base_url())
        .with_catalog_rules_text(RULES)
        .with_units_per_chunk(4);
    let adapter: Arc<dyn SourceAdapter> = if mode == Mode::SingleJobAdapter {
        Arc::new(SingleJob(fred))
    } else {
        Arc::new(fred)
    };
    let mut w = worker(pool, adapter);
    if mode == Mode::SwitchedOff {
        w = w.with_single_job_discovery(vec![SourceId::Fred]);
    }
    sync(pool, &w).await;
    enqueue_discovery(pool).await;
    drain(&w).await;
    gdp.retain(|id| id != "T756GDP07");
    mock.reset().await;
    mount_fred(&mock, &gdp).await;
    enqueue_discovery(pool).await;
    drain(&w).await;
    outcome(pool).await
}

/// On fixtures, chunked discovery stores exactly what single-job discovery stores: the same
/// metadata, the same restricted and confirmed-missing lifecycle, the same miss for a series
/// that left the release, under the same scope.
#[tokio::test]
async fn chunked_fred_discovery_matches_single_job_discovery() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let single = two_discoveries(p, Mode::SingleJobAdapter).await;
    let chunked = two_discoveries(p, Mode::Chunked).await;
    assert_eq!(chunked, single);

    let (metadata, lifecycle, scope) = chunked;
    assert_eq!(scope, Some((rules_scope(), false)));
    // 20 GDP rows, the restricted one, and the include override FRED has.
    assert_eq!(metadata.len(), 22);
    let status = |id: &str| {
        lifecycle
            .iter()
            .find(|r| r.external_id == id)
            .map(|r| (r.status.as_str(), r.missing_refreshes))
    };
    assert_eq!(
        status("T756GDPR"),
        Some(("restricted", 0)),
        "copyright filter"
    );
    assert_eq!(status("T756GDP07"), Some(("active", 1)), "one miss");
    assert_eq!(
        status("T756PROBE").map(|s| s.1),
        Some(2),
        "confirmed missing twice"
    );
    assert_eq!(status("T756OTHER"), None, "unselected: never tracked");
    let runs = runs(p).await;
    assert_eq!(runs.len(), 2);
    assert!(runs
        .iter()
        .all(|(state, scope)| state == "complete" && scope.as_deref() == Some(&*rules_scope())));
}

/// A brand-new worker that never walked FRED runs its first chunked discovery under the rules'
/// scope and deactivates nothing: neither a stored series the catalog doesn't list nor one it
/// lists outside the rules.
#[tokio::test]
async fn a_fresh_worker_runs_its_first_chunked_discovery_under_the_rules_scope() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let source_id = persist::data_source_id(p, SourceId::Fred).await.unwrap();
    exec(
        p,
        &format!(
            "INSERT INTO series_metadata (source_id, external_id, title, is_active) VALUES \
             ('{source_id}', 'T756OLD', 'Not in the catalog', TRUE), \
             ('{source_id}', 'T756OTHER', 'An unrelated series', TRUE)"
        ),
    )
    .await;
    let mock = MockSource::start().await;
    mount_fred(&mock, &gdp_ids()).await;
    let fred = FredAdapter::new(mock.base_url()).with_catalog_rules_text(RULES);
    assert_eq!(fred.catalog_scope(), None, "this process never walked FRED");
    let w = worker(p, Arc::new(fred));
    sync(p, &w).await;
    enqueue_discovery(p).await;
    drain(&w).await;
    assert_eq!(
        runs(p).await,
        vec![("complete".to_string(), Some(rules_scope()))]
    );
    assert_eq!(accepted_scope(p).await, Some((rules_scope(), false)));
    let metadata = metadata(p).await;
    // Only the copyright-restricted series is hidden, exactly as single-job discovery hides it.
    assert!(
        metadata
            .iter()
            .all(|m| m.is_active == (m.external_id != "T756GDPR")),
        "nothing else deactivated: {metadata:?}"
    );
    assert!(lifecycle(p)
        .await
        .iter()
        .all(|r| r.status == "active" || r.external_id == "T756GDPR"));
}

/// A brand-new worker whose rules can't be read refuses the chunked discovery: no run, no
/// chunk, no request, and no `is_active` or lifecycle row changes.
#[tokio::test]
async fn a_fresh_worker_with_unusable_rules_refuses_and_changes_nothing() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let source_id = persist::data_source_id(p, SourceId::Fred).await.unwrap();
    exec(
        p,
        &format!(
            "INSERT INTO series_metadata (source_id, external_id, title, is_active) VALUES \
             ('{source_id}', 'T756OLD', 'Stored series', TRUE)"
        ),
    )
    .await;
    let before = (metadata(p).await, lifecycle(p).await);
    let mock = MockSource::start().await;
    mount_fred(&mock, &gdp_ids()).await;
    let fred = FredAdapter::new(mock.base_url()).with_catalog_rules_text("max_series = 0");
    let w = worker(p, Arc::new(fred));
    sync(p, &w).await;
    enqueue_discovery(p).await;
    let outcome = w.run_once().await;
    assert!(runs(p).await.is_empty(), "no run started");
    assert_eq!(
        count(
            p,
            "SELECT count(*) AS n FROM crawl_queue WHERE kind = 'discover_chunk'"
        )
        .await,
        0
    );
    assert!(mock.received_requests().await.is_empty(), "no request");
    assert_eq!((metadata(p).await, lifecycle(p).await), before);
    assert!(matches!(outcome, Some(JobOutcome::Failed { .. })));
}

/// The first chunked run after deploy retires nothing wrongly. With the scope single-job runs
/// recorded (the same rules), counters carry on: nothing listed gains a miss, and only confirmed
/// NotFound overrides continue toward retirement as they would have. With a scope the chunked
/// runs don't share (rules changed with the deploy), the runs start a new baseline: a series the
/// old scope tracked that the catalog no longer lists keeps its old row and counters and is
/// never counted, so never retired, under the new scope.
#[tokio::test]
async fn the_first_chunked_run_after_deploy_retires_nothing_wrongly() {
    let Some(db) = db().await else { return };
    let p = &db.pool;

    // Same scope: two single-job discoveries, then the first chunked one.
    let mock = MockSource::start().await;
    mount_fred(&mock, &gdp_ids()).await;
    let fred = || FredAdapter::new(mock.base_url()).with_catalog_rules_text(RULES);
    let single = worker(p, Arc::new(SingleJob(fred())));
    sync(p, &single).await;
    for _ in 0..2 {
        enqueue_discovery(p).await;
        drain(&single).await;
    }
    let before = lifecycle(p).await;
    let chunked = worker(p, Arc::new(fred()));
    enqueue_discovery(p).await;
    drain(&chunked).await;
    assert_eq!(accepted_scope(p).await, Some((rules_scope(), false)));
    let after = lifecycle(p).await;
    assert_eq!(after.len(), before.len());
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(a.external_id, b.external_id);
        assert_eq!(
            a.scope_fingerprint, b.scope_fingerprint,
            "{}",
            a.external_id
        );
        if b.external_id == "T756PROBE" || b.external_id == "T756LOST" {
            // Confirmed NotFound overrides: their count carries on from the single-job runs
            // (2 -> 3), so the third confirmation retires them, as a third single-job run
            // would. That is the evidence rule working across the switch, not a wrong retirement.
            assert_eq!(
                (a.missing_refreshes, a.status.as_str()),
                (b.missing_refreshes + 1, "retired"),
                "{}",
                a.external_id
            );
        } else {
            assert_eq!(a.status, b.status, "{}", a.external_id);
            assert_eq!(a.missing_refreshes, 0, "{}", a.external_id);
        }
    }

    // A different scope: a series single-job runs saw missing twice is not retired.
    reset(p).await;
    let source_id = persist::data_source_id(p, SourceId::Fred).await.unwrap();
    exec(
        p,
        &format!(
            "INSERT INTO catalog_lifecycle_scopes (source_id, fingerprint, last_refresh_at) \
             VALUES ('{source_id}', 'fred-catalog-v1:older-rules', NOW() - INTERVAL '7 days')"
        ),
    )
    .await;
    exec(
        p,
        &format!(
            "INSERT INTO series_metadata (source_id, external_id, title, is_active) VALUES \
             ('{source_id}', 'T756GONE', 'Gone upstream', TRUE)"
        ),
    )
    .await;
    exec(
        p,
        &format!(
            "INSERT INTO series_lifecycle (source_id, external_id, status, missing_refreshes, \
                 scope_fingerprint, in_accepted_catalog, listed_in_accepted_catalog) \
             VALUES ('{source_id}', 'T756GONE', 'active', 2, 'fred-catalog-v1:older-rules', \
                 TRUE, TRUE)"
        ),
    )
    .await;
    // Three chunked runs: more than the three-absence rule would need under one scope.
    for run in 1..=3 {
        enqueue_discovery(p).await;
        drain(&chunked).await;
        assert_eq!(accepted_scope(p).await, Some((rules_scope(), false)));
        let gone = lifecycle(p)
            .await
            .into_iter()
            .find(|r| r.external_id == "T756GONE")
            .unwrap();
        // Outside the new scope's snapshot, the old row is left as it was: never counted
        // under the new scope, so never retired by it.
        assert_eq!(
            (
                gone.status.as_str(),
                gone.missing_refreshes,
                gone.scope_fingerprint.as_str()
            ),
            ("active", 2, "fred-catalog-v1:older-rules"),
            "run {run}"
        );
        assert!(
            metadata(p)
                .await
                .iter()
                .find(|m| m.external_id == "T756GONE")
                .unwrap()
                .is_active,
            "run {run}"
        );
    }
}

/// A transient `/series` failure for an include override retries only the chunk holding it;
/// the other chunks run once and the run completes.
#[tokio::test]
async fn a_transient_error_retries_only_its_chunk() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let mock = MockSource::start().await;
    // Three failures: every in-process attempt of the first chunk try fails.
    Mock::given(method("GET"))
        .and(path("/series"))
        .and(query_param("series_id", "T756INC"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(3)
        .with_priority(1)
        .mount(mock.server())
        .await;
    mount_fred(&mock, &gdp_ids()).await;
    let fred = FredAdapter::new(mock.base_url())
        .with_catalog_rules_text(RULES)
        .with_units_per_chunk(4);
    let w = worker(p, Arc::new(fred));
    sync(p, &w).await;
    enqueue_discovery(p).await;
    drain(&w).await;
    assert_eq!(runs(p).await[0].0, "complete");
    #[derive(QueryableByName)]
    struct Chunk {
        #[diesel(sql_type = Text)]
        series_id: String,
        #[diesel(sql_type = Integer)]
        retry_count: i32,
    }
    let mut conn = p.get().await.unwrap();
    let chunks: Vec<Chunk> = diesel::sql_query(
        "SELECT series_id, retry_count FROM crawl_queue WHERE kind = 'discover_chunk' \
         ORDER BY series_id",
    )
    .load(&mut conn)
    .await
    .unwrap();
    let retried: Vec<&Chunk> = chunks.iter().filter(|c| c.retry_count > 0).collect();
    assert_eq!(retried.len(), 1, "only the chunk holding T756INC");
    // Units in id order, 4 a chunk: T756INC is the 22nd of 24 units, in chunk 5.
    assert!(
        retried[0].series_id.ends_with("/5"),
        "{}",
        retried[0].series_id
    );
    assert_eq!(chunks.len(), 6);
}

/// Every request a chunk makes goes through FRED's rate limiter: with a slow policy, a chunk of
/// include overrides takes at least as long as the policy allows for its requests.
#[tokio::test]
async fn chunk_requests_obey_freds_rate_limit() {
    let mock = MockSource::start().await;
    for n in 0..4 {
        mock.mount(
            &Route::get("/series").query("series_id", format!("T756R{n}")),
            Reply::json_str(r#"{"seriess":[{"title":"Rate","frequency":"Monthly"}]}"#),
        )
        .await;
    }
    let mut policy = fast_policy(SourceId::Fred);
    policy.requests_per_second = 10.0;
    policy.burst = 1;
    let ctx = test_ctx_with(HashMap::from([(SourceId::Fred, policy)]));
    let fred = FredAdapter::new(mock.base_url());
    let units: Vec<DiscoveryUnit> = (0..4)
        .map(|n| {
            DiscoveryUnit::with_listing(
                format!("T756R{n}"),
                serde_json::to_value(FredUnit::Include {
                    expected_missing: false,
                })
                .unwrap(),
            )
        })
        .collect();
    let started = std::time::Instant::now();
    let chunk = fred.discover_units(&ctx, &units).await.unwrap();
    assert_eq!(chunk.found.len(), 4);
    // Four requests, one token at a time at ten a second: at least 300 ms.
    assert!(
        started.elapsed() >= Duration::from_millis(290),
        "{:?}",
        started.elapsed()
    );
}

// ---------------------------------------------------------------------------
// The single-job switch
// ---------------------------------------------------------------------------

/// With the switch set for FRED, discovery is single-job, exactly as an adapter that can't
/// chunk: no run, and the same metadata, lifecycle and scope.
#[tokio::test]
async fn the_switch_set_discovers_fred_by_single_job_unchanged() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let single = two_discoveries(p, Mode::SingleJobAdapter).await;
    let switched = two_discoveries(p, Mode::SwitchedOff).await;
    assert_eq!(switched, single);
    assert!(runs(p).await.is_empty(), "no discovery run");
}

/// Title of the series `T756GDP01` as stored.
async fn title_of_gdp01(pool: &DatabasePool) -> String {
    metadata(pool)
        .await
        .into_iter()
        .find(|m| m.external_id == "T756GDP01")
        .map(|m| m.title)
        .unwrap_or_default()
}

/// A run started in chunks, the switch set for FRED mid-run, a single-job discovery, then the
/// switch cleared again with the run's jobs still around: the run is superseded and never
/// commits, so the catalog is the single-job discovery's alone (the run had read the catalog
/// before it changed, which would show as an older title). Covers a run still `running` and one
/// already `finalizing`.
#[tokio::test]
async fn switching_to_single_job_mid_run_never_commits_the_run() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    for finalizing in [false, true] {
        reset(p).await;
        let mock = MockSource::start().await;
        mount_fred(&mock, &gdp_ids()).await;
        let fred = || {
            FredAdapter::new(mock.base_url())
                .with_catalog_rules_text(RULES)
                .with_units_per_chunk(4)
        };
        let chunked = worker(p, Arc::new(fred()));
        sync(p, &chunked).await;
        enqueue_discovery(p).await;
        // Enumerate, then two chunks, or every chunk.
        let jobs = if finalizing { 1 + 6 } else { 1 + 2 };
        for _ in 0..jobs {
            assert!(matches!(
                chunked.run_once().await,
                Some(JobOutcome::Completed(_))
            ));
        }
        let expected = if finalizing { "finalizing" } else { "running" };
        assert_eq!(runs(p).await[0].0, expected);

        // The catalog changes; the switch is set; a single-job discovery runs first.
        mock.reset().await;
        mount_fred_titled(&mock, &gdp_ids(), " (revised)").await;
        let single = worker(p, Arc::new(fred())).with_single_job_discovery(vec![SourceId::Fred]);
        CrawlQueueItem::enqueue(
            p,
            &NewCrawlQueueItem {
                source: "FRED".into(),
                series_id: "catalog".into(),
                priority: 9,
                max_retries: 3,
                scheduled_for: None,
                kind: JobKind::DiscoverCatalog.as_str().to_string(),
            },
        )
        .await
        .unwrap()
        .unwrap();
        assert!(matches!(
            single.run_once().await,
            Some(JobOutcome::Completed(_))
        ));
        let fresh = title_of_gdp01(p).await;
        assert!(fresh.ends_with("(revised)"), "{fresh}");

        // The switch is cleared again: whatever is left of the run must not commit.
        drain(&chunked).await;
        assert_eq!(title_of_gdp01(p).await, fresh, "finalizing={finalizing}");
        assert_eq!(runs(p).await[0].0, "superseded", "finalizing={finalizing}");
        assert_eq!(runs(p).await.len(), 1);
    }
}

/// The other direction. A run in progress whose chunk is claimed by a worker with the switch
/// set fails cleanly and commits nothing; with the switch cleared, the next discovery runs in
/// chunks and commits once.
#[tokio::test]
async fn switching_back_to_chunks_runs_the_next_discovery_in_chunks() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let mock = MockSource::start().await;
    mount_fred(&mock, &gdp_ids()).await;
    let fred = || {
        FredAdapter::new(mock.base_url())
            .with_catalog_rules_text(RULES)
            .with_units_per_chunk(4)
    };
    let chunked = worker(p, Arc::new(fred()));
    let single = worker(p, Arc::new(fred())).with_single_job_discovery(vec![SourceId::Fred]);
    sync(p, &chunked).await;
    enqueue_discovery(p).await;
    chunked.run_once().await.unwrap();
    assert!(matches!(
        single.run_once().await,
        Some(JobOutcome::Failed { .. })
    ));
    drain(&single).await;
    assert_eq!(runs(p).await[0].0, "failed");
    assert_eq!(accepted_scope(p).await, None, "nothing reconciled");
    assert!(metadata(p).await.is_empty(), "nothing committed");

    enqueue_discovery(p).await;
    drain(&chunked).await;
    let all = runs(p).await;
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].0, "complete");
    assert_eq!(accepted_scope(p).await, Some((rules_scope(), false)));
}

// ---------------------------------------------------------------------------
// Include overrides, single-job supersede races, expected-missing probes
// ---------------------------------------------------------------------------

const RULES_OTHER_INCLUDED: &str = r#"
max_series = 100
max_catalog_series = 1000
include = ["T756INC", "T756OTHER"]
expected_missing = []
exclude = []

[[rules]]
release_id = 53
title_contains = "gross domestic product"
frequencies = ["Quarterly"]
"#;

/// An include override that a walked release still lists (outside the rules) but `/series`
/// reports missing. Three discoveries on each path.
async fn three_discoveries_other_included(pool: &DatabasePool, mode: Mode) -> Vec<LifecycleRow> {
    reset(pool).await;
    let mock = MockSource::start().await;
    mount_fred(&mock, &gdp_ids()).await;
    mock.mount(
        &Route::get("/series").query("series_id", "T756OTHER"),
        Reply::json_str(ERR_NO_SERIES).with_status(400),
    )
    .await;
    let fred = FredAdapter::new(mock.base_url())
        .with_catalog_rules_text(RULES_OTHER_INCLUDED)
        .with_units_per_chunk(4);
    let adapter: Arc<dyn SourceAdapter> = if mode == Mode::SingleJobAdapter {
        Arc::new(SingleJob(fred))
    } else {
        Arc::new(fred)
    };
    let w = worker(pool, adapter);
    sync(pool, &w).await;
    for _ in 0..3 {
        enqueue_discovery(pool).await;
        drain(&w).await;
    }
    lifecycle(pool).await
}

/// An include override that a walked release still lists outside the rules, but `/series`
/// reports missing, stays published (unselected) on both paths: over three discoveries the
/// chunked path never counts it absent, exactly as single-job discovery doesn't.
#[tokio::test]
async fn a_listed_include_override_fred_reports_missing_agrees_with_single_job() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let single = three_discoveries_other_included(p, Mode::SingleJobAdapter).await;
    let chunked = three_discoveries_other_included(p, Mode::Chunked).await;
    let pick = |rows: &[LifecycleRow]| {
        rows.iter()
            .find(|r| r.external_id == "T756OTHER")
            .map(|r| (r.status.clone(), r.missing_refreshes))
    };
    assert_eq!(pick(&single), Some(("active".to_string(), 0)));
    assert_eq!(pick(&chunked), pick(&single));
    assert_eq!(chunked, single);
}

/// Inserts a FRED run that is `finalizing` with every chunk done.
async fn finalizing_run(pool: &DatabasePool) -> uuid::Uuid {
    #[derive(QueryableByName)]
    struct Id {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        id: uuid::Uuid,
    }
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(
        "INSERT INTO discovery_runs (source, state, catalog_scope, chunks_total, chunks_done) \
         VALUES ('FRED', 'finalizing', 'x', 1, 1) RETURNING id",
    )
    .get_result::<Id>(&mut conn)
    .await
    .unwrap()
    .id
}

async fn run_state(pool: &DatabasePool, id: uuid::Uuid) -> String {
    #[derive(QueryableByName)]
    struct S {
        #[diesel(sql_type = Text)]
        state: String,
    }
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(format!(
        "SELECT state::text AS state FROM discovery_runs WHERE id = '{id}'"
    ))
    .get_result::<S>(&mut conn)
    .await
    .unwrap()
    .state
}

/// Finalize's conditional commit and a single-job supersede, interleaved on real Postgres in
/// both orders, with finalize also holding the source's exclusive lifecycle lock as its
/// reconciliation does. Exactly one wins; no deadlock.
#[tokio::test]
async fn a_single_job_supersede_races_a_finalize_commit_safely() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let source_id = persist::data_source_id(p, SourceId::Fred).await.unwrap();

    // Order 1: finalize's transaction has moved the run to complete (uncommitted) and holds the
    // source lock; supersede must wait, then find nothing to supersede.
    reset(p).await;
    let run = finalizing_run(p).await;
    let mut a = p.get().await.unwrap();
    diesel::sql_query("BEGIN").execute(&mut a).await.unwrap();
    let done = crate::discovery_run::complete_run_conn(&mut a, run)
        .await
        .unwrap();
    assert!(done.is_some());
    crate::lifecycle::lock_source_exclusive(&mut a, source_id)
        .await
        .unwrap();
    let pool = p.clone();
    let sup = tokio::spawn(async move {
        crate::discovery_run::supersede_active_runs(&pool, SourceId::Fred).await
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !sup.is_finished(),
        "supersede must wait for finalize's row lock"
    );
    diesel::sql_query("COMMIT").execute(&mut a).await.unwrap();
    let n = tokio::time::timeout(Duration::from_secs(10), sup)
        .await
        .expect("no deadlock")
        .unwrap()
        .unwrap();
    assert_eq!(n, 0);
    assert_eq!(run_state(p, run).await, "complete");

    // Order 2: supersede first (its transaction holds the run row); finalize waits and then
    // commits nothing.
    reset(p).await;
    let run = finalizing_run(p).await;
    let mut b = p.get().await.unwrap();
    diesel::sql_query("BEGIN").execute(&mut b).await.unwrap();
    diesel::sql_query(format!(
        "UPDATE discovery_runs SET state = 'superseded', finished_at = NOW() WHERE id = '{run}' AND state = 'finalizing'"
    ))
    .execute(&mut b)
    .await
    .unwrap();
    let pool = p.clone();
    let fin = tokio::spawn(async move {
        let mut c = pool.get().await.unwrap();
        diesel::sql_query("BEGIN").execute(&mut c).await.unwrap();
        let r = crate::discovery_run::complete_run_conn(&mut c, run)
            .await
            .unwrap();
        diesel::sql_query("COMMIT").execute(&mut c).await.unwrap();
        r
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!fin.is_finished());
    diesel::sql_query("COMMIT").execute(&mut b).await.unwrap();
    let r = tokio::time::timeout(Duration::from_secs(10), fin)
        .await
        .unwrap()
        .unwrap();
    assert!(r.is_none(), "finalize must not complete a superseded run");
    assert_eq!(run_state(p, run).await, "superseded");

    // Order 3: the real supersede_active_runs, then finalize: nothing completes.
    reset(p).await;
    let run = finalizing_run(p).await;
    assert_eq!(
        crate::discovery_run::supersede_active_runs(p, SourceId::Fred)
            .await
            .unwrap(),
        1
    );
    let mut c = p.get().await.unwrap();
    assert!(crate::discovery_run::complete_run_conn(&mut c, run)
        .await
        .unwrap()
        .is_none());
    reset(p).await;
}

/// An include override FRED reports missing in a chunk: a known-retired probe
/// (`expected_missing`) is confirmed missing without being a curated entry missing; any other is
/// both. The flag is set at enumeration and honoured in the chunk.
#[tokio::test]
async fn expected_missing_probes_are_not_curated_missing_in_chunks() {
    let mock = MockSource::start().await;
    mount_fred(&mock, &gdp_ids()).await;
    let fred = FredAdapter::new(mock.base_url()).with_catalog_rules_text(RULES);
    let Enumeration::Units { units, .. } = fred.enumerate_discovery(&test_ctx()).await.unwrap()
    else {
        panic!("units expected");
    };
    let flag = |id: &str| {
        let unit = units.iter().find(|u| u.key == id).unwrap();
        match serde_json::from_value(unit.listing.clone().unwrap()).unwrap() {
            FredUnit::Include { expected_missing } => expected_missing,
            FredUnit::Listed { .. } => panic!("{id} is an include override"),
        }
    };
    assert!(flag("T756PROBE"));
    assert!(!flag("T756LOST"));
    assert!(!flag("T756INC"));

    let includes: Vec<DiscoveryUnit> = units
        .into_iter()
        .filter(|u| u.key == "T756PROBE" || u.key == "T756LOST")
        .collect();
    let chunk = fred.discover_units(&test_ctx(), &includes).await.unwrap();
    assert!(chunk.found.is_empty());
    assert_eq!(chunk.curated_missing, ["T756LOST"]);
    let mut confirmed = chunk.lifecycle.confirmed_missing_ids.clone();
    confirmed.sort();
    assert_eq!(confirmed, ["T756LOST", "T756PROBE"]);
}

/// Through the worker, a chunked run publishes the same curated-missing gauge as single-job
/// discovery: one entry (the unexpectedly missing override), not the expected-missing probe.
#[tokio::test]
async fn the_curated_missing_gauge_matches_single_job_discovery() {
    let Some(db) = db().await else { return };
    let p = &db.pool;
    let gauge = || {
        econ_graph_metrics::crawler::CRAWLER_DISCOVERY_METRICS
            .curated_missing
            .with_label_values(&["FRED"])
            .get()
    };
    for mode in [Mode::SingleJobAdapter, Mode::Chunked] {
        reset(p).await;
        let mock = MockSource::start().await;
        mount_fred(&mock, &gdp_ids()).await;
        let fred = FredAdapter::new(mock.base_url())
            .with_catalog_rules_text(RULES)
            .with_units_per_chunk(4);
        let adapter: Arc<dyn SourceAdapter> = if mode == Mode::SingleJobAdapter {
            Arc::new(SingleJob(fred))
        } else {
            Arc::new(fred)
        };
        let w = worker(p, adapter);
        sync(p, &w).await;
        econ_graph_metrics::crawler::CRAWLER_DISCOVERY_METRICS
            .curated_missing
            .with_label_values(&["FRED"])
            .set(-1);
        enqueue_discovery(p).await;
        drain(&w).await;
        assert_eq!(gauge(), 1, "only T756LOST, not the expected-missing probe");
    }
}
