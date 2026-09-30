// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The worker's HTTP metrics endpoint and the queue-gauge poller.
//!
//! - `GET /metrics`: Prometheus text exposition of `econ_graph_metrics::DEFAULT_REGISTRY`, the
//!   registry `CRAWLER_METRICS` (HTTP-level `econgraph_crawler_*`), `CRAWLER_QUEUE_METRICS`
//!   (`crawler_jobs_total`, `crawler_queue_*`, `crawler_last_{success,failure}_timestamp_seconds`)
//!   and `CRAWLER_COVERAGE_METRICS` (`crawler_coverage_*`) use.
//! - `GET /healthz`: 200 while the worker loop runs, 503 before it starts and once it stops.
//!
//! [`queue_gauge_loop`] refreshes the queue gauges from
//! [`econ_graph_crawler::status::crawler_status`] every interval, and [`coverage_gauge_loop`] the
//! coverage gauges from [`econ_graph_crawler::coverage::crawl_coverage`] (a heavier query, so on a
//! longer interval).

use std::collections::HashSet;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use econ_graph_core::error::AppResult;
use econ_graph_core::DatabasePool;
use econ_graph_crawler::coverage::{crawl_coverage, SourceCoverage};
use econ_graph_crawler::status::{crawler_status, CrawlerStatusSnapshot};
use econ_graph_crawler::SourceId;
use econ_graph_metrics::crawler::{
    CrawlerCoverageMetrics, CrawlerQueueMetrics, CRAWLER_COVERAGE_METRICS, CRAWLER_METRICS,
    CRAWLER_QUEUE_METRICS,
};
use econ_graph_metrics::prometheus::{Encoder, TextEncoder};
use econ_graph_metrics::DEFAULT_REGISTRY;
use tokio::sync::watch;
use warp::http::StatusCode;
use warp::Filter;

/// Default `--metrics-addr`.
pub const DEFAULT_METRICS_ADDR: &str = "0.0.0.0:9102";

/// Parses `--metrics-addr`: empty or `off` (any case) disables the server.
pub fn parse_metrics_addr(s: &str) -> Result<Option<SocketAddr>, String> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("off") {
        return Ok(None);
    }
    s.parse().map(Some).map_err(|e| {
        format!("invalid metrics address {s:?}: {e} (use host:port, empty or \"off\")")
    })
}

/// Makes sure the lazily registered metric families exist before the first scrape.
pub fn init_metrics() {
    let _ = &*CRAWLER_METRICS;
    let _ = &*CRAWLER_QUEUE_METRICS;
    let _ = &*CRAWLER_COVERAGE_METRICS;
}

/// Unix seconds of `t`, as a gauge value.
fn unix_secs(t: chrono::DateTime<chrono::Utc>) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let secs = t.timestamp_millis() as f64 / 1000.0;
    secs
}

/// Sets `gauge{source}` to `t`, or removes the series when `t` is `None`.
fn set_or_remove(
    gauge: &econ_graph_metrics::prometheus::GaugeVec,
    source: &str,
    t: Option<chrono::DateTime<chrono::Utc>>,
) {
    match t {
        Some(t) => gauge.with_label_values(&[source]).set(unix_secs(t)),
        None => {
            let _ = gauge.remove_label_values(&[source]);
        }
    }
}

/// The text exposition of the shared registry.
pub fn render_metrics() -> Result<String, String> {
    let mut buf = Vec::new();
    TextEncoder::new()
        .encode(&DEFAULT_REGISTRY.gather(), &mut buf)
        .map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

fn routes(
    alive: Arc<AtomicBool>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let metrics = warp::path!("metrics")
        .and(warp::get())
        .map(|| match render_metrics() {
            Ok(body) => warp::reply::with_status(
                warp::reply::with_header(body, "content-type", TextEncoder::new().format_type()),
                StatusCode::OK,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "encoding metrics failed");
                warp::reply::with_status(
                    warp::reply::with_header(e, "content-type", "text/plain"),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            }
        });
    let healthz = warp::path!("healthz").and(warp::get()).map(move || {
        if alive.load(Ordering::SeqCst) {
            warp::reply::with_status("ok", StatusCode::OK)
        } else {
            warp::reply::with_status("worker not running", StatusCode::SERVICE_UNAVAILABLE)
        }
    });
    metrics.or(healthz)
}

/// Binds the metrics server on `addr` (port 0 picks a free port). Returns the bound address and
/// the server future, which completes after `shutdown` resolves.
pub fn bind(
    addr: SocketAddr,
    alive: Arc<AtomicBool>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(SocketAddr, impl Future<Output = ()>), warp::Error> {
    warp::serve(routes(alive)).try_bind_with_graceful_shutdown(addr, shutdown)
}

/// Sets the queue gauges in `m` from `snapshot`. `known` holds the sources set by the previous
/// call; sources no longer present have their series removed.
pub fn apply_snapshot(
    m: &CrawlerQueueMetrics,
    snapshot: &CrawlerStatusSnapshot,
    known: &mut HashSet<String>,
) {
    let current: HashSet<String> = snapshot
        .per_source
        .iter()
        .map(|s| s.source.clone())
        .collect();
    for gone in known.difference(&current) {
        for status in ["pending", "processing", "retrying"] {
            let _ = m.queue_items.remove_label_values(&[gone.as_str(), status]);
        }
        let _ = m.queue_failed_24h.remove_label_values(&[gone]);
        let _ = m
            .last_success_timestamp_seconds
            .remove_label_values(&[gone]);
        let _ = m
            .last_failure_timestamp_seconds
            .remove_label_values(&[gone]);
    }
    for s in &snapshot.per_source {
        let src = s.source.as_str();
        for (status, n) in [
            ("pending", s.pending),
            ("processing", s.processing),
            ("retrying", s.retrying),
        ] {
            m.queue_items.with_label_values(&[src, status]).set(n);
        }
        m.queue_failed_24h
            .with_label_values(&[src])
            .set(s.failed_24h);
        set_or_remove(&m.last_success_timestamp_seconds, src, s.last_success);
        set_or_remove(&m.last_failure_timestamp_seconds, src, s.last_failure);
    }
    *known = current;
}

/// Sets the coverage gauges in `m` from `rows`. `known` holds the sources set by the previous
/// call; sources no longer present (e.g. disabled) have their series removed.
pub fn apply_coverage(
    m: &CrawlerCoverageMetrics,
    rows: &[SourceCoverage],
    known: &mut HashSet<String>,
) {
    let current: HashSet<String> = rows.iter().map(|c| c.source.clone()).collect();
    for gone in known.difference(&current) {
        let src = [gone.as_str()];
        let _ = m.series_discovered.remove_label_values(&src);
        let _ = m.series_with_data.remove_label_values(&src);
        let _ = m.ratio.remove_label_values(&src);
        let _ = m.series_overdue.remove_label_values(&src);
        let _ = m.oldest_success_timestamp_seconds.remove_label_values(&src);
    }
    for c in rows {
        let src = c.source.as_str();
        m.series_discovered
            .with_label_values(&[src])
            .set(c.discovered);
        m.series_with_data
            .with_label_values(&[src])
            .set(c.with_data);
        m.ratio.with_label_values(&[src]).set(c.ratio());
        m.series_overdue.with_label_values(&[src]).set(c.overdue);
        set_or_remove(&m.oldest_success_timestamp_seconds, src, c.oldest_success);
    }
    *known = current;
}

/// Reads coverage for `sources` and updates the gauges in `m`, then records the refresh time.
/// On error the gauges keep their previous values; the unchanged refresh time shows they're
/// stale.
pub async fn update_coverage_gauges(
    pool: &DatabasePool,
    sources: &[SourceId],
    m: &CrawlerCoverageMetrics,
    known: &mut HashSet<String>,
) -> AppResult<()> {
    let rows = crawl_coverage(pool, sources).await?;
    apply_coverage(m, &rows, known);
    m.last_refresh_timestamp_seconds
        .set(unix_secs(chrono::Utc::now()));
    Ok(())
}

/// Reads `crawl_queue` and updates the gauges in `m`.
pub async fn update_queue_gauges(
    pool: &DatabasePool,
    m: &CrawlerQueueMetrics,
    known: &mut HashSet<String>,
) -> AppResult<()> {
    let snapshot = crawler_status(pool).await?;
    apply_snapshot(m, &snapshot, known);
    Ok(())
}

/// Refreshes the global queue gauges every `interval` until `stop` becomes true.
pub async fn queue_gauge_loop(
    pool: DatabasePool,
    interval: Duration,
    mut stop: watch::Receiver<bool>,
) {
    let mut known = HashSet::new();
    loop {
        if *stop.borrow() {
            return;
        }
        if let Err(e) = update_queue_gauges(&pool, &CRAWLER_QUEUE_METRICS, &mut known).await {
            tracing::warn!(error = %e, "updating crawl_queue gauges failed");
        }
        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            changed = stop.changed() => if changed.is_err() { return; },
        }
    }
}

/// Refreshes the global coverage gauges for `sources` every `interval` until `stop` becomes
/// true.
pub async fn coverage_gauge_loop(
    pool: DatabasePool,
    sources: Vec<SourceId>,
    interval: Duration,
    mut stop: watch::Receiver<bool>,
) {
    let mut known = HashSet::new();
    loop {
        if *stop.borrow() {
            return;
        }
        if let Err(e) =
            update_coverage_gauges(&pool, &sources, &CRAWLER_COVERAGE_METRICS, &mut known).await
        {
            tracing::warn!(error = %e, "updating coverage gauges failed");
        }
        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            changed = stop.changed() => if changed.is_err() { return; },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::sql_types::Text;
    use diesel_async::RunQueryDsl;
    use econ_graph_crawler::adapter::AdapterRegistry;
    use econ_graph_crawler::{ApiKeys, CrawlCtx, HttpConfig, HttpFetcher, Worker, WorkerConfig};
    use econ_graph_metrics::prometheus::Registry;
    use std::collections::HashMap;

    /// DB tests empty `crawl_queue`; serialise them within this binary.
    static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn db() -> Option<(DatabasePool, tokio::sync::MutexGuard<'static, ()>)> {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            eprintln!("DATABASE_URL not set; skipping DB-backed metrics test");
            return None;
        };
        let guard = DB_LOCK.lock().await;
        econ_graph_core::run_migrations(&url)
            .await
            .expect("running migrations");
        let pool = econ_graph_core::create_pool(&url).await.expect("pool");
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM crawl_queue")
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        Some((pool, guard))
    }

    /// Inserts a row; `finished_ago` is a Postgres interval (e.g. `'1 hour'`) or NULL.
    async fn seed(
        pool: &DatabasePool,
        source: &str,
        series: &str,
        status: &str,
        finished_ago: Option<&str>,
    ) {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query(
            "INSERT INTO crawl_queue (source, series_id, priority, status, max_retries, finished_at, kind) \
             VALUES ($1, $2, 5, $3, 3, NOW() - $4::interval, 'fetch_series')",
        )
        .bind::<Text, _>(source)
        .bind::<Text, _>(series)
        .bind::<Text, _>(status)
        .bind::<diesel::sql_types::Nullable<Text>, _>(finished_ago)
        .execute(&mut conn)
        .await
        .unwrap();
    }

    async fn get(addr: SocketAddr, path: &str) -> (u16, String) {
        let resp = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
        let status = resp.status().as_u16();
        (status, resp.text().await.unwrap())
    }

    fn start_server(alive: Arc<AtomicBool>) -> (SocketAddr, tokio::sync::oneshot::Sender<()>) {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let (addr, server) = bind("127.0.0.1:0".parse().unwrap(), alive, async {
            let _ = rx.await;
        })
        .expect("bind");
        tokio::spawn(server);
        (addr, tx)
    }

    #[test]
    fn metrics_addr_parsing() {
        assert_eq!(parse_metrics_addr("").unwrap(), None);
        assert_eq!(parse_metrics_addr(" OFF ").unwrap(), None);
        assert_eq!(
            parse_metrics_addr(DEFAULT_METRICS_ADDR).unwrap(),
            Some("0.0.0.0:9102".parse().unwrap())
        );
        assert!(parse_metrics_addr("nonsense").is_err());
    }

    #[tokio::test]
    async fn healthz_follows_alive_flag_and_unknown_path_is_404() {
        let alive = Arc::new(AtomicBool::new(false));
        let (addr, stop) = start_server(alive.clone());
        assert_eq!(get(addr, "/healthz").await.0, 503);
        alive.store(true, Ordering::SeqCst);
        assert_eq!(get(addr, "/healthz").await, (200, "ok".to_string()));
        assert_eq!(get(addr, "/nope").await.0, 404);
        let _ = stop.send(());
    }

    #[tokio::test]
    async fn metrics_endpoint_serves_shared_registry() {
        init_metrics();
        CRAWLER_METRICS.record_request("fred", "api.stlouisfed.org", "/series", "200", 0.2);
        let (addr, stop) = start_server(Arc::new(AtomicBool::new(true)));
        let (status, body) = get(addr, "/metrics").await;
        assert_eq!(status, 200);
        assert!(body.contains("econgraph_crawler_requests_total{"), "{body}");
        let _ = stop.send(());
    }

    /// Runs a real job through `Worker::run_once` and checks `crawler_jobs_total` on /metrics.
    #[tokio::test]
    async fn metrics_contains_jobs_total_after_a_job() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        init_metrics();
        // No adapter registered for FRED, so the job fails permanently ("no adapter registered").
        seed(&pool, "FRED", "metrics_job_1", "pending", None).await;
        let ctx = CrawlCtx {
            http: HttpFetcher::new(HttpConfig::default(), HashMap::new()).unwrap(),
            pool: pool.clone(),
            keys: ApiKeys::default(),
        };
        let worker = Worker::new(
            ctx,
            AdapterRegistry::new(),
            WorkerConfig {
                worker_id: "metrics-test".into(),
                ..WorkerConfig::default()
            },
        );
        let before = CRAWLER_QUEUE_METRICS
            .jobs_total
            .with_label_values(&["FRED", "fetch_series", "failed"])
            .get();
        assert!(worker.run_once().await.is_some(), "job should be claimed");
        assert_eq!(
            CRAWLER_QUEUE_METRICS
                .jobs_total
                .with_label_values(&["FRED", "fetch_series", "failed"])
                .get(),
            before + 1
        );

        let (addr, stop) = start_server(Arc::new(AtomicBool::new(true)));
        let (status, body) = get(addr, "/metrics").await;
        assert_eq!(status, 200);
        assert!(
            body.lines().any(|l| l.starts_with("crawler_jobs_total{")
                && l.contains(r#"source="FRED""#)
                && l.contains(r#"kind="fetch_series""#)
                && l.contains(r#"outcome="failed""#)),
            "{body}"
        );
        let _ = stop.send(());
    }

    #[tokio::test]
    async fn queue_gauges_from_seeded_rows() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let registry = Registry::new();
        let m = CrawlerQueueMetrics::new(&registry).unwrap();
        let mut known = HashSet::new();

        seed(&pool, "FRED", "g1", "pending", None).await;
        seed(&pool, "FRED", "g2", "pending", None).await;
        seed(&pool, "FRED", "g3", "retrying", None).await;
        seed(&pool, "FRED", "g4", "completed", Some("1 hour")).await;
        seed(&pool, "BLS", "g5", "processing", None).await;
        seed(&pool, "BLS", "g6", "failed", Some("2 hours")).await;
        seed(&pool, "BLS", "g7", "failed", Some("48 hours")).await;

        update_queue_gauges(&pool, &m, &mut known).await.unwrap();
        let items = |src: &str, st: &str| m.queue_items.with_label_values(&[src, st]).get();
        assert_eq!(items("FRED", "pending"), 2);
        assert_eq!(items("FRED", "retrying"), 1);
        assert_eq!(items("FRED", "processing"), 0);
        assert_eq!(items("BLS", "processing"), 1);
        assert_eq!(items("BLS", "pending"), 0);
        assert_eq!(m.queue_failed_24h.with_label_values(&["FRED"]).get(), 0);
        assert_eq!(m.queue_failed_24h.with_label_values(&["BLS"]).get(), 1);
        let bls_failed = m
            .last_failure_timestamp_seconds
            .with_label_values(&["BLS"])
            .get();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let fred_last = m
            .last_success_timestamp_seconds
            .with_label_values(&["FRED"])
            .get();
        assert!(
            (now - 3600.0 - fred_last).abs() < 60.0,
            "{fred_last} vs {now}"
        );
        assert!(
            (now - 2.0 * 3600.0 - bls_failed).abs() < 60.0,
            "{bls_failed} vs {now}"
        );
        assert_eq!(
            known,
            HashSet::from(["FRED".to_string(), "BLS".to_string()])
        );

        // BLS rows disappear: its series are removed on the next refresh.
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM crawl_queue WHERE source = 'BLS'")
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        update_queue_gauges(&pool, &m, &mut known).await.unwrap();
        let sources: HashSet<String> = registry
            .gather()
            .iter()
            .flat_map(|f| f.get_metric().iter())
            .flat_map(|mm| mm.get_label().iter())
            .filter(|l| l.name() == "source")
            .map(|l| l.value().to_string())
            .collect();
        assert_eq!(sources, HashSet::from(["FRED".to_string()]));
        assert_eq!(known, HashSet::from(["FRED".to_string()]));
    }

    fn coverage_row(source: &str, discovered: i64, with_data: i64) -> SourceCoverage {
        #[allow(clippy::cast_precision_loss)]
        let percent = (discovered > 0).then(|| 100.0 * with_data as f64 / discovered as f64);
        SourceCoverage {
            source: source.into(),
            discovered,
            with_data,
            percent,
            overdue: 1,
            oldest_success: Some(chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap()),
        }
    }

    #[test]
    fn coverage_gauges_set_and_removed() {
        let registry = Registry::new();
        let m = CrawlerCoverageMetrics::new(&registry).unwrap();
        let mut known = HashSet::new();
        apply_coverage(
            &m,
            &[coverage_row("FRED", 200, 190), coverage_row("BLS", 0, 0)],
            &mut known,
        );
        assert_eq!(m.series_discovered.with_label_values(&["FRED"]).get(), 200);
        assert_eq!(m.series_with_data.with_label_values(&["FRED"]).get(), 190);
        assert!((m.ratio.with_label_values(&["FRED"]).get() - 0.95).abs() < 1e-9);
        assert_eq!(m.ratio.with_label_values(&["BLS"]).get(), 0.0);
        assert_eq!(m.series_overdue.with_label_values(&["FRED"]).get(), 1);
        assert_eq!(
            m.oldest_success_timestamp_seconds
                .with_label_values(&["FRED"])
                .get(),
            1_700_000_000.0
        );

        // BLS drops out (disabled): its series go away.
        apply_coverage(&m, &[coverage_row("FRED", 200, 195)], &mut known);
        let sources: HashSet<String> = registry
            .gather()
            .iter()
            .flat_map(|f| f.get_metric().iter())
            .flat_map(|mm| mm.get_label().iter())
            .filter(|l| l.name() == "source")
            .map(|l| l.value().to_string())
            .collect();
        assert_eq!(sources, HashSet::from(["FRED".to_string()]));
    }

    /// Coverage from the database lands on /metrics for every covered source.
    #[tokio::test]
    async fn metrics_contains_coverage_gauges() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        init_metrics();
        let sources = econ_graph_crawler::coverage::covered_sources(
            &econ_graph_crawler::sources::default_registry().ids(),
        );
        assert!(!sources.is_empty());
        let mut known = HashSet::new();
        update_coverage_gauges(&pool, &sources, &CRAWLER_COVERAGE_METRICS, &mut known)
            .await
            .unwrap();
        let (addr, stop) = start_server(Arc::new(AtomicBool::new(true)));
        let (status, body) = get(addr, "/metrics").await;
        assert_eq!(status, 200);
        for name in [
            "crawler_coverage_series_discovered",
            "crawler_coverage_series_with_data",
            "crawler_coverage_ratio",
            "crawler_coverage_series_overdue",
        ] {
            for source in &known {
                let prefix = format!("{name}{{source=\"{source}\"}} ");
                assert!(
                    body.lines().any(|l| l.starts_with(&prefix)),
                    "{prefix} missing:\n{body}"
                );
            }
        }
        assert!(!known.is_empty(), "no enabled covered source reported");
        let refreshed = CRAWLER_COVERAGE_METRICS
            .last_refresh_timestamp_seconds
            .get();
        let now = unix_secs(chrono::Utc::now());
        assert!((now - refreshed).abs() < 60.0, "{refreshed} vs {now}");
        assert!(
            body.contains("crawler_coverage_last_refresh_timestamp_seconds "),
            "{body}"
        );
        let _ = stop.send(());
    }
}
