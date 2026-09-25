// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The `crawler` operator CLI.
//!
//! Everything except `fetch` only touches `crawl_queue`: jobs are enqueued here and processed by
//! the `crawler-worker` deployment.
//!
//! ```text
//! crawler enqueue  --source FRED --series GDP,UNRATE [--priority N]
//! crawler discover --source FRED [--priority N]
//! crawler status   [--json]
//! crawler sources
//! crawler fetch    --source FRED --series GDP [--full]   # one fetch + persist, in-process (debugging)
//! ```
//!
//! Diagnostics go through `tracing` (stderr, `RUST_LOG`); stdout carries only the command's output.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::Duration;

use anyhow::{anyhow, bail, Context as _};
use clap::{Parser, Subcommand};
use econ_graph_core::models::{CrawlQueueItem, JobKind, NewCrawlQueueItem};
use econ_graph_core::DatabasePool;

use crate::adapter::{AdapterRegistry, ApiKeys, CrawlCtx};
use crate::http::{HttpConfig, HttpFetcher};
use crate::persist;
use crate::policy::SourcePolicy;
use crate::source::SourceId;
use crate::sources::default_registry;
use crate::status::{crawler_status, CrawlerStatusSnapshot};

/// `series_id` stored for `discover_catalog` jobs (one per source).
pub const CATALOG_SERIES_ID: &str = "catalog";

/// Default queue priority (1 = lowest, 10 = highest).
pub const DEFAULT_PRIORITY: i32 = 5;

/// `crawler` command line.
#[derive(Debug, Parser, PartialEq)]
#[command(
    name = "crawler",
    version,
    about = "Enqueue crawl jobs and inspect the crawl queue"
)]
pub struct Cli {
    /// Postgres connection string.
    #[arg(long, env = "DATABASE_URL", global = true, hide_env_values = true)]
    pub database_url: Option<String>,

    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// `crawler` subcommands.
#[derive(Debug, Subcommand, PartialEq)]
pub enum Command {
    /// Enqueue fetch_series jobs.
    Enqueue {
        /// Source, e.g. FRED, BLS, WORLD_BANK.
        #[arg(long)]
        source: SourceId,
        /// Comma-separated series ids, e.g. GDP,UNRATE.
        #[arg(long, value_delimiter = ',', required = true, num_args = 1..)]
        series: Vec<String>,
        /// Queue priority, 1 (lowest) to 10 (highest).
        #[arg(long, default_value_t = DEFAULT_PRIORITY, value_parser = clap::value_parser!(i32).range(1..=10))]
        priority: i32,
    },
    /// Enqueue a discover_catalog job for a source.
    Discover {
        /// Source, e.g. FRED.
        #[arg(long)]
        source: SourceId,
        /// Queue priority, 1 (lowest) to 10 (highest).
        #[arg(long, default_value_t = DEFAULT_PRIORITY, value_parser = clap::value_parser!(i32).range(1..=10))]
        priority: i32,
    },
    /// Print crawler status derived from crawl_queue.
    Status {
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// List known sources, their policy and whether their API key is configured.
    Sources,
    /// Fetch ONE series now, in this process, through the adapter and the worker's persistence
    /// (for debugging; bypasses the queue).
    Fetch {
        /// Source, e.g. FRED.
        #[arg(long)]
        source: SourceId,
        /// Series id, e.g. GDP.
        #[arg(long)]
        series: String,
        /// Ignore stored observations and fetch the full history.
        #[arg(long)]
        full: bool,
    },
}

/// Environment variable holding `source`'s API key, if the source uses one.
pub fn api_key_env_var(source: SourceId) -> Option<&'static str> {
    match source {
        SourceId::Fred => Some("FRED_API_KEY"),
        SourceId::Bls => Some("BLS_API_KEY"),
        SourceId::Bea => Some("BEA_API_KEY"),
        SourceId::Census => Some("CENSUS_API_KEY"),
        _ => None,
    }
}

/// Queue row for one job, with `max_retries` from the source's default policy.
pub fn new_job(
    source: SourceId,
    series_id: &str,
    kind: JobKind,
    priority: i32,
) -> NewCrawlQueueItem {
    let max_retries = SourcePolicy::default_for(source).max_retries.min(10) as i32;
    NewCrawlQueueItem {
        source: source.as_str().to_string(),
        series_id: series_id.to_string(),
        priority,
        max_retries,
        scheduled_for: None,
        kind: kind.as_str().to_string(),
    }
}

/// Enqueues `jobs`; returns how many were inserted (jobs with an active duplicate are skipped).
pub async fn enqueue_all(pool: &DatabasePool, jobs: &[NewCrawlQueueItem]) -> anyhow::Result<usize> {
    let mut inserted = 0;
    for job in jobs {
        match CrawlQueueItem::enqueue(pool, job).await? {
            Some(item) => {
                tracing::debug!(id = %item.id, source = %item.source, series_id = %item.series_id, kind = %item.kind, "enqueued");
                inserted += 1;
            }
            None => {
                tracing::info!(source = %job.source, series_id = %job.series_id, kind = %job.kind, "already queued; skipped");
            }
        }
    }
    Ok(inserted)
}

/// Trims, drops empty entries and de-duplicates (keeping order).
fn clean_series(series: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in series.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if !out.iter().any(|o| o == s) {
            out.push(s.to_string());
        }
    }
    out
}

impl Cli {
    /// Runs the command, writing its output to stdout.
    pub async fn run(self) -> anyhow::Result<()> {
        let out = self.execute().await?;
        print!("{out}");
        Ok(())
    }

    /// Runs the command and returns what it would print.
    pub async fn execute(self) -> anyhow::Result<String> {
        if self.command == Command::Sources {
            return Ok(format_sources(&default_registry(), &ApiKeys::from_env()));
        }
        let url = self
            .database_url
            .ok_or_else(|| anyhow!("DATABASE_URL is not set (or pass --database-url)"))?;
        let pool = econ_graph_core::create_pool(&url)
            .await
            .context("connecting to the database")?;

        match self.command {
            Command::Enqueue {
                source,
                series,
                priority,
            } => {
                let series = clean_series(&series);
                if series.is_empty() {
                    bail!("--series must name at least one series id");
                }
                let jobs: Vec<_> = series
                    .iter()
                    .map(|s| new_job(source, s, JobKind::FetchSeries, priority))
                    .collect();
                let n = enqueue_all(&pool, &jobs).await?;
                Ok(format!(
                    "enqueued {n} of {} fetch_series job(s) for {source}\n",
                    jobs.len()
                ))
            }
            Command::Discover { source, priority } => {
                let job = new_job(
                    source,
                    CATALOG_SERIES_ID,
                    JobKind::DiscoverCatalog,
                    priority,
                );
                let n = enqueue_all(&pool, &[job]).await?;
                Ok(if n == 1 {
                    format!("enqueued discover_catalog for {source}\n")
                } else {
                    format!("discover_catalog for {source} is already queued\n")
                })
            }
            Command::Status { json } => {
                let snapshot = crawler_status(&pool).await?;
                if json {
                    Ok(format!("{}\n", serde_json::to_string_pretty(&snapshot)?))
                } else {
                    Ok(format_status(&snapshot))
                }
            }
            Command::Fetch {
                source,
                series,
                full,
            } => fetch_one(pool, source, series.trim(), full).await,
            Command::Sources => unreachable!("handled above"),
        }
    }
}

/// One fetch + persist, the same calls the worker makes for a `fetch_series` job.
async fn fetch_one(
    pool: DatabasePool,
    source: SourceId,
    external_id: &str,
    full: bool,
) -> anyhow::Result<String> {
    if external_id.is_empty() {
        bail!("--series must not be empty");
    }
    let registry = default_registry();
    let adapter = registry
        .get(source)
        .ok_or_else(|| anyhow!("no adapter registered for {source}"))?;
    let ctx = CrawlCtx {
        http: build_http(&registry)?,
        pool: pool.clone(),
        keys: ApiKeys::from_env(),
    };
    let since = if full {
        None
    } else {
        persist::latest_point_date(&pool, source, external_id).await?
    };
    tracing::info!(%source, series_id = external_id, ?since, "fetching");
    let fetched = adapter.fetch_series(&ctx, external_id, since).await?;
    let write = persist::persist_series(&pool, source, external_id, &fetched).await?;
    Ok(format!(
        "{source} {external_id}: {} point(s) written ({} new), latest {}, series {}{}\n",
        write.points_upserted,
        write.points_new,
        write
            .latest_date
            .map_or_else(|| "-".to_string(), |d| d.to_string()),
        write.series_id,
        if write.series_created {
            " (created)"
        } else {
            ""
        },
    ))
}

/// The HTTP layer the worker binary builds: built-in policies overridden by adapters' own.
fn build_http(registry: &AdapterRegistry) -> anyhow::Result<HttpFetcher> {
    let policies: HashMap<SourceId, SourcePolicy> = SourceId::ALL
        .into_iter()
        .map(|id| {
            let policy = registry
                .get(id)
                .map_or_else(|| SourcePolicy::default_for(id), |a| a.policy());
            (id, policy)
        })
        .collect();
    Ok(HttpFetcher::new(
        HttpConfig {
            timeout: Duration::from_secs(30),
            ..HttpConfig::default()
        },
        policies,
    )?)
}

fn fmt_time(t: Option<chrono::DateTime<chrono::Utc>>) -> String {
    t.map_or_else(|| "-".to_string(), |t| t.to_rfc3339())
}

/// Text rendering of a [`CrawlerStatusSnapshot`].
pub fn format_status(s: &CrawlerStatusSnapshot) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "running:        {}", s.is_running);
    let _ = writeln!(out, "active workers: {}", s.active_workers);
    let _ = writeln!(out, "last crawl:     {}", fmt_time(s.last_crawl));
    let _ = writeln!(out, "next due:       {}", fmt_time(s.next_scheduled_crawl));
    if !s.per_source.is_empty() {
        let _ = writeln!(
            out,
            "\n{:<12} {:>8} {:>10} {:>9} {:>10}  last success",
            "SOURCE", "PENDING", "PROCESSING", "RETRYING", "FAILED/24H"
        );
        for p in &s.per_source {
            let _ = writeln!(
                out,
                "{:<12} {:>8} {:>10} {:>9} {:>10}  {}",
                p.source,
                p.pending,
                p.processing,
                p.retrying,
                p.failed_24h,
                fmt_time(p.last_success)
            );
        }
    }
    out
}

/// Text rendering of every [`SourceId`] with its policy, adapter and API key state.
pub fn format_sources(registry: &AdapterRegistry, keys: &ApiKeys) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<12} {:>8} {:>6} {:>5} {:>8}  {:<8} API KEY",
        "SOURCE", "REQ/MIN", "BURST", "CONC", "RETRIES", "ADAPTER"
    );
    for id in SourceId::ALL {
        let adapter = registry.get(id);
        let policy = adapter
            .as_ref()
            .map_or_else(|| SourcePolicy::default_for(id), |a| a.policy());
        let key = match api_key_env_var(id) {
            Some(var) => {
                let set = if keys.get(id).is_some() {
                    "set"
                } else {
                    "NOT SET"
                };
                let required = if policy.needs_api_key {
                    "required"
                } else {
                    "optional"
                };
                format!("{var} {set} ({required})")
            }
            None if policy.needs_api_key => "required (no env var)".to_string(),
            None => "-".to_string(),
        };
        let _ = writeln!(
            out,
            "{:<12} {:>8.1} {:>6} {:>5} {:>8}  {:<8} {}",
            id.as_str(),
            policy.requests_per_second * 60.0,
            policy.burst,
            policy.max_concurrency,
            policy.max_retries,
            if adapter.is_some() { "yes" } else { "no" },
            key
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        let mut full = vec!["crawler", "--database-url", "postgres://x"];
        full.extend_from_slice(args);
        Cli::try_parse_from(full)
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn cli_parses_enqueue() {
        let cli = parse(&["enqueue", "--source", "fred", "--series", "GDP,UNRATE"]).unwrap();
        assert_eq!(
            cli.command,
            Command::Enqueue {
                source: SourceId::Fred,
                series: vec!["GDP".into(), "UNRATE".into()],
                priority: DEFAULT_PRIORITY,
            }
        );
        assert_eq!(cli.database_url.as_deref(), Some("postgres://x"));
    }

    #[test]
    fn cli_parses_enqueue_priority_and_world_bank() {
        let cli = parse(&[
            "enqueue",
            "--source",
            "WORLD_BANK",
            "--series",
            "NY.GDP.MKTP.CD",
            "--priority",
            "9",
        ])
        .unwrap();
        assert_eq!(
            cli.command,
            Command::Enqueue {
                source: SourceId::WorldBank,
                series: vec!["NY.GDP.MKTP.CD".into()],
                priority: 9,
            }
        );
    }

    #[test]
    fn cli_rejects_bad_enqueue_args() {
        assert!(parse(&["enqueue", "--source", "NOPE", "--series", "GDP"]).is_err());
        assert!(
            parse(&["enqueue", "--source", "FRED"]).is_err(),
            "series required"
        );
        assert!(
            parse(&["enqueue", "--series", "GDP"]).is_err(),
            "source required"
        );
        for p in ["0", "11", "x"] {
            assert!(
                parse(&[
                    "enqueue",
                    "--source",
                    "FRED",
                    "--series",
                    "GDP",
                    "--priority",
                    p
                ])
                .is_err(),
                "priority {p}"
            );
        }
    }

    #[test]
    fn cli_parses_other_commands() {
        assert_eq!(
            parse(&["discover", "--source", "BLS"]).unwrap().command,
            Command::Discover {
                source: SourceId::Bls,
                priority: DEFAULT_PRIORITY
            }
        );
        assert_eq!(
            parse(&["status"]).unwrap().command,
            Command::Status { json: false }
        );
        assert_eq!(
            parse(&["status", "--json"]).unwrap().command,
            Command::Status { json: true }
        );
        assert_eq!(parse(&["sources"]).unwrap().command, Command::Sources);
        assert_eq!(
            parse(&["fetch", "--source", "FRED", "--series", "GDP"])
                .unwrap()
                .command,
            Command::Fetch {
                source: SourceId::Fred,
                series: "GDP".into(),
                full: false
            }
        );
        assert!(parse(&["fetch", "--source", "FRED"]).is_err());
        assert!(parse(&[]).is_err(), "subcommand required");
    }

    #[test]
    fn cli_clean_series_trims_and_dedupes() {
        let s = clean_series(&[" GDP".into(), "".into(), "UNRATE".into(), "GDP ".into()]);
        assert_eq!(s, vec!["GDP".to_string(), "UNRATE".to_string()]);
    }

    #[test]
    fn cli_new_job_uses_policy_retries_and_kind() {
        let job = new_job(
            SourceId::Bls,
            CATALOG_SERIES_ID,
            JobKind::DiscoverCatalog,
            7,
        );
        assert_eq!(job.source, "BLS");
        assert_eq!(job.series_id, "catalog");
        assert_eq!(job.kind, "discover_catalog");
        assert_eq!(job.priority, 7);
        assert_eq!(
            job.max_retries,
            SourcePolicy::default_for(SourceId::Bls).max_retries as i32
        );
    }

    #[test]
    fn cli_sources_lists_every_source_and_key_state() {
        let keys = ApiKeys {
            fred: Some("secret-value".into()),
            ..ApiKeys::default()
        };
        let out = format_sources(&default_registry(), &keys);
        for id in SourceId::ALL {
            assert!(out.contains(id.as_str()), "{id} missing:\n{out}");
        }
        assert!(out.contains("FRED_API_KEY set (required)"), "{out}");
        assert!(out.contains("BEA_API_KEY NOT SET (required)"), "{out}");
        assert!(!out.contains("secret-value"), "never print key values");
    }

    #[test]
    fn cli_status_text_rendering() {
        let s = CrawlerStatusSnapshot {
            active_workers: 2,
            is_running: true,
            last_crawl: None,
            next_scheduled_crawl: None,
            per_source: vec![crate::status::SourceQueueStatus {
                source: "FRED".into(),
                pending: 3,
                processing: 1,
                retrying: 0,
                failed_24h: 4,
                last_success: None,
            }],
        };
        let out = format_status(&s);
        assert!(out.contains("active workers: 2"));
        assert!(out
            .lines()
            .any(|l| l.starts_with("FRED") && l.contains(" 3 ") && l.contains(" 4 ")));
    }

    #[tokio::test]
    async fn cli_enqueue_and_discover_against_db() {
        // DB-backed; skipped without DATABASE_URL. Empties crawl_queue (use --test-threads=1).
        let Ok(url) = std::env::var("DATABASE_URL") else {
            return;
        };
        econ_graph_core::run_migrations(&url).await.unwrap();
        let pool = econ_graph_core::create_pool(&url).await.unwrap();
        {
            use diesel_async::RunQueryDsl;
            let mut conn = pool.get().await.unwrap();
            diesel::sql_query("DELETE FROM crawl_queue")
                .execute(&mut conn)
                .await
                .unwrap();
        }
        let run = |args: &[&str]| {
            let mut full = vec!["crawler", "--database-url", url.as_str()];
            full.extend_from_slice(args);
            Cli::try_parse_from(full).unwrap().execute()
        };
        let out = run(&["enqueue", "--source", "FRED", "--series", "GDP,UNRATE,GDP"])
            .await
            .unwrap();
        assert_eq!(out, "enqueued 2 of 2 fetch_series job(s) for FRED\n");
        let out = run(&["enqueue", "--source", "FRED", "--series", "GDP,PAYEMS"])
            .await
            .unwrap();
        assert_eq!(out, "enqueued 1 of 2 fetch_series job(s) for FRED\n");
        let out = run(&["discover", "--source", "FRED"]).await.unwrap();
        assert_eq!(out, "enqueued discover_catalog for FRED\n");
        let out = run(&["discover", "--source", "FRED"]).await.unwrap();
        assert_eq!(out, "discover_catalog for FRED is already queued\n");
        let out = run(&["status", "--json"]).await.unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["per_source"][0]["source"], "FRED");
        assert_eq!(v["per_source"][0]["pending"], 4);
    }
}
