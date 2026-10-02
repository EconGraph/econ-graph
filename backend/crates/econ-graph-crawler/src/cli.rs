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
//! crawler coverage [--json] [--fail-under PCT]   # per-source coverage and freshness
//! crawler sources
//! crawler fetch    --source FRED --series GDP [--full]   # one fetch + persist, in-process (debugging)
//! crawler record-reference-seeds --source BLS [--migrations-dir DIR]   # no database needed
//! ```
//!
//! `record-reference-seeds` downloads a source's code lists and writes them, with the `ETag`s
//! they were served with, as a seed migration for new databases (see [`crate::reference_file`]).
//!
//! Diagnostics go through `tracing` (stderr, `RUST_LOG`); stdout carries only the command's output.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context as _};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use econ_graph_core::models::{CrawlQueueItem, JobKind, NewCrawlQueueItem};
use econ_graph_core::DatabasePool;

use crate::adapter::{AdapterRegistry, ApiKeys, CrawlCtx, SourceAdapter};
use crate::coverage::{covered_sources, crawl_coverage, SourceCoverage};
use crate::dataset::DatasetCatalog;
use crate::http::{HttpConfig, HttpFetcher};
use crate::persist;
use crate::policy::SourcePolicy;
use crate::reference_file;
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
    /// Print, per enabled source the refresh scheduler refreshes, series discovered, series with
    /// data, coverage percent, series overdue for refresh and the oldest successful crawl.
    Coverage {
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Exit with an error when any source's coverage is below this percentage (e.g. 95).
        #[arg(long, value_name = "PCT", value_parser = parse_percent)]
        fail_under: Option<f64>,
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
    /// Download a source's code lists and write them as a seed migration that gives a new
    /// database their labels (with the ETags they were served with) before the first crawl.
    RecordReferenceSeeds {
        /// Source, e.g. BLS.
        #[arg(long)]
        source: SourceId,
        /// Diesel migrations directory to write the migration into.
        #[arg(long, default_value = DEFAULT_MIGRATIONS_DIR)]
        migrations_dir: PathBuf,
        /// Write the lists that downloaded even if others failed, leaving the failed ones
        /// unseeded (the crawl still fetches them). Without it, any failure writes nothing.
        #[arg(long)]
        skip_failed: bool,
    },
}

/// Version of the migration that creates `seed_reference_codes`
/// (`2026-10-02-000250_seed_reference_codes`); every seed migration's version must be later.
const SEED_FUNCTION_VERSION: &str = "2026-10-02-000250";

/// `backend/migrations` of the checkout this binary was built from.
const DEFAULT_MIGRATIONS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");

/// Parses a percentage in `[0, 100]`.
fn parse_percent(s: &str) -> Result<f64, String> {
    let p: f64 = s
        .trim()
        .trim_end_matches('%')
        .parse()
        .map_err(|e| format!("{s:?} is not a number: {e}"))?;
    if (0.0..=100.0).contains(&p) {
        Ok(p)
    } else {
        Err(format!("{p} is not between 0 and 100"))
    }
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
        if let Command::RecordReferenceSeeds {
            source,
            migrations_dir,
            skip_failed,
        } = &self.command
        {
            let registry = default_registry();
            let adapter = registry
                .get(*source)
                .ok_or_else(|| anyhow!("no adapter registered for {source}"))?;
            let catalog = DatasetCatalog::load(&registry)?;
            return record_reference_seeds(
                &*adapter,
                &ApiKeys::from_env(),
                &build_http(&registry)?,
                &catalog,
                migrations_dir,
                *skip_failed,
                Utc::now(),
            )
            .await;
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
            Command::Coverage { json, fail_under } => {
                let sources = covered_sources(&default_registry().ids());
                let coverage = crawl_coverage(&pool, &sources).await?;
                let out = if json {
                    format!("{}\n", serde_json::to_string_pretty(&coverage)?)
                } else {
                    format_coverage(&coverage)
                };
                if let Some(target) = fail_under {
                    let below = sources_below(&coverage, target);
                    if !below.is_empty() {
                        // Print the report first, so the failure comes with the numbers.
                        print!("{out}");
                        bail!("coverage below {target}% for {}", below.join(", "));
                    }
                }
                Ok(out)
            }
            Command::Fetch {
                source,
                series,
                full,
            } => fetch_one(pool, source, series.trim(), full).await,
            Command::Sources | Command::RecordReferenceSeeds { .. } => {
                unreachable!("handled above")
            }
        }
    }
}

/// Downloads `adapter`'s code lists and writes them as the migration
/// `{migrations_dir}/{recorded_at}_seed_{source}_reference_codes` (`source` lowercase),
/// replacing the source's previous seed migration; returns what it wrote. If any list fails it
/// writes nothing and lists every failure, unless `skip_failed`.
pub async fn record_reference_seeds(
    adapter: &dyn SourceAdapter,
    keys: &ApiKeys,
    http: &HttpFetcher,
    catalog: &DatasetCatalog,
    migrations_dir: &Path,
    skip_failed: bool,
    recorded_at: DateTime<Utc>,
) -> anyhow::Result<String> {
    let source = adapter.id();
    let lists = adapter.code_lists(keys);
    if lists.is_empty() {
        bail!("{source} publishes no code lists to seed");
    }
    let reference_file::SeedDownload { entries, failures } =
        reference_file::download_seed_entries(http, source, &lists, catalog).await?;
    let failed: Vec<String> = failures
        .iter()
        .map(|(url, e)| format!("{url}: {e}"))
        .collect();
    if !failed.is_empty() && !skip_failed {
        bail!(
            "{} of {} {source} code lists failed (--skip-failed writes the rest): {}",
            failed.len(),
            lists.len(),
            failed.join("; ")
        );
    }
    if entries.is_empty() {
        bail!("every {source} code list failed: {}", failed.join("; "));
    }
    let (up, down) = reference_file::seed_migration_sql(source, recorded_at, &entries)?;
    let suffix = format!("_seed_{}_reference_codes", source.as_str().to_lowercase());
    let name = format!("{}{suffix}", recorded_at.format("%Y-%m-%d-%H%M%S"));
    if recorded_at.format("%Y-%m-%d-%H%M%S").to_string().as_str() <= SEED_FUNCTION_VERSION {
        bail!("{name} would not run after migration {SEED_FUNCTION_VERSION}, which creates seed_reference_codes; check the clock");
    }
    // A new recording replaces the previous one. Both would run on a new database, and the older
    // would store the file first, so the newer one's codes would never load. Removing an applied
    // migration's directory is safe: Diesel only runs versions it hasn't recorded.
    let mut replaced = Vec::new();
    for entry in std::fs::read_dir(migrations_dir)
        .with_context(|| format!("reading {}", migrations_dir.display()))?
    {
        let path = entry?.path();
        if path.is_dir()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(&suffix) && n != name)
        {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("removing {}", path.display()))?;
            replaced.push(path);
        }
    }
    let dir = migrations_dir.join(&name);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(dir.join("up.sql"), up)?;
    std::fs::write(dir.join("down.sql"), down)?;
    let mut out = format!("wrote {}\n", dir.display());
    for path in &replaced {
        let _ = writeln!(out, "  replaced {}", path.display());
    }
    for e in &entries {
        let _ = writeln!(
            out,
            "  {}.{}: {} codes from {} (etag {})",
            e.dataset,
            e.dimension.name,
            e.codes.len(),
            e.url,
            e.etag.as_deref().unwrap_or("none")
        );
    }
    for f in &failed {
        let _ = writeln!(out, "  skipped {f}");
    }
    Ok(out)
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
    let known_vintage = if full || !adapter.tracks_vintages() {
        None
    } else {
        persist::latest_revision_date(&pool, source, external_id).await?
    };
    // A vintage-tracking adapter gets no `since`: with a known vintage it ignores it, and
    // without one it must fetch every date's history.
    let since = if full || adapter.tracks_vintages() {
        None
    } else {
        persist::latest_point_date(&pool, source, external_id)
            .await?
            .and_then(|latest| crate::worker::incremental_since(latest, ctx.http.policy(source)))
    };
    let mut datasets = DatasetCatalog::empty();
    datasets.load_adapter(&*adapter)?;
    persist::sync_datasets(&pool, &datasets).await?;
    tracing::info!(%source, series_id = external_id, ?since, ?known_vintage, "fetching");
    let fetched = adapter
        .fetch_series_incremental(&ctx, external_id, since, known_vintage)
        .await?;
    datasets.check(source, external_id, &fetched.dataset)?;
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

/// Sources in `rows` below `target_percent` (including those with nothing discovered).
pub fn sources_below(rows: &[SourceCoverage], target_percent: f64) -> Vec<&str> {
    rows.iter()
        .filter(|c| !c.meets(target_percent))
        .map(|c| c.source.as_str())
        .collect()
}

/// Text rendering of [`crawl_coverage`]'s result.
pub fn format_coverage(rows: &[SourceCoverage]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<12} {:>10} {:>9} {:>8} {:>7}  oldest success",
        "SOURCE", "DISCOVERED", "WITH DATA", "COVERAGE", "OVERDUE"
    );
    for c in rows {
        let percent = c
            .percent
            .map_or_else(|| "-".to_string(), |p| format!("{p:.1}%"));
        let _ = writeln!(
            out,
            "{:<12} {:>10} {:>9} {:>8} {:>7}  {}",
            c.source,
            c.discovered,
            c.with_data,
            percent,
            c.overdue,
            fmt_time(c.oldest_success)
        );
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
    fn cli_parses_record_reference_seeds() {
        let cli = parse(&["record-reference-seeds", "--source", "bls"]).unwrap();
        assert_eq!(
            cli.command,
            Command::RecordReferenceSeeds {
                source: SourceId::Bls,
                migrations_dir: PathBuf::from(DEFAULT_MIGRATIONS_DIR),
                skip_failed: false,
            }
        );
        // The default is this checkout's migrations directory.
        assert!(Path::new(DEFAULT_MIGRATIONS_DIR)
            .join("00000000000000_diesel_initial_setup")
            .is_dir());
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
            parse(&["coverage"]).unwrap().command,
            Command::Coverage {
                json: false,
                fail_under: None
            }
        );
        assert_eq!(
            parse(&["coverage", "--json", "--fail-under", "95%"])
                .unwrap()
                .command,
            Command::Coverage {
                json: true,
                fail_under: Some(95.0)
            }
        );
        for bad in ["x", "-1", "101"] {
            assert!(parse(&["coverage", "--fail-under", bad]).is_err(), "{bad}");
        }
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
        assert!(out.contains("CENSUS_API_KEY NOT SET (required)"), "{out}");
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
                last_failure: None,
            }],
        };
        let out = format_status(&s);
        assert!(out.contains("active workers: 2"));
        assert!(out
            .lines()
            .any(|l| l.starts_with("FRED") && l.contains(" 3 ") && l.contains(" 4 ")));
    }

    #[test]
    fn cli_coverage_text_rendering() {
        let rows = vec![
            SourceCoverage {
                source: "FRED".into(),
                discovered: 200,
                with_data: 191,
                percent: Some(95.5),
                overdue: 2,
                oldest_success: None,
            },
            SourceCoverage {
                source: "BLS".into(),
                discovered: 0,
                with_data: 0,
                percent: None,
                overdue: 0,
                oldest_success: None,
            },
        ];
        let out = format_coverage(&rows);
        let fred = out.lines().find(|l| l.starts_with("FRED")).unwrap();
        assert!(fred.contains(" 200 ") && fred.contains(" 191 ") && fred.contains("95.5%"));
        let bls = out.lines().find(|l| l.starts_with("BLS")).unwrap();
        assert!(bls.contains(" - "), "{bls}");
        assert_eq!(sources_below(&rows, 95.0), vec!["BLS"]);
        assert_eq!(sources_below(&rows, 96.0), vec!["FRED", "BLS"]);
    }

    #[tokio::test]
    async fn cli_coverage_against_db() {
        // DB-backed; skipped without DATABASE_URL. Other tests leave FRED series behind, so this
        // only checks what its own seeded series guarantees.
        let Some((url, _guard)) = crate::testkit::lock_test_db("cli coverage").await else {
            return;
        };
        let pool = econ_graph_core::create_pool(&url).await.unwrap();
        let fred = persist::data_source_id(&pool, SourceId::Fred)
            .await
            .unwrap();
        #[derive(diesel::QueryableByName)]
        struct WasEnabled {
            #[diesel(sql_type = diesel::sql_types::Bool)]
            was_enabled: bool,
        }
        let was_enabled = {
            use diesel::sql_types::Uuid as SqlUuid;
            use diesel_async::RunQueryDsl;
            let mut conn = pool.get().await.unwrap();
            let prev: WasEnabled = diesel::sql_query(
                "WITH prev AS (SELECT is_enabled FROM data_sources WHERE id = $1) \
                 UPDATE data_sources SET is_enabled = TRUE WHERE id = $1 \
                 RETURNING (SELECT is_enabled FROM prev) AS was_enabled",
            )
            .bind::<SqlUuid, _>(fred)
            .get_result(&mut conn)
            .await
            .unwrap();
            let dataset = econ_graph_core::test_utils::test_dataset_id(&mut conn, fred).await;
            diesel::sql_query(
                "INSERT INTO economic_series \
                   (source_id, external_id, title, frequency, end_date, last_crawled_at, \
                    dataset_id) \
                 VALUES ($1, 't_covcli_1', 't', 'Monthly', DATE '2026-01-01', NOW(), $2) \
                 ON CONFLICT (source_id, external_id) DO UPDATE SET end_date = DATE '2026-01-01', \
                     is_active = TRUE, last_crawled_at = NOW()",
            )
            .bind::<SqlUuid, _>(fred)
            .bind::<SqlUuid, _>(dataset)
            .execute(&mut conn)
            .await
            .unwrap();
            prev.was_enabled
        };
        // Catch a mid-assertion panic so the shared FRED row and t_covcli_1 series are always
        // restored/cleaned up below, instead of leaking into other tests.
        let assertions = futures::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(async {
            let run = |args: &[&str]| {
                let mut full = vec!["crawler", "--database-url", url.as_str()];
                full.extend_from_slice(args);
                Cli::try_parse_from(full).unwrap().execute()
            };

            let out = run(&["coverage", "--json"]).await.unwrap();
            let rows: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
            let expected: Vec<String> = covered_sources(&default_registry().ids())
                .iter()
                .map(|s| s.as_str().to_string())
                .collect();
            for r in &rows {
                let source = r["source"].as_str().unwrap();
                assert!(expected.iter().any(|e| e == source), "{source} not covered");
            }
            let fred_row = rows
                .iter()
                .find(|r| r["source"] == "FRED")
                .expect("FRED row");
            assert!(fred_row["discovered"].as_i64().unwrap() >= 1);
            assert!(fred_row["with_data"].as_i64().unwrap() >= 1);
            assert!(fred_row["oldest_success"].is_string());

            let text = run(&["coverage"]).await.unwrap();
            assert!(text.starts_with("SOURCE"), "{text}");
            assert!(text.lines().any(|l| l.starts_with("FRED ")), "{text}");
        }))
        .await;

        use diesel_async::RunQueryDsl;
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM economic_series WHERE external_id = 't_covcli_1'")
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("UPDATE data_sources SET is_enabled = $2 WHERE id = $1")
            .bind::<diesel::sql_types::Uuid, _>(fred)
            .bind::<diesel::sql_types::Bool, _>(was_enabled)
            .execute(&mut conn)
            .await
            .unwrap();

        if let Err(panic) = assertions {
            std::panic::resume_unwind(panic);
        }
    }

    #[tokio::test]
    async fn cli_enqueue_and_discover_against_db() {
        // DB-backed; skipped without DATABASE_URL. Empties crawl_queue, so it holds the
        // crate-wide DB test lock.
        let Some((url, _guard)) = crate::testkit::lock_test_db("cli").await else {
            return;
        };
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
