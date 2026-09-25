// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! `crawler-worker`: drains `crawl_queue` until SIGINT/SIGTERM.
//!
//! Environment: `DATABASE_URL` (required), `FRED_API_KEY` / `BLS_API_KEY` / `BEA_API_KEY` /
//! `CENSUS_API_KEY` (optional), `RUST_LOG` (default `info`). Every flag can also be set through
//! the `CRAWLER_*` variable shown in `--help`.
//!
//! The worker does not run database migrations; the backend applies them at startup.

use std::collections::HashMap;
use std::time::Duration;

use clap::Parser;
use econ_graph_crawler::sources::default_registry;
use econ_graph_crawler::{
    ApiKeys, CrawlCtx, HttpConfig, HttpFetcher, SourceId, SourcePolicy, Worker, WorkerConfig,
};

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

    /// Pause a source after this many consecutive rate-limit/auth errors (0 = never).
    #[arg(long, env = "CRAWLER_PAUSE_AFTER", default_value_t = 5)]
    pause_after: u32,

    /// How long a paused source stays paused, in seconds.
    #[arg(long, env = "CRAWLER_PAUSE_SECS", default_value_t = 300)]
    pause_secs: u64,

    /// Per-request HTTP timeout in seconds.
    #[arg(long, env = "CRAWLER_HTTP_TIMEOUT_SECS", default_value_t = 30)]
    http_timeout_secs: u64,
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
    };
    if registry.ids().is_empty() {
        tracing::warn!("no source adapters registered; every job will fail");
    }

    Worker::new(ctx, registry, config)
        .run(shutdown_signal())
        .await;
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
