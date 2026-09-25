// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! `crawler-worker`: drains `crawl_queue` until SIGINT/SIGTERM.
//!
//! Handles `fetch_series` / `discover_catalog` jobs for every adapter in
//! `econ_graph_crawler::sources::default_registry()`, plus SEC `fetch_filing` jobs (series_id =
//! company CIK) through `econ_graph_sec_crawler::SecFilingHandler`. It lives in its own crate
//! because `econ-graph-crawler` must not depend on the SEC crate.
//!
//! Environment: `DATABASE_URL` (required), `FRED_API_KEY` / `BLS_API_KEY` / `BEA_API_KEY` /
//! `CENSUS_API_KEY` (optional), `RUST_LOG` (default `info`). Every flag can also be set through
//! the `CRAWLER_*` variable shown in `--help`.
//!
//! The worker does not run database migrations; the backend applies them at startup.
//!
//! Metrics: unless `--metrics-addr` / `CRAWLER_METRICS_ADDR` is empty or `off`, an HTTP server on
//! that address (default `0.0.0.0:9102`) serves `GET /metrics` (Prometheus text format) and
//! `GET /healthz` (200 while the worker loop runs), and a background task refreshes the
//! `crawler_queue_*` gauges from `crawl_queue` every `--queue-metrics-interval-secs`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use econ_graph_core::models::JobKind;
use econ_graph_crawler::sources::default_registry;
use econ_graph_crawler::{
    ApiKeys, CrawlCtx, HttpConfig, HttpFetcher, SourceId, SourcePolicy, Worker, WorkerConfig,
};
use econ_graph_sec_crawler::SecFilingHandler;

mod metrics;

#[derive(Debug, Parser)]
#[command(name = "crawler-worker", version, about = "Processes crawl_queue jobs")]
struct Args {
    /// Concurrent jobs.
    #[arg(long, env = "CRAWLER_CONCURRENCY", default_value_t = 4)]
    concurrency: usize,

    /// Value stored in crawl_queue.locked_by (default: <hostname>-<pid>).
    #[arg(long, env = "CRAWLER_WORKER_ID")]
    worker_id: Option<String>,

    /// Only process these sources (comma-separated, e.g. FRED,BLS). Default: all.
    #[arg(long, env = "CRAWLER_SOURCES", value_delimiter = ',')]
    sources: Option<Vec<SourceId>>,

    /// Seconds to sleep when no job is due.
    #[arg(long, env = "CRAWLER_POLL_INTERVAL_SECS", default_value_t = 5)]
    poll_interval_secs: u64,

    /// Return items stuck in `processing` for this long to `pending`.
    #[arg(long, env = "CRAWLER_STUCK_AFTER_SECS", default_value_t = 30 * 60)]
    stuck_after_secs: u64,

    /// Delete completed/failed crawl_queue rows that finished more than this many days ago
    /// (checked hourly). 0 disables purging.
    #[arg(long, env = "CRAWLER_QUEUE_RETENTION_DAYS", default_value_t = 14)]
    queue_retention_days: u64,

    /// Pause a source after this many consecutive rate-limit/auth errors (0 = never).
    #[arg(long, env = "CRAWLER_PAUSE_AFTER", default_value_t = 5)]
    pause_after: u32,

    /// How long a paused source stays paused, in seconds.
    #[arg(long, env = "CRAWLER_PAUSE_SECS", default_value_t = 300)]
    pause_secs: u64,

    /// Per-request HTTP timeout in seconds.
    #[arg(long, env = "CRAWLER_HTTP_TIMEOUT_SECS", default_value_t = 30)]
    http_timeout_secs: u64,

    /// Address for the /metrics and /healthz HTTP server; empty or "off" disables it.
    #[arg(long, env = "CRAWLER_METRICS_ADDR", default_value = metrics::DEFAULT_METRICS_ADDR)]
    metrics_addr: String,

    /// Seconds between crawl_queue polls for the queue gauges (only with the metrics server).
    #[arg(
        long,
        env = "CRAWLER_QUEUE_METRICS_INTERVAL_SECS",
        default_value_t = 30
    )]
    queue_metrics_interval_secs: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let args = Args::parse();
    let metrics_addr = metrics::parse_metrics_addr(&args.metrics_addr)?;
    let database_url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?;
    let pool = econ_graph_core::create_pool(&database_url).await?;

    let registry = default_registry();
    // Built-in policies, overridden by whatever each registered adapter declares.
    let policies: HashMap<SourceId, SourcePolicy> = SourceId::ALL
        .into_iter()
        .map(|id| {
            let policy = registry
                .get(id)
                .map_or_else(|| SourcePolicy::default_for(id), |a| a.policy());
            (id, policy)
        })
        .collect();
    let http = HttpFetcher::new(
        HttpConfig {
            timeout: Duration::from_secs(args.http_timeout_secs),
            ..HttpConfig::default()
        },
        policies,
    )?;
    let ctx = CrawlCtx {
        http,
        pool,
        keys: ApiKeys::from_env(),
    };

    let config = WorkerConfig {
        worker_id: args
            .worker_id
            .unwrap_or_else(econ_graph_crawler::worker::default_worker_id),
        concurrency: args.concurrency.max(1),
        poll_interval: Duration::from_secs(args.poll_interval_secs.max(1)),
        stuck_after: Duration::from_secs(args.stuck_after_secs.max(1)),
        source_filter: args.sources,
        pause_after_consecutive: args.pause_after,
        pause_for: Duration::from_secs(args.pause_secs),
        queue_retention: (args.queue_retention_days > 0)
            .then(|| Duration::from_secs(args.queue_retention_days.saturating_mul(24 * 60 * 60))),
    };
    if registry.ids().is_empty() {
        tracing::warn!("no source adapters registered; only SEC fetch_filing jobs can succeed");
    }

    // Metrics server + queue gauges. `stop` ends both after the worker has drained.
    let alive = Arc::new(AtomicBool::new(false));
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let mut background = Vec::new();
    if let Some(addr) = metrics_addr {
        metrics::init_metrics();
        let mut server_stop = stop_rx.clone();
        let (bound, server) = metrics::bind(addr, alive.clone(), async move {
            let _ = server_stop.wait_for(|stop| *stop).await;
        })?;
        tracing::info!(addr = %bound, "metrics server listening (/metrics, /healthz)");
        background.push(tokio::spawn(server));
        background.push(tokio::spawn(metrics::queue_gauge_loop(
            ctx.pool.clone(),
            Duration::from_secs(args.queue_metrics_interval_secs.max(1)),
            stop_rx,
        )));
    } else {
        tracing::info!("metrics server disabled");
    }

    let worker = Worker::new(ctx, registry, config).with_handler(
        SourceId::Sec,
        JobKind::FetchFiling,
        Arc::new(SecFilingHandler::new()),
    );
    alive.store(true, Ordering::SeqCst);
    worker.run(shutdown_signal()).await;
    alive.store(false, Ordering::SeqCst);

    let _ = stop_tx.send(true);
    for task in background {
        let _ = task.await;
    }
    Ok(())
}

/// Resolves on SIGINT (Ctrl-C) or SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "listening for Ctrl-C failed");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => {
                tracing::error!(error = %e, "listening for SIGTERM failed");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => tracing::info!("received SIGINT"),
        () = term => tracing::info!("received SIGTERM"),
    }
}
