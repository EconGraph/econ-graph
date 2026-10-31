// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The `crawler` operator CLI.
//!
//! Discovery and fetch jobs are enqueued here and processed by `crawler-worker`.
//! `confirm-successor` records an operator-reviewed link without changing either history.
//! `accept-guarded-catalog` shows, and with `--accept` applies, the newest catalog snapshot the
//! mass-disappearance guard rejected (see [`crate::lifecycle`]).
//!
//! ```text
//! crawler enqueue  --source FRED --series GDP,UNRATE [--priority N]
//! crawler discover --source FRED [--priority N]
//! crawler refetch  (--source FRED,BLS | --all) [--dry-run] [--completed-since TIME] [--priority N] [--include-inactive] [--json]
//! crawler refetch-status (--source FRED,BLS | --all) [--since TIME] [--failures N] [--json]
//! crawler status   [--json]
//! crawler coverage [--json] [--fail-under PCT]   # per-source coverage and freshness
//! crawler sources
//! crawler fetch    --source FRED --series GDP [--full]   # one fetch + persist, in-process (debugging)
//! crawler record-reference-seeds --source BLS [--migrations-dir DIR]   # no database needed
//! crawler check-selection [--source FRED[,BEA,...]] [--json]   # no database needed
//! crawler migrate  [--dry-run]   # apply the data migrations
//! crawler confirm-successor --source FRED --series OLD --successor NEW
//! crawler accept-guarded-catalog --source BEA [--accept --guarded-at TIME]
//! ```
//!
//! `migrate` applies the data migrations (`backend/migrations/data`) to the data database. No
//! service applies them at startup, so this is how they reach a deployed database. It never
//! touches the app migrations.
//!
//! `refetch` enqueues a forced full-history re-fetch (`refetch_series` jobs, see
//! [`crate::refetch`]) and `refetch-status` reports its per-source progress.
//!
//! `record-reference-seeds` downloads a source's code lists and writes them, with the `ETag`s
//! they were served with, as a seed migration for new databases (see [`crate::reference_file`]).
//!
//! `check-selection` checks curated lists and catalog selection rules against the live upstream
//! catalogs, read-only and without a database (see [`crate::check_selection`]). It exits 0 when
//! every checked source is clean, 1 when any has a problem, 2 when a check could not finish and
//! 3 when the rest are clean but a source was skipped for a missing API key.
//!
//! A command line the CLI can't run (a parse error, or `check-selection --source` naming a
//! source it doesn't check) exits [`EXIT_USAGE`] (64).
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
use econ_graph_core::{DataPool, DatabasePool};

use crate::adapter::{AdapterRegistry, ApiKeys, CrawlCtx, SourceAdapter};
use crate::coverage::{covered_sources, crawl_coverage, SourceCoverage};
use crate::dataset::DatasetCatalog;
use crate::http::{HttpConfig, HttpFetcher};
use crate::licence;
use crate::persist;
use crate::policy::SourcePolicy;
use crate::reference_file;
use crate::refetch::{self, RefetchOptions, SourcePlan, SourceProgress, DEFAULT_REFETCH_PRIORITY};
use crate::source::SourceId;
use crate::sources::default_registry;
use crate::status::{crawler_status, CrawlerStatusSnapshot};

/// `series_id` stored for `discover_catalog` jobs (one per source).
pub const CATALOG_SERIES_ID: &str = "catalog";

/// `check-selection --rate-percent` default: a quarter of each source's policy rate, so the
/// check beside a running worker stays within 125% of it.
pub const DEFAULT_CHECK_RATE_PERCENT: u8 = 25;

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
    /// The data database's Postgres connection string. `DATABASE_URL` is not read. Unused by
    /// sources, record-reference-seeds and check-selection, which need no database.
    #[arg(long, env = "DATA_DATABASE_URL", global = true, hide_env_values = true)]
    pub database_url: Option<String>,

    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// `crawler` subcommands.
#[derive(Debug, Subcommand, PartialEq)]
pub enum Command {
    /// Show the newest catalog snapshot the mass-disappearance guard rejected and the series
    /// it would count as absent. With --accept, apply it as an ordinary complete catalog:
    /// each newly absent series gets one absence (three are still needed to mark it missing).
    AcceptGuardedCatalog {
        /// Source whose guarded snapshot to review, e.g. BEA.
        #[arg(long)]
        source: SourceId,
        /// Apply the snapshot after reviewing it. Without this flag nothing is written.
        #[arg(long, requires = "guarded_at")]
        accept: bool,
        /// The guarded-at time the review printed (RFC 3339). Accepting refuses a different,
        /// newer snapshot.
        #[arg(long)]
        guarded_at: Option<chrono::DateTime<chrono::Utc>>,
    },
    /// Confirm a replacement series after reviewing its meaning and units. Records a link;
    /// never merges observations, deletes a series, or moves annotations.
    ConfirmSuccessor {
        /// Source containing both series, e.g. FRED.
        #[arg(long)]
        source: SourceId,
        /// Existing series whose publication has ended.
        #[arg(long)]
        series: String,
        /// Replacement series id, in the same source.
        #[arg(long)]
        successor: String,
    },
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
    /// Enqueue a forced full-history re-fetch (`refetch_series` jobs) of every series of the
    /// given sources: the worker re-reads each series' whole history, ignoring stored validators
    /// and incremental bounds, and rewrites stored values in place. Idempotent: series with an
    /// active re-fetch job (or, with --completed-since, a completed one) are skipped.
    Refetch {
        #[command(flatten)]
        sources: SourceSelection,
        /// Print what would be enqueued without enqueuing anything.
        #[arg(long)]
        dry_run: bool,
        /// Skip series whose re-fetch completed at or after this RFC 3339 time (to resume a
        /// campaign started then without redoing finished series).
        #[arg(long, value_name = "TIME")]
        completed_since: Option<DateTime<Utc>>,
        /// Queue priority, 1 (lowest) to 10 (highest). Below scheduled refreshes by default.
        #[arg(long, default_value_t = DEFAULT_REFETCH_PRIORITY, value_parser = clap::value_parser!(i32).range(1..=10))]
        priority: i32,
        /// Also re-fetch series marked inactive (retired by the source's catalog).
        #[arg(long)]
        include_inactive: bool,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Print per-source progress of re-fetch jobs: for each series, the state of its latest
    /// refetch_series job.
    RefetchStatus {
        #[command(flatten)]
        sources: SourceSelection,
        /// Only count jobs created at or after this RFC 3339 time.
        #[arg(long, value_name = "TIME")]
        since: Option<DateTime<Utc>>,
        /// Also list up to N failed series with their errors.
        #[arg(long, value_name = "N", default_value_t = 0)]
        failures: i64,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
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
    /// Check curated series lists and catalog selection rules against each source's live
    /// upstream catalog: entries missing upstream, discontinued or copyright-restricted, rules
    /// that match nothing or more than their max_matches, selections past their limits, and the
    /// requests a refresh needs against the source's rate and daily quota. Read-only; needs no
    /// database. Exit status: 0 clean, 1 problems, 2 a check failed, 3 a source was skipped
    /// because its API key isn't set (and nothing worse).
    CheckSelection {
        /// Comma-separated sources to check, e.g. FRED,BEA. Default: FRED, BEA, BLS,
        /// WORLD_BANK, CENSUS and FHFA.
        #[arg(long, value_delimiter = ',', num_args = 1..)]
        source: Vec<SourceId>,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Percent of each source's policy rate, burst and concurrency the check uses. Its rate
        /// limiter is its own, so with the worker running on the same keys the two together send
        /// up to (100 + PCT)% of the policy rate.
        #[arg(long, value_name = "PCT", default_value_t = DEFAULT_CHECK_RATE_PERCENT,
              value_parser = clap::value_parser!(u8).range(1..=100))]
        rate_percent: u8,
        /// Count FRED's whole selection past max_series instead of stopping at it, so an
        /// over-cap selection reports its size (still a problem). The catalog walk stays
        /// bounded by max_catalog_series. Only this check: discovery always stops at the cap.
        #[arg(long)]
        count_uncapped: bool,
    },
    /// Apply the pending data migrations (backend/migrations/data) and print each version
    /// applied, one per line. Never touches the app migrations.
    Migrate {
        /// Print the pending versions, one per line, and change nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

impl Command {
    /// The sources whose upstream this command crawls, or queues a crawl of. `refetch --all`
    /// leaves gated sources out in [`SourceSelection::resolve`].
    fn upstream_sources(&self) -> Vec<SourceId> {
        match self {
            Command::Enqueue { source, .. }
            | Command::Discover { source, .. }
            | Command::Fetch { source, .. }
            | Command::RecordReferenceSeeds { source, .. } => vec![*source],
            Command::Refetch { sources, .. } => sources.source.clone(),
            // Its default list leaves gated sources out (`check_selection_sources`).
            Command::CheckSelection { source, .. } => source.clone(),
            // Read-only, or stored data only with no upstream request: nothing to gate.
            Command::AcceptGuardedCatalog { .. }
            | Command::ConfirmSuccessor { .. }
            | Command::RefetchStatus { .. }
            | Command::Status { .. }
            | Command::Coverage { .. }
            | Command::Sources
            | Command::Migrate { .. } => Vec::new(),
        }
    }
}

/// The sources a `refetch` / `refetch-status` command covers.
#[derive(Debug, Clone, PartialEq, Eq, clap::Args)]
#[group(required = true, multiple = false)]
pub struct SourceSelection {
    /// Comma-separated sources, e.g. FRED,WORLD_BANK.
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    pub source: Vec<SourceId>,
    /// Every source the refresh scheduler refreshes (FRED, BLS, BEA, Census, World Bank, FHFA
    /// in a release build).
    #[arg(long)]
    pub all: bool,
}

impl SourceSelection {
    /// The selected sources, sorted and de-duplicated. A named source must be one the refresh
    /// scheduler refreshes (it has an adapter that fetches series).
    pub fn resolve(&self, registry: &AdapterRegistry) -> anyhow::Result<Vec<SourceId>> {
        let mut fetchable = covered_sources(&registry.ids());
        fetchable.retain(|&s| licence::allows(s));
        if self.all {
            return Ok(fetchable);
        }
        let mut out = self.source.clone();
        out.sort_unstable();
        out.dedup();
        for &source in &out {
            licence::check(source)?;
        }
        if let Some(bad) = out.iter().find(|s| !fetchable.contains(s)) {
            bail!("{bad} has no adapter that fetches series; refetchable sources: {fetchable:?}");
        }
        Ok(out)
    }
}

/// Version of the migration that creates `seed_reference_codes` (the v4.0.0 data baseline,
/// `migrations/data/2026-10-21-000100_v4_0_baseline`); every seed migration's version must be
/// later.
const SEED_FUNCTION_VERSION: &str = "2026-10-21-000100";

/// `backend/migrations/data` of the checkout this binary was built from: seed migrations belong to
/// the data set (docs/development/MIGRATIONS.md).
const DEFAULT_MIGRATIONS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations/data");

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
/// Refuses, inserting nothing, if any job is for a [licence-gated](crate::licence) source.
pub async fn enqueue_all(pool: &DatabasePool, jobs: &[NewCrawlQueueItem]) -> anyhow::Result<usize> {
    for job in jobs {
        if let Ok(source) = job.source.parse::<SourceId>() {
            licence::check(source)?;
        }
    }
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

/// Exit status for a command line the CLI can't run: a parse error, or a source a command
/// doesn't support (`EX_USAGE` from sysexits.h). Distinct from `check-selection`'s statuses.
pub const EXIT_USAGE: u8 = 64;

/// A command that must end with a nonzero exit status other than 1: `check-selection`'s
/// findings (after printing its output) or [`EXIT_USAGE`]. The `crawler` binary exits with
/// `code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitStatus {
    /// The process exit status.
    pub code: u8,
    /// What it means, for the log.
    pub message: String,
}

impl std::fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (exit status {})", self.message, self.code)
    }
}

impl std::error::Error for ExitStatus {}

/// The sources `check-selection --source` names, de-duplicated in report order, or every
/// checked source when it names none.
pub fn check_selection_sources(named: &[SourceId]) -> anyhow::Result<Vec<SourceId>> {
    use crate::check_selection::CHECKED_SOURCES;
    if let Some(bad) = named.iter().find(|s| !CHECKED_SOURCES.contains(s)) {
        return Err(ExitStatus {
            code: EXIT_USAGE,
            message: format!(
                "{bad} has no curated list or selection rules to check; checked sources: {CHECKED_SOURCES:?}"
            ),
        }
        .into());
    }
    Ok(CHECKED_SOURCES
        .into_iter()
        .filter(|s| named.is_empty() || named.contains(s))
        .filter(|&s| licence::allows(s))
        .collect())
}

/// `check-selection`'s output, and the [`ExitStatus`] to end with unless every source is clean.
pub fn check_selection_output(
    report: &crate::check_selection::SelectionReport,
    json: bool,
) -> anyhow::Result<(String, Option<ExitStatus>)> {
    use crate::check_selection::{EXIT_CLEAN, EXIT_FAILED, EXIT_PROBLEMS};
    let out = if json {
        format!("{}\n", serde_json::to_string_pretty(report)?)
    } else {
        report.to_text()
    };
    let code = report.exit_code();
    let message = match code {
        EXIT_CLEAN => return Ok((out, None)),
        EXIT_PROBLEMS => "check-selection found problems",
        EXIT_FAILED => "check-selection could not check every source",
        _ => "check-selection skipped a source whose API key is not set",
    };
    Ok((
        out,
        Some(ExitStatus {
            code,
            message: message.to_string(),
        }),
    ))
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
        for source in self.command.upstream_sources() {
            licence::check(source)?;
        }
        if let Command::CheckSelection {
            source,
            json,
            rate_percent,
            count_uncapped,
        } = &self.command
        {
            let sources = check_selection_sources(source)?;
            let mut checker = crate::check_selection::Checker::live(
                http_config(),
                worker_policies(&default_registry()),
                *rate_percent,
                ApiKeys::from_env(),
                Utc::now().date_naive(),
            )?;
            checker.count_uncapped = *count_uncapped;
            let report = checker.check(&sources).await;
            let (out, status) = check_selection_output(&report, *json)?;
            return match status {
                None => Ok(out),
                Some(status) => {
                    // Print the report first, so the exit status comes with the findings.
                    print!("{out}");
                    Err(status.into())
                }
            };
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
        // A blank value (an empty `DATA_DATABASE_URL`) counts as unset, as in `data_database_url`.
        let url = match self.database_url.filter(|u| !u.trim().is_empty()) {
            Some(url) => url,
            None => econ_graph_core::config::data_database_url()
                .map_err(|e| anyhow!("{e} (or pass --database-url)"))?,
        };
        if let Command::Migrate { dry_run } = self.command {
            return migrate(&url, dry_run).await;
        }
        let pool = DataPool(
            econ_graph_core::create_pool(&url)
                .await
                .context("connecting to the data database")?,
        );

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
            Command::Refetch {
                sources,
                dry_run,
                completed_since,
                priority,
                include_inactive,
                json,
            } => {
                let sources = sources.resolve(&default_registry())?;
                let options = RefetchOptions {
                    priority,
                    include_inactive,
                    completed_since,
                };
                let plans = if dry_run {
                    refetch::plan(&pool, &sources, &options).await?
                } else {
                    refetch::enqueue(&pool, &sources, &options).await?
                };
                if json {
                    Ok(format!("{}\n", serde_json::to_string_pretty(&plans)?))
                } else {
                    Ok(format_refetch_plan(&plans, dry_run))
                }
            }
            Command::RefetchStatus {
                sources,
                since,
                failures,
                json,
            } => {
                let sources = sources.resolve(&default_registry())?;
                let progress = refetch::progress(&pool, &sources, since).await?;
                let failed = if failures > 0 {
                    refetch::failures(&pool, &sources, since, failures).await?
                } else {
                    Vec::new()
                };
                if json {
                    Ok(format!(
                        "{}\n",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "sources": progress,
                            "failures": failed,
                        }))?
                    ))
                } else {
                    let mut out = format_refetch_progress(&progress);
                    for f in &failed {
                        let _ = writeln!(
                            out,
                            "failed {} {}: {}",
                            f.source,
                            f.series_id,
                            f.error_message.as_deref().unwrap_or("-")
                        );
                    }
                    Ok(out)
                }
            }
            Command::Fetch {
                source,
                series,
                full,
            } => fetch_one(pool.0, source, series.trim(), full).await,
            Command::ConfirmSuccessor {
                source,
                series,
                successor,
            } => {
                crate::lifecycle::confirm_successor(&pool, source, series.trim(), successor.trim())
                    .await
                    .context("confirming the reviewed successor")?;
                Ok(format!(
                    "confirmed successor for {source} {series}: {successor}; histories unchanged\n"
                ))
            }
            Command::AcceptGuardedCatalog {
                source,
                accept,
                guarded_at,
            } => {
                let reviewed = if accept { guarded_at } else { None };
                let review = crate::lifecycle::review_guarded_catalog(&pool, source, reviewed)
                    .await
                    .context("reviewing the guarded catalog snapshot")?;
                Ok(format_guarded_review(source, &review, accept))
            }
            Command::Sources
            | Command::RecordReferenceSeeds { .. }
            | Command::CheckSelection { .. }
            | Command::Migrate { .. } => {
                unreachable!("handled above")
            }
        }
    }
}

/// Text for `crawler accept-guarded-catalog`.
fn format_guarded_review(
    source: SourceId,
    review: &crate::lifecycle::GuardedReview,
    accepted: bool,
) -> String {
    let mut out = format!(
        "{source} catalog guarded at {}: {} listed; {} of {} series in the last accepted catalog are absent\n",
        review.guarded_at.to_rfc3339(),
        review.listed,
        review.newly_missing.len(),
        review.previously_listed,
    );
    for id in &review.newly_missing {
        let _ = writeln!(out, "absent {id}");
    }
    if accepted {
        let _ = writeln!(
            out,
            "accepted: one absence recorded per absent series; {} status changes",
            review.transitions
        );
    } else {
        let _ = writeln!(
            out,
            "nothing written; to apply this snapshot: crawler accept-guarded-catalog --source {source} --accept --guarded-at {}",
            review.guarded_at.to_rfc3339()
        );
    }
    out
}

/// `crawler migrate`: applies the data set's pending migrations to `database_url` under the
/// migration advisory lock and returns their versions, one per line; with `dry_run`, returns the
/// pending versions and changes nothing. Fails unless the data set is current afterwards.
pub async fn migrate(database_url: &str, dry_run: bool) -> anyhow::Result<String> {
    use econ_graph_core::MigrationSet;

    let pending = || async {
        econ_graph_core::pending_migration_versions(database_url, MigrationSet::Data)
            .await
            .context("reading the pending data migrations")
    };
    let versions = if dry_run {
        let pending = pending().await?;
        tracing::info!("{} data migration(s) pending", pending.len());
        pending
    } else {
        let applied = econ_graph_core::run_migration_set(database_url, MigrationSet::Data)
            .await
            .context("running the data migrations")?;
        let still_pending = pending().await?;
        if !still_pending.is_empty() {
            bail!(
                "data migrations still pending after the run: {}",
                still_pending.join(", ")
            );
        }
        tracing::info!(
            "applied {} data migration(s); the data set is current",
            applied.len()
        );
        applied
    };
    Ok(versions.iter().map(|v| format!("{v}\n")).collect())
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
    // Seed migrations belong to the data set, and their versions must not collide with the app
    // set's (a database holding both shares one migrations table), so require the set layout
    // (`.../data` next to `.../app`) before downloading anything.
    let app_dir = match (
        migrations_dir.file_name().and_then(|n| n.to_str()),
        migrations_dir.parent(),
    ) {
        (Some("data"), Some(root)) if root.join("app").is_dir() => root.join("app"),
        _ => bail!(
            "{} is not a data migration set: --migrations-dir must be the data/ directory next to \
             an app/ directory (backend/migrations/data)",
            migrations_dir.display()
        ),
    };
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
    let version = recorded_at.format("%Y-%m-%d-%H%M%S").to_string();
    let name = format!("{version}{suffix}");
    if version.as_str() <= SEED_FUNCTION_VERSION {
        bail!("{name} would not run after migration {SEED_FUNCTION_VERSION}, which defines seed_reference_codes; check the clock");
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(migrations_dir)
        .with_context(|| format!("reading {}", migrations_dir.display()))?
    {
        let path = entry?.path();
        if let Some(n) = path.file_name().and_then(|n| n.to_str())
            && path.is_dir()
        {
            names.push((n.to_string(), path.clone()));
        }
    }
    // Diesel identifies a migration by its version alone, so a second migration with this
    // version would be skipped on any database that already ran the first. That includes the app
    // set next to the data set: a database holding both shares one migrations table.
    let mut taken_names: Vec<String> = names.iter().map(|(n, _)| n.clone()).collect();
    for entry in
        std::fs::read_dir(&app_dir).with_context(|| format!("reading {}", app_dir.display()))?
    {
        let path = entry?.path();
        if let (true, Some(n)) = (path.is_dir(), path.file_name().and_then(|n| n.to_str())) {
            taken_names.push(n.to_string());
        }
    }
    // Compared as Diesel compares them: the name up to the first "_", without dashes.
    let diesel_version = |n: &str| n.split('_').next().unwrap_or("").replace('-', "");
    if let Some(taken) = taken_names
        .iter()
        .find(|n| diesel_version(n) == diesel_version(&version))
    {
        bail!("migration version {version} is already taken by {taken}; record again");
    }
    // Write the new recording before removing the old one, so a failed write leaves the
    // previous seed in place.
    let dir = migrations_dir.join(&name);
    std::fs::create_dir(&dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(dir.join("up.sql"), up)
        .and_then(|()| std::fs::write(dir.join("down.sql"), down))
        .with_context(|| format!("writing {}", dir.display()))
        .inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&dir);
        })?;
    // A new recording replaces the previous one. Both would run on a new database, and the older
    // would store the file first, so the newer one's codes would never load. Removing an applied
    // migration's directory is safe: Diesel only runs versions it hasn't recorded.
    let mut replaced = Vec::new();
    for (n, path) in names {
        if n.ends_with(&suffix) {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("removing {}", path.display()))?;
            replaced.push(path);
        }
    }
    let mut out = format!("wrote {}\n", dir.display());
    for path in &replaced {
        let _ = writeln!(out, "  replaced {}", path.display());
    }
    for e in &entries {
        let _ = writeln!(
            out,
            "  {}.{}: {} codes from {} (etag {}, last-modified {})",
            e.dataset,
            e.dimension.name,
            e.codes.len(),
            e.url,
            e.validators.etag.as_deref().unwrap_or("none"),
            e.validators.last_modified.as_deref().unwrap_or("none")
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
        bypass_validators: full,
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
    crate::value::check_fetched_values(external_id, &fetched)?;
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

/// The worker binary's policies: built-in ones overridden by adapters' own.
fn worker_policies(registry: &AdapterRegistry) -> HashMap<SourceId, SourcePolicy> {
    SourceId::ALL
        .into_iter()
        .map(|id| {
            let policy = registry
                .get(id)
                .map_or_else(|| SourcePolicy::default_for(id), |a| a.policy());
            (id, policy)
        })
        .collect()
}

/// The HTTP settings the worker binary uses.
fn http_config() -> HttpConfig {
    HttpConfig {
        timeout: Duration::from_secs(30),
        ..HttpConfig::default()
    }
}

/// The HTTP layer the worker binary builds.
fn build_http(registry: &AdapterRegistry) -> anyhow::Result<HttpFetcher> {
    Ok(HttpFetcher::new(http_config(), worker_policies(registry))?)
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
            // A licence-gated source's queued jobs are never claimed, so say why they wait.
            let gated = p
                .source
                .parse::<SourceId>()
                .ok()
                .and_then(licence::blocking)
                .map_or_else(String::new, |g| format!("  ({}; not claimed)", g.label()));
            let _ = writeln!(
                out,
                "{:<12} {:>8} {:>10} {:>9} {:>10}  {}{gated}",
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

/// Text rendering of [`refetch::plan`] / [`refetch::enqueue`] results.
pub fn format_refetch_plan(plans: &[SourcePlan], dry_run: bool) -> String {
    let mut out = String::new();
    let verb = if dry_run { "WOULD ENQUEUE" } else { "ENQUEUED" };
    let _ = writeln!(
        out,
        "{:<12} {:>8} {:>8} {:>10} {:>14}",
        "SOURCE", "SERIES", "QUEUED", "COMPLETED", verb
    );
    for p in plans {
        let n = if dry_run { p.to_enqueue() } else { p.enqueued };
        let _ = writeln!(
            out,
            "{:<12} {:>8} {:>8} {:>10} {:>14}",
            p.source, p.series, p.already_queued, p.already_completed, n
        );
    }
    out
}

/// Text rendering of [`refetch::progress`].
pub fn format_refetch_progress(rows: &[SourceProgress]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<12} {:>8} {:>8} {:>10} {:>9} {:>10} {:>7} {:>10}  state",
        "SOURCE", "SERIES", "PENDING", "PROCESSING", "RETRYING", "COMPLETED", "FAILED", "CANCELLED"
    );
    for p in rows {
        let state = if p.total() == 0 {
            "none"
        } else if p.finished() {
            "finished"
        } else {
            "running"
        };
        let _ = writeln!(
            out,
            "{:<12} {:>8} {:>8} {:>10} {:>9} {:>10} {:>7} {:>10}  {state}",
            p.source,
            p.total(),
            p.pending,
            p.processing,
            p.retrying,
            p.completed,
            p.failed,
            p.cancelled
        );
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

/// Text rendering of every [`SourceId`] with its policy, adapter, whether this build may crawl it
/// (`disabled: licence (<ticket>)` for a [licence-gated](crate::licence) source) and API key state.
pub fn format_sources(registry: &AdapterRegistry, keys: &ApiKeys) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<12} {:>8} {:>6} {:>5} {:>8}  {:<8} {:<28} API KEY",
        "SOURCE", "REQ/MIN", "BURST", "CONC", "RETRIES", "ADAPTER", "CRAWL"
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
        let crawl = licence::blocking(id).map_or_else(|| "enabled".to_string(), |g| g.label());
        let _ = writeln!(
            out,
            "{:<12} {:>8.1} {:>6} {:>5} {:>8}  {:<8} {:<28} {}",
            id.as_str(),
            policy.requests_per_second * 60.0,
            policy.burst,
            policy.max_concurrency,
            policy.max_retries,
            if adapter.is_some() { "yes" } else { "no" },
            crawl,
            key
        );
    }
    out
}

#[cfg(test)]
mod migrate_tests;

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
    fn check_selection_parses_sources_and_json() {
        assert_eq!(
            parse(&["check-selection"]).unwrap().command,
            Command::CheckSelection {
                source: vec![],
                json: false,
                rate_percent: DEFAULT_CHECK_RATE_PERCENT,
                count_uncapped: false,
            }
        );
        assert_eq!(
            parse(&["check-selection", "--source", "BEA,FRED", "--json"])
                .unwrap()
                .command,
            Command::CheckSelection {
                source: vec![SourceId::Bea, SourceId::Fred],
                json: true,
                rate_percent: DEFAULT_CHECK_RATE_PERCENT,
                count_uncapped: false,
            }
        );
        assert!(matches!(
            parse(&["check-selection", "--rate-percent", "50"])
                .unwrap()
                .command,
            Command::CheckSelection {
                rate_percent: 50,
                ..
            }
        ));
        assert!(matches!(
            parse(&["check-selection", "--source", "FRED", "--count-uncapped"])
                .unwrap()
                .command,
            Command::CheckSelection {
                count_uncapped: true,
                ..
            }
        ));
        for bad in ["0", "101"] {
            assert!(
                parse(&["check-selection", "--rate-percent", bad]).is_err(),
                "{bad}"
            );
        }
        assert!(parse(&["check-selection", "--source", "NOPE"]).is_err());
        // No database URL is needed to parse it (or to run it).
        assert!(Cli::try_parse_from(["crawler", "check-selection"]).is_ok());
    }

    /// `check-selection --source` goes through the licence gate like the other crawling
    /// commands; with no `--source` its default list leaves gated sources out.
    #[test]
    fn check_selection_names_its_sources_to_the_licence_gate() {
        let cli = parse(&["check-selection", "--source", "FRED,BEA"]).unwrap();
        assert_eq!(
            cli.command.upstream_sources(),
            [SourceId::Fred, SourceId::Bea]
        );
        let cli = parse(&["check-selection"]).unwrap();
        assert!(cli.command.upstream_sources().is_empty());
        for source in check_selection_sources(&[]).unwrap() {
            assert!(licence::allows(source), "{source}");
        }
    }

    #[test]
    fn check_selection_sources_default_to_every_checked_source_in_report_order() {
        use crate::check_selection::CHECKED_SOURCES;
        assert_eq!(check_selection_sources(&[]).unwrap(), CHECKED_SOURCES);
        assert_eq!(
            check_selection_sources(&[SourceId::Fhfa, SourceId::Fred, SourceId::Fred]).unwrap(),
            [SourceId::Fred, SourceId::Fhfa]
        );
        let err = check_selection_sources(&[SourceId::Sec]).unwrap_err();
        let status = err.downcast_ref::<ExitStatus>().unwrap();
        assert_eq!(status.code, EXIT_USAGE);
        assert!(status.message.contains("no curated list"), "{status}");
    }

    #[test]
    fn check_selection_output_sets_the_exit_status() {
        use crate::check_selection::{SelectionReport, EXIT_SKIPPED};
        let clean = SelectionReport { sources: vec![] };
        let (out, status) = check_selection_output(&clean, true).unwrap();
        assert_eq!(status, None);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&out).unwrap(),
            serde_json::json!({"sources": []})
        );
        let skipped = SelectionReport {
            sources: vec![skipped_check()],
        };
        let (_, status) = check_selection_output(&skipped, false).unwrap();
        let status = status.unwrap();
        assert_eq!(status.code, EXIT_SKIPPED);
        let err: anyhow::Error = status.into();
        assert_eq!(err.downcast_ref::<ExitStatus>().unwrap().code, EXIT_SKIPPED);
    }

    fn skipped_check() -> crate::check_selection::SourceCheck {
        crate::check_selection::SourceCheck {
            source: SourceId::Fred,
            status: crate::check_selection::CheckStatus::Skipped,
            skipped: Some("no API key".into()),
            error: None,
            curated: None,
            selected: None,
            rules: vec![],
            bounds: vec![],
            problems: vec![],
            notices: vec![],
            refresh: None,
        }
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn guarded_catalog_acceptance_needs_an_explicit_flag() {
        assert_eq!(
            parse(&["accept-guarded-catalog", "--source", "BEA"])
                .unwrap()
                .command,
            Command::AcceptGuardedCatalog {
                source: SourceId::Bea,
                accept: false,
                guarded_at: None
            }
        );
        assert!(parse(&["accept-guarded-catalog", "--source", "BEA", "--accept"]).is_err());
        assert_eq!(
            parse(&[
                "accept-guarded-catalog",
                "--source",
                "BEA",
                "--accept",
                "--guarded-at",
                "2026-10-30T18:02:15.123456+00:00"
            ])
            .unwrap()
            .command,
            Command::AcceptGuardedCatalog {
                source: SourceId::Bea,
                accept: true,
                guarded_at: Some("2026-10-30T18:02:15.123456Z".parse().unwrap())
            }
        );
        assert!(parse(&["accept-guarded-catalog"]).is_err());
    }

    #[test]
    fn successor_confirmation_requires_both_series_and_source() {
        assert_eq!(
            parse(&[
                "confirm-successor",
                "--source",
                "FRED",
                "--series",
                "OLD",
                "--successor",
                "NEW"
            ])
            .unwrap()
            .command,
            Command::ConfirmSuccessor {
                source: SourceId::Fred,
                series: "OLD".into(),
                successor: "NEW".into()
            }
        );
        assert!(parse(&["confirm-successor", "--source", "FRED", "--series", "OLD"]).is_err());
        assert!(parse(&["confirm-successor", "--series", "OLD", "--successor", "NEW"]).is_err());
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
    fn cli_parses_refetch() {
        let cli = parse(&["refetch", "--source", "fred,world_bank", "--dry-run"]).unwrap();
        assert_eq!(
            cli.command,
            Command::Refetch {
                sources: SourceSelection {
                    source: vec![SourceId::Fred, SourceId::WorldBank],
                    all: false,
                },
                dry_run: true,
                completed_since: None,
                priority: DEFAULT_REFETCH_PRIORITY,
                include_inactive: false,
                json: false,
            }
        );
        let cli = parse(&[
            "refetch",
            "--all",
            "--completed-since",
            "2026-10-26T00:00:00Z",
            "--priority",
            "2",
        ])
        .unwrap();
        let Command::Refetch {
            sources,
            completed_since,
            priority,
            ..
        } = cli.command
        else {
            panic!("not refetch");
        };
        assert!(sources.all);
        assert_eq!(
            completed_since,
            Some("2026-10-26T00:00:00Z".parse().unwrap())
        );
        assert_eq!(priority, 2);
        // A source selection is required, and --source and --all exclude each other.
        assert!(parse(&["refetch"]).is_err());
        assert!(parse(&["refetch", "--all", "--source", "FRED"]).is_err());
        assert!(parse(&["refetch-status", "--all", "--failures", "20", "--json"]).is_ok());
    }

    #[test]
    fn refetch_sources_resolve_to_fetchable_sources() {
        let registry = default_registry();
        let all = SourceSelection {
            source: Vec::new(),
            all: true,
        };
        let resolved = all.resolve(&registry).unwrap();
        for s in [
            SourceId::Fred,
            SourceId::Bls,
            SourceId::Bea,
            SourceId::Census,
            SourceId::WorldBank,
            SourceId::Fhfa,
        ] {
            assert!(resolved.contains(&s), "{s}");
        }
        assert!(!resolved.contains(&SourceId::Sec));
        let named = SourceSelection {
            source: vec![SourceId::WorldBank, SourceId::Fred, SourceId::Fred],
            all: false,
        };
        assert_eq!(
            named.resolve(&registry).unwrap(),
            vec![SourceId::Fred, SourceId::WorldBank]
        );
        let sec = SourceSelection {
            source: vec![SourceId::Sec],
            all: false,
        };
        assert!(sec.resolve(&registry).is_err());
    }

    #[test]
    fn refetch_reports_render() {
        let plan = vec![SourcePlan {
            source: "FRED".into(),
            series: 10,
            already_queued: 2,
            already_completed: 3,
            enqueued: 0,
        }];
        let text = format_refetch_plan(&plan, true);
        assert!(text.contains("WOULD ENQUEUE"), "{text}");
        assert!(
            text.lines().nth(1).unwrap().trim_end().ends_with(" 5"),
            "{text}"
        );
        let progress = vec![SourceProgress {
            source: "FRED".into(),
            completed: 4,
            failed: 1,
            ..SourceProgress::default()
        }];
        let text = format_refetch_progress(&progress);
        assert!(text.contains("finished"), "{text}");
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
        // The default is this checkout's data migrations directory.
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
            parse(&["migrate"]).unwrap().command,
            Command::Migrate { dry_run: false }
        );
        assert_eq!(
            parse(&["migrate", "--dry-run"]).unwrap().command,
            Command::Migrate { dry_run: true }
        );
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
