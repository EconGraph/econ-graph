//! Soak / rate test for the queue worker (A13 verification).
//!
//! Phase 1: two `Worker`s (concurrency 4 each, separate pools and separate `HttpFetcher`s, as two
//! replicas would have) drain 200 jobs (100 FRED + 100 BLS) against wiremock servers that serve
//! the FRED/BLS fixtures with ~10% injected 429/500s. A few "flaky" series always return 500.
//! Phase 2: one worker drains 80 jobs so the single-process request rate can be checked against
//! the policy.
//!
//! Asserts: every job ends `completed` or `failed` with no lock; no job is fetched by two loops
//! at once; flaky jobs fail after exactly `max_retries` attempts spaced by at least the policy
//! backoff; every claim happened at/after `scheduled_for`; the single-worker per-source request
//! rate never exceeds `burst + rate * window`.
//!
//! Policy overrides (to keep the run ~2 minutes): FRED keeps its real rate (2 req/s, burst 4,
//! concurrency 4) but queue backoff is 2s..8s instead of 30s..30min; BLS runs at 2 req/s
//! (real: 25/min) with its real burst 1 / concurrency 1 and the same short backoff. The circuit
//! breaker pause is 10s instead of 5 min.
//!
//! The soak test needs `DATABASE_URL` and takes ~2-4 minutes, so it is `#[ignore]`d and kept out
//! of CI. Run it manually with:
//! `DATABASE_URL=postgres://... cargo test -p econ-graph-crawler --all-features --test soak -- --ignored --nocapture`
//! It deletes every `crawl_queue` row, so never point it at a shared database.
//!
//! The two limiter tests at the bottom (`limiter_permits_conform_to_policy`,
//! `limiter_burst_after_idle`) need no database, run on paused tokio time, and run by default.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::models::{CrawlQueueItem, NewCrawlQueueItem};
use econ_graph_core::schema::crawl_queue;
use econ_graph_core::DatabasePool;
use econ_graph_crawler::sources::{bls::BlsAdapter, fred::FredAdapter};
use econ_graph_crawler::{
    AdapterRegistry, ApiKeys, CrawlCtx, CrawlError, DiscoveredSeries, FetchedSeries, HttpConfig,
    HttpFetcher, SourceAdapter, SourceId, SourcePolicy, Worker, WorkerConfig,
};
use rand::{Rng, SeedableRng};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const MAX_RETRIES: i32 = 3;
const QUEUE_RETRY_AFTER_SECS: u64 = 7; // > 5s, so the fetcher hands it back to the queue

// ---------------------------------------------------------------------------------------------
// Mock upstream with error injection and request log
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Api {
    Fred,
    Bls,
}

struct Upstream {
    api: Api,
    rng: Mutex<rand::rngs::StdRng>,
    flaky: HashSet<String>,
    log: Arc<Mutex<Vec<(Instant, String, u16)>>>,
    fred_series: String,
    fred_obs: String,
    bls: String,
}

impl Upstream {
    fn series_of(&self, req: &Request) -> String {
        match self.api {
            Api::Fred => req
                .url
                .query_pairs()
                .find(|(k, _)| k == "series_id")
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default(),
            Api::Bls => serde_json::from_slice::<serde_json::Value>(&req.body)
                .ok()
                .and_then(|v| v["seriesid"][0].as_str().map(str::to_string))
                .unwrap_or_default(),
        }
    }
}

impl Respond for Upstream {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let now = Instant::now();
        let series = self.series_of(req);
        let roll: f64 = self.rng.lock().unwrap().gen();
        let (status, resp) = if self.flaky.contains(&series) || roll < 0.05 {
            (
                500,
                ResponseTemplate::new(500).set_body_string("injected 500"),
            )
        } else if roll < 0.08 {
            // short throttle: retried in-process by HttpFetcher
            (429, ResponseTemplate::new(429))
        } else if roll < 0.10 {
            // long throttle: returned to the queue as RateLimited (not counted)
            (
                429,
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", QUEUE_RETRY_AFTER_SECS.to_string().as_str()),
            )
        } else {
            let body = match (self.api, req.url.path().ends_with("/observations")) {
                (Api::Fred, true) => &self.fred_obs,
                (Api::Fred, false) => &self.fred_series,
                (Api::Bls, _) => &self.bls,
            };
            (
                200,
                ResponseTemplate::new(200).set_body_raw(body.clone(), "application/json"),
            )
        };
        self.log.lock().unwrap().push((now, series, status));
        resp
    }
}

struct Mocked {
    _server: MockServer,
    base: String,
    log: Arc<Mutex<Vec<(Instant, String, u16)>>>,
}

async fn start_upstream(api: Api, flaky: HashSet<String>, seed: u64) -> Mocked {
    let read = |p: &str| std::fs::read_to_string(format!("{FIXTURES}/{p}")).expect(p);
    let log = Arc::new(Mutex::new(Vec::new()));
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(Upstream {
            api,
            rng: Mutex::new(rand::rngs::StdRng::seed_from_u64(seed)),
            flaky,
            log: log.clone(),
            fred_series: read("fred/series_gdp.json"),
            fred_obs: read("fred/observations_gdp.json"),
            bls: read("bls/cpi_monthly.json"),
        })
        .mount(&server)
        .await;
    Mocked {
        base: server.uri(),
        _server: server,
        log,
    }
}

// ---------------------------------------------------------------------------------------------
// Adapter wrapper: detects the same job being fetched by two loops at once
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Tracker {
    inflight: Mutex<HashSet<(SourceId, String)>>,
    violations: AtomicUsize,
    /// start time of every fetch_series call, per (source, series)
    calls: Mutex<HashMap<(SourceId, String), Vec<Instant>>>,
}

struct Tracking {
    inner: Arc<dyn SourceAdapter>,
    t: Arc<Tracker>,
}

#[async_trait]
impl SourceAdapter for Tracking {
    fn id(&self) -> SourceId {
        self.inner.id()
    }
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        self.inner.discover(ctx).await
    }
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let key = (self.inner.id(), external_id.to_string());
        if !self.t.inflight.lock().unwrap().insert(key.clone()) {
            AtomicUsize::fetch_add(&self.t.violations, 1, Ordering::SeqCst);
            eprintln!("VIOLATION: {key:?} fetched concurrently");
        }
        self.t
            .calls
            .lock()
            .unwrap()
            .entry(key.clone())
            .or_default()
            .push(Instant::now());
        let r = self.inner.fetch_series(ctx, external_id, since).await;
        self.t.inflight.lock().unwrap().remove(&key);
        r
    }
}

// ---------------------------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------------------------

fn policies() -> HashMap<SourceId, SourcePolicy> {
    let fred = SourcePolicy {
        base_backoff: Duration::from_secs(2),
        max_backoff: Duration::from_secs(8),
        ..SourcePolicy::default_for(SourceId::Fred)
    };
    let bls = SourcePolicy {
        requests_per_second: 2.0,
        base_backoff: Duration::from_secs(2),
        max_backoff: Duration::from_secs(8),
        ..SourcePolicy::default_for(SourceId::Bls)
    };
    HashMap::from([(SourceId::Fred, fred), (SourceId::Bls, bls)])
}

async fn make_worker(
    url: &str,
    id: &str,
    fred: &Mocked,
    bls: &Mocked,
    tracker: &Arc<Tracker>,
) -> Worker {
    let pool = econ_graph_core::create_pool(url).await.expect("pool");
    let http = HttpFetcher::new(
        HttpConfig {
            timeout: Duration::from_secs(10),
            user_agent: "EconGraph-soak/0".into(),
        },
        policies(),
    )
    .unwrap();
    let ctx = CrawlCtx {
        http,
        pool,
        keys: ApiKeys {
            fred: Some("soak-fred-key".into()),
            bls: Some("soak-bls-key".into()),
            bea: None,
            census: None,
        },
    };
    let mut reg = AdapterRegistry::new();
    reg.register(Arc::new(Tracking {
        inner: Arc::new(FredAdapter::new(fred.base.clone())),
        t: tracker.clone(),
    }));
    reg.register(Arc::new(Tracking {
        inner: Arc::new(BlsAdapter::new(bls.base.clone())),
        t: tracker.clone(),
    }));
    Worker::new(
        ctx,
        reg,
        WorkerConfig {
            worker_id: id.to_string(),
            concurrency: 4,
            poll_interval: Duration::from_millis(100),
            stuck_after: Duration::from_secs(3600),
            source_filter: None,
            pause_after_consecutive: 5,
            pause_for: Duration::from_secs(10),
            queue_retention: None,
        },
    )
}

#[derive(Queryable, Debug)]
struct Row {
    source: String,
    series_id: String,
    status: String,
    retry_count: i32,
    locked_by: Option<String>,
    locked_at: Option<DateTime<Utc>>,
    scheduled_for: Option<DateTime<Utc>>,
    started_at: Option<DateTime<Utc>>,
}

async fn rows(pool: &DatabasePool) -> Vec<Row> {
    let mut conn = pool.get().await.unwrap();
    crawl_queue::table
        .select((
            crawl_queue::source,
            crawl_queue::series_id,
            crawl_queue::status,
            crawl_queue::retry_count,
            crawl_queue::locked_by,
            crawl_queue::locked_at,
            crawl_queue::scheduled_for,
            crawl_queue::started_at,
        ))
        .load(&mut conn)
        .await
        .unwrap()
}

async fn wait_all_terminal(pool: &DatabasePool, n: usize, limit: Duration) {
    let deadline = Instant::now() + limit;
    loop {
        let r = rows(pool).await;
        let done = r
            .iter()
            .filter(|r| r.status == "completed" || r.status == "failed")
            .count();
        if done >= n || Instant::now() > deadline {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Max over all windows [t_i, t_j] of `count - (burst + rate * (t_j - t_i))`, plus the count
/// in the worst window and its length.
fn rate_excess(times: &[Instant], rate: f64, burst: u32) -> (f64, usize, f64) {
    let mut t: Vec<Instant> = times.to_vec();
    t.sort();
    let mut worst = (f64::NEG_INFINITY, 0usize, 0.0f64);
    for i in 0..t.len() {
        for j in i..t.len() {
            let dt = (t[j] - t[i]).as_secs_f64();
            let count = j - i + 1;
            let excess = count as f64 - (burst as f64 + rate * dt);
            if excess > worst.0 {
                worst = (excess, count, dt);
            }
        }
    }
    worst
}

fn avg_rate(times: &[Instant]) -> f64 {
    let (Some(a), Some(b)) = (times.iter().min(), times.iter().max()) else {
        return 0.0;
    };
    let span = (*b - *a).as_secs_f64();
    if span == 0.0 {
        0.0
    } else {
        (times.len() - 1) as f64 / span
    }
}

struct PhaseReport {
    fred_excess: (f64, usize, f64),
    bls_excess: (f64, usize, f64),
}

#[allow(clippy::too_many_arguments)]
async fn run_phase(
    name: &str,
    url: &str,
    admin: &DatabasePool,
    n_per_source: usize,
    n_flaky: usize,
    n_workers: usize,
    seed: u64,
    limit: Duration,
) -> PhaseReport {
    {
        let mut conn = admin.get().await.unwrap();
        for sql in [
            "DELETE FROM crawl_queue",
            "DELETE FROM economic_series WHERE external_id LIKE 'soak\\_%'",
        ] {
            diesel::sql_query(sql).execute(&mut conn).await.unwrap();
        }
    }
    let flaky_ids = |p: &str| -> HashSet<String> {
        (0..n_flaky)
            .map(|i| format!("soak_{name}_{p}_{i:03}"))
            .collect()
    };
    let (fred_flaky, bls_flaky) = (flaky_ids("fred"), flaky_ids("bls"));
    let fred = start_upstream(Api::Fred, fred_flaky.clone(), seed).await;
    let bls = start_upstream(Api::Bls, bls_flaky.clone(), seed + 1).await;

    let mut rng = rand::rngs::StdRng::seed_from_u64(seed + 2);
    for i in 0..n_per_source {
        for (src, p) in [(SourceId::Fred, "fred"), (SourceId::Bls, "bls")] {
            CrawlQueueItem::enqueue(
                admin,
                &NewCrawlQueueItem {
                    source: src.as_str().to_string(),
                    series_id: format!("soak_{name}_{p}_{i:03}"),
                    priority: rng.gen_range(1..=10),
                    max_retries: MAX_RETRIES,
                    ..Default::default()
                },
            )
            .await
            .unwrap()
            .expect("enqueued");
        }
    }
    let total = n_per_source * 2;

    let tracker = Arc::new(Tracker::default());
    let mut workers = Vec::new();
    for w in 0..n_workers {
        workers.push(make_worker(url, &format!("soak-{name}-w{w}"), &fred, &bls, &tracker).await);
    }
    let started = Instant::now();
    futures::future::join_all(
        workers
            .iter()
            .map(|w| w.run(wait_all_terminal(admin, total, limit))),
    )
    .await;
    let elapsed = started.elapsed();

    // ---- queue end state ----
    let r = rows(admin).await;
    let by_status = r.iter().fold(HashMap::<&str, usize>::new(), |mut m, r| {
        *m.entry(r.status.as_str()).or_default() += 1;
        m
    });
    let not_terminal: Vec<_> = r
        .iter()
        .filter(|r| r.status != "completed" && r.status != "failed")
        .collect();
    let locked: Vec<_> = r
        .iter()
        .filter(|r| r.locked_by.is_some() || r.locked_at.is_some())
        .collect();
    let claimed_early: Vec<_> = r
        .iter()
        .filter(|r| matches!((r.scheduled_for, r.started_at), (Some(s), Some(st)) if st < s))
        .collect();
    let retried: usize = r.iter().filter(|r| r.retry_count > 0).count();
    let rescheduled: usize = r.iter().filter(|r| r.scheduled_for.is_some()).count();

    // ---- upstream logs ----
    let fred_log = fred.log.lock().unwrap().clone();
    let bls_log = bls.log.lock().unwrap().clone();
    let count_status =
        |log: &[(Instant, String, u16)], s: u16| log.iter().filter(|(_, _, st)| *st == s).count();
    let times = |log: &[(Instant, String, u16)]| log.iter().map(|(t, _, _)| *t).collect::<Vec<_>>();
    let p = policies();
    let (pf, pb) = (p[&SourceId::Fred], p[&SourceId::Bls]);
    let fred_excess = rate_excess(&times(&fred_log), pf.requests_per_second, pf.burst);
    let bls_excess = rate_excess(&times(&bls_log), pb.requests_per_second, pb.burst);

    println!(
        "=== soak phase {name}: {n_workers} worker(s) x concurrency 4, {total} jobs, {:.1}s ===",
        elapsed.as_secs_f64()
    );
    println!("  queue end state: {by_status:?}; rows with retry_count>0: {retried}; rows ever rescheduled: {rescheduled}");
    println!(
        "  FRED: {} requests ({} x200, {} x429, {} x500), avg {:.2} req/s (policy {:.2}/s burst {}), worst window: {} req in {:.2}s, excess over burst+rate*dt = {:.2}",
        fred_log.len(), count_status(&fred_log, 200), count_status(&fred_log, 429), count_status(&fred_log, 500),
        avg_rate(&times(&fred_log)), pf.requests_per_second, pf.burst, fred_excess.1, fred_excess.2, fred_excess.0
    );
    println!(
        "  BLS:  {} requests ({} x200, {} x429, {} x500), avg {:.2} req/s (policy {:.2}/s burst {}), worst window: {} req in {:.2}s, excess over burst+rate*dt = {:.2}",
        bls_log.len(), count_status(&bls_log, 200), count_status(&bls_log, 429), count_status(&bls_log, 500),
        avg_rate(&times(&bls_log)), pb.requests_per_second, pb.burst, bls_excess.1, bls_excess.2, bls_excess.0
    );
    println!(
        "  concurrent-processing violations: {}",
        AtomicUsize::load(&tracker.violations, Ordering::SeqCst)
    );

    assert!(
        not_terminal.is_empty(),
        "jobs not terminal: {not_terminal:?}"
    );
    assert!(locked.is_empty(), "terminal jobs still locked: {locked:?}");
    assert_eq!(r.len(), total);
    assert_eq!(
        AtomicUsize::load(&tracker.violations, Ordering::SeqCst),
        0,
        "a job was fetched concurrently"
    );
    assert!(
        claimed_early.is_empty(),
        "claimed before scheduled_for: {claimed_early:?}"
    );

    // ---- retries honoured ----
    let calls = tracker.calls.lock().unwrap().clone();
    for row in &r {
        let flaky = fred_flaky.contains(&row.series_id) || bls_flaky.contains(&row.series_id);
        let src: SourceId = row.source.parse().unwrap();
        let starts = calls
            .get(&(src, row.series_id.clone()))
            .cloned()
            .unwrap_or_default();
        if flaky {
            assert_eq!(
                row.status, "failed",
                "flaky {} must fail: {row:?}",
                row.series_id
            );
            assert_eq!(row.retry_count, MAX_RETRIES, "{row:?}");
            // 429-with-Retry-After never hits a flaky series (it always 500s), so attempts == max_retries.
            assert_eq!(
                starts.len(),
                MAX_RETRIES as usize,
                "{} attempts: {}",
                row.series_id,
                starts.len()
            );
            let policy = p[&src];
            for (n, w) in starts.windows(2).enumerate() {
                let gap = w[1] - w[0];
                let min = policy.backoff(n as u32);
                assert!(
                    gap >= min,
                    "{}: attempt {} after {:?} < backoff {:?}",
                    row.series_id,
                    n + 2,
                    gap,
                    min
                );
            }
        } else {
            assert_eq!(row.status, "completed", "non-flaky job failed: {row:?}");
        }
    }
    PhaseReport {
        fred_excess,
        bls_excess,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "soak: run manually"]
async fn soak_two_workers_then_rate_check_single_worker() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping soak test");
        return;
    };
    econ_graph_core::run_migrations(&url)
        .await
        .expect("migrations");
    let admin = econ_graph_core::create_pool(&url).await.expect("pool");

    // Phase 1: correctness under two workers. Rate is reported only: each worker has its own
    // limiter, so the aggregate is up to 2x policy (the k8s manifest documents replicas: 1).
    let two = run_phase("two", &url, &admin, 100, 5, 2, 42, Duration::from_secs(240)).await;
    println!(
        "  two-worker aggregate excess: FRED {:.2}, BLS {:.2} (expected > 0: limiter is per process)",
        two.fred_excess.0, two.bls_excess.0
    );

    // Phase 2: single worker must stay within policy. The limiter is strict (at most
    // burst + rate * window at grant time); the slack of 0.5 requests only absorbs jitter,
    // because timestamps are taken at the mock server after send latency.
    let one = run_phase("one", &url, &admin, 40, 2, 1, 7, Duration::from_secs(240)).await;
    assert!(
        one.fred_excess.0 < 0.5,
        "single worker FRED rate exceeded policy: {:?}",
        one.fred_excess
    );
    assert!(
        one.bls_excess.0 < 0.5,
        "single worker BLS rate exceeded policy: {:?}",
        one.bls_excess
    );
}

/// Client-side check (no network): permit grant times from `SourceRateLimiter` under bursty,
/// concurrent demand never exceed `burst + rate * window`. Runs on paused (virtual) tokio time,
/// which the limiter uses as its clock, so it is instant and exact.
#[tokio::test(start_paused = true)]
async fn limiter_permits_conform_to_policy() {
    use econ_graph_crawler::SourceRateLimiter;
    for (rate, burst) in [(2.0, 4u32), (2.0, 1u32)] {
        let policy = SourcePolicy {
            requests_per_second: rate,
            burst,
            max_concurrency: 4,
            ..SourcePolicy::default_for(SourceId::Fred)
        };
        let limiter = SourceRateLimiter::new(&HashMap::from([(SourceId::Fred, policy)])).unwrap();
        let grants = Arc::new(Mutex::new(Vec::new()));
        let mut tasks = Vec::new();
        for t in 0..4u64 {
            let (limiter, grants) = (limiter.clone(), grants.clone());
            tasks.push(tokio::spawn(async move {
                let mut rng = rand::rngs::StdRng::seed_from_u64(t);
                for _ in 0..6 {
                    let permit = limiter.acquire(SourceId::Fred).await;
                    grants
                        .lock()
                        .unwrap()
                        .push(tokio::time::Instant::now().into_std());
                    tokio::time::sleep(Duration::from_millis(rng.gen_range(0..300))).await;
                    drop(permit);
                    if rng.gen_bool(0.2) {
                        tokio::time::sleep(Duration::from_millis(1500)).await; // let tokens refill
                    }
                }
            }));
        }
        for t in tasks {
            t.await.unwrap();
        }
        let g = grants.lock().unwrap().clone();
        let worst = rate_excess(&g, rate, burst);
        println!(
            "limiter rate {rate}/s burst {burst}: {} grants, worst excess {:.3} ({} in {:.3}s)",
            g.len(),
            worst.0,
            worst.1,
            worst.2
        );
        // Strict bound: at most burst + rate * window (epsilon only for f64 rounding).
        assert!(
            worst.0 <= 1e-6,
            "limiter granted more than burst + rate*dt: {worst:?}"
        );
    }
}

/// After the bucket has been idle, how many permits are granted back-to-back? Must be `burst`.
///
/// Regression for the A13 finding against governor 0.6.3 (a fresh bucket granted `burst`, an
/// idle one `burst + 1`). Paused tokio time: "immediate" means zero virtual time elapsed.
#[tokio::test(start_paused = true)]
async fn limiter_burst_after_idle() {
    use econ_graph_crawler::SourceRateLimiter;
    use tokio::time::Instant;
    let mut results = Vec::new();
    for burst in [1u32, 4] {
        let policy = SourcePolicy {
            requests_per_second: 2.0,
            burst,
            max_concurrency: 16,
            ..SourcePolicy::default_for(SourceId::Fred)
        };
        let limiter = SourceRateLimiter::new(&HashMap::from([(SourceId::Fred, policy)])).unwrap();
        let immediate = |n: &mut usize, t: Instant| {
            if t.elapsed() == Duration::ZERO {
                *n += 1;
            }
        };
        // Fresh bucket.
        let (mut fresh, start) = (0usize, Instant::now());
        for _ in 0..burst + 2 {
            drop(limiter.acquire(SourceId::Fred).await);
            immediate(&mut fresh, start);
        }
        // Idle long enough to refill completely, then the same burst.
        tokio::time::sleep(Duration::from_secs(4)).await;
        let (mut after_idle, start) = (0usize, Instant::now());
        for _ in 0..burst + 2 {
            drop(limiter.acquire(SourceId::Fred).await);
            immediate(&mut after_idle, start);
        }
        println!("burst {burst}: fresh bucket grants {fresh} immediately, after idle {after_idle}");
        results.push((burst, fresh, after_idle));
    }
    for (burst, fresh, after_idle) in results {
        assert_eq!(fresh, burst as usize);
        assert_eq!(
            after_idle, burst as usize,
            "burst {burst}: {after_idle} granted after idle"
        );
    }
}
