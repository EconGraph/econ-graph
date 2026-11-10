// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Historical backfill of Massive (market data vendor) unadjusted daily bars into the Iceberg
//! `stock_price_daily` dataset, for every symbol in the [`SymbolTable`].
//!
//! `crawler massive-backfill` runs it in process, as a one-off Kubernetes Job
//! (`platform/application/massive-backfill.yaml`). The crawl queue's job kinds don't fit it:
//! `refetch_series` re-reads one series per job, so a ticker's six fields would cost six
//! requests, and the queue has no depth or grouped-daily control.
//!
//! - **Window.** It ends on the last trading day before the daily crawl's first stored day, or
//!   before today when the daily crawl has stored nothing, so the two paths never write the same
//!   day. It starts `--years` earlier (default [`DEFAULT_YEARS`]). If the plan's history is
//!   shorter, the start is clamped to the oldest date the plan serves
//!   ([`ClientError::OutsidePlanWindow`] marks the limit), and the run continues.
//! - **Mode.** It makes one per-ticker range request per symbol, or one grouped-daily request per
//!   trading day, whichever needs fewer ([`choose_mode`]). Per-ticker also writes fewer, larger
//!   files, so it wins ties.
//! - **Bounded commits.** Each Iceberg commit holds at most `--batch-rows` observations, and never
//!   more than `ICEBERG_MAX_SCAN_ROWS`.
//! - **Resumable, no duplicates.** The run's window is stored in `massive_backfill_runs` when it
//!   starts. Each finished unit (a share class in per-ticker mode, a date in grouped mode) gets a
//!   `massive_backfill_units` row, in the same transaction as the Iceberg pointer
//!   compare-and-swap that publishes its rows. A crash therefore leaves each unit either fully
//!   published and recorded, or neither. A rerun resumes the open run and skips recorded units.
//!   Commit ids are UUIDv5 of the run and its units, so a retried commit is idempotent too. Once
//!   a run finishes, a later run loads only days before the earliest finished window, so raising
//!   `--years` adds older history without rewriting what is stored.
//! - **Each row written once.** Rows go straight to the Iceberg append, never through
//!   `data_points` or the worker's mirror, which re-appends a series' whole history on every
//!   publish (ECO-875). Series rows are created only when missing, since upserting an existing
//!   series would mirror its daily-crawl history again.
//! - **Request budget.** Requests go through the run's own [`HttpFetcher`] at
//!   `--requests-per-second`. That limiter is separate from the worker's, so the daily refresh
//!   keeps its own budget. A rate-limited response is retried after its `Retry-After`.
//! - **Metrics.** `crawler_backfill_days_remaining{source}` and
//!   `crawler_backfill_runs_total{source,outcome}`.
//!
//! The run and unit tables, with each unit's commit id, list every Iceberg commit the backfill
//! made. A purge, which Massive's terms require when the contract ends, starts from them.
//!
//! [`HttpFetcher`]: crate::http::HttpFetcher

pub mod estimate;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use anyhow::{bail, Context as _};
use chrono::{Datelike, NaiveDate, Utc};
use diesel::sql_types::{Date, Integer, Nullable, Text, Uuid as SqlUuid};
use diesel::QueryableByName;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use econ_graph_core::models::Observation;
use econ_graph_core::observation_storage::{ObservationBackend, ObservationStorage};
use econ_graph_core::DatabasePool;
use econ_graph_metrics::crawler::CRAWLER_SOURCE_HEALTH_METRICS;
use econ_graph_timeseries::SqlCatalog;
use serde::Serialize;
use uuid::Uuid;

use super::calendar;
use super::client::{ClientError, DailyBars};
use super::dataset::{self, DailyBar, PriceField};
use super::symbols::{SymbolRow, SymbolTable};
use crate::source::SourceId;

/// Default `--years`: the Starter tier's history.
pub const DEFAULT_YEARS: u32 = 5;
/// Default `--requests-per-second`. Paid tiers have no hard limit; Massive asks clients to stay
/// well under about 100 per second, and the daily refresh shares the key.
pub const DEFAULT_REQUESTS_PER_SECOND: f64 = 10.0;
/// Observations per bar: one per [`PriceField`] (`vwap` may be absent).
pub const FIELDS: usize = 6;
/// Retries of one request that was rate limited, before the run fails.
const MAX_RATE_LIMIT_RETRIES: u32 = 8;
/// Wait after a rate-limited response that gave no usable `Retry-After`.
const DEFAULT_RATE_LIMIT_WAIT: Duration = Duration::from_secs(15);
/// When the plan's history is shorter than requested, the clamped start moves this many months
/// further forward, so the plan's rolling window does not pass it during the run.
const PLAN_WINDOW_MARGIN_MONTHS: u32 = 1;
/// Session advisory lock key: one backfill at a time ("MASSIVEB" in ASCII).
const RUN_LOCK_KEY: i64 = 0x4d41_5353_4956_4542;
/// Namespace of commit ids: UUIDv5 of `https://econgraph.com/ns/massive-backfill-commit` in the
/// URL namespace. Fixed forever.
pub const COMMIT_ID_NAMESPACE: Uuid = Uuid::from_u128(0x8ec341bb_7e5d_5299_8c23_19acd7a4f377);

/// How bars are requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum FetchMode {
    /// One range request per ticker for the whole window.
    TickerRange,
    /// One grouped-daily request per trading day, for every ticker at once.
    GroupedDaily,
}

impl FetchMode {
    /// The value stored in `massive_backfill_runs.mode`.
    pub fn as_str(self) -> &'static str {
        match self {
            FetchMode::TickerRange => "ticker_range",
            FetchMode::GroupedDaily => "grouped_daily",
        }
    }

    fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "ticker_range" => Ok(FetchMode::TickerRange),
            "grouped_daily" => Ok(FetchMode::GroupedDaily),
            other => bail!("unknown backfill mode {other:?}"),
        }
    }
}

/// The cheaper mode: per-ticker needs one request per ticker segment (a share class whose ticker
/// changed in the window has one segment per ticker), grouped-daily one per trading day.
/// Per-ticker wins ties: it writes one file per series instead of one per series per commit.
pub fn choose_mode(ticker_requests: usize, trading_days: usize) -> FetchMode {
    if trading_days < ticker_requests {
        FetchMode::GroupedDaily
    } else {
        FetchMode::TickerRange
    }
}

/// The first trading day of a window `years` long that ends on `end`.
pub fn window_start(end: NaiveDate, years: u32) -> NaiveDate {
    let year = end.year() - i32::try_from(years).unwrap_or(i32::MAX);
    // 29 February has no anniversary in a common year: use 28 February.
    let same_day = NaiveDate::from_ymd_opt(year, end.month(), end.day())
        .or_else(|| NaiveDate::from_ymd_opt(year, end.month(), end.day() - 1))
        .unwrap_or(NaiveDate::MIN);
    first_trading_day_from(same_day.succ_opt().unwrap_or(same_day))
}

fn first_trading_day_from(mut d: NaiveDate) -> NaiveDate {
    while !calendar::is_trading_day(d) {
        d = d.succ_opt().expect("date in range");
    }
    d
}

/// Observations per commit: `--batch-rows`, at most (and by default) `ICEBERG_MAX_SCAN_ROWS`,
/// the most one Iceberg append accepts.
fn effective_batch_rows(requested: Option<usize>) -> anyhow::Result<usize> {
    let limit = econ_graph_timeseries::query::QueryLimits::from_env()?.max_rows;
    Ok(requested.unwrap_or(limit).clamp(1, limit))
}

/// What `crawler massive-backfill` asks for. A rerun resumes the open run, whose window and mode
/// were fixed when it started: an explicit `end` or `mode` must match them.
#[derive(Debug, Clone)]
pub struct BackfillOptions {
    /// History depth in years.
    pub years: u32,
    /// Last day to load. Default: see the [module docs](self).
    pub end: Option<NaiveDate>,
    /// Request mode. Default: [`choose_mode`].
    pub mode: Option<FetchMode>,
    /// Most observations per Iceberg commit. Default and maximum: `ICEBERG_MAX_SCAN_ROWS`.
    pub batch_rows: Option<usize>,
    /// Today, for the default end.
    pub today: NaiveDate,
}

/// A run's fixed window and mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RunWindow {
    /// First day loaded.
    pub start: NaiveDate,
    /// Last day loaded.
    pub end: NaiveDate,
    /// Request mode.
    pub mode: FetchMode,
}

/// What a run did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackfillReport {
    /// `massive_backfill_runs.id`.
    pub run_id: Uuid,
    /// The run's window and mode.
    pub window: RunWindow,
    /// Whether this invocation resumed an open run.
    pub resumed: bool,
    /// Units in the run.
    pub units_total: usize,
    /// Units already done when this invocation started.
    pub units_skipped: usize,
    /// Units this invocation finished.
    pub units_done: usize,
    /// Observations this invocation committed.
    pub rows_written: usize,
    /// Iceberg commits this invocation made.
    pub commits: usize,
    /// Requests this invocation sent.
    pub requests: usize,
}

impl BackfillReport {
    /// Text rendering for the CLI.
    pub fn to_text(&self) -> String {
        format!(
            "run {} ({}): {} to {}, {}\nunits: {} total, {} already done, {} done now\n\
             rows written: {}, commits: {}, requests: {}\n",
            self.run_id,
            if self.resumed { "resumed" } else { "new" },
            self.window.start,
            self.window.end,
            self.window.mode.as_str(),
            self.units_total,
            self.units_skipped,
            self.units_done,
            self.rows_written,
            self.commits,
            self.requests,
        )
    }
}

/// The row of one share class (`rows`, oldest first) that owns `day`: the row active on it, or
/// for a day before the first row, the first row. The symbol table lists today's constituents
/// from the day it was made, so their history is fetched under their first known ticker. A
/// share class whose last row has ended owns no later day.
fn row_on<'a>(rows: &[&'a SymbolRow], day: NaiveDate) -> Option<&'a SymbolRow> {
    let first = rows.first()?;
    if day < first.valid_from {
        return Some(first);
    }
    rows.iter().copied().find(|r| r.is_active_on(day))
}

/// The symbols that own each trading day of a window, indexed for the fetch ([`row_on`]).
struct Membership<'a> {
    /// `(date, ticker)` to the row active for that ticker on that date.
    by_day_ticker: HashMap<(NaiveDate, &'a str), &'a SymbolRow>,
    /// Share class to its ticker segments `(ticker, first day, last day)` in the window.
    segments: BTreeMap<&'a str, Vec<(&'a str, NaiveDate, NaiveDate)>>,
    /// Share class to its newest row in the window (whose ticker its series carry).
    latest_row: BTreeMap<&'a str, &'a SymbolRow>,
    /// Trading days in the window.
    days: Vec<NaiveDate>,
}

impl<'a> Membership<'a> {
    fn new(symbols: &'a SymbolTable, start: NaiveDate, end: NaiveDate) -> Self {
        let days = calendar::trading_days(start, end);
        let mut by_day_ticker = HashMap::new();
        let mut spans: BTreeMap<(&str, &str), (NaiveDate, NaiveDate)> = BTreeMap::new();
        let mut latest_row: BTreeMap<&str, &SymbolRow> = BTreeMap::new();
        // Each share class's rows, oldest first.
        let mut by_figi: BTreeMap<&str, Vec<&SymbolRow>> = BTreeMap::new();
        for row in symbols.rows() {
            by_figi
                .entry(row.share_class_figi.as_str())
                .or_default()
                .push(row);
        }
        for rows in by_figi.values_mut() {
            rows.sort_by_key(|r| r.valid_from);
        }
        for &day in &days {
            for row in by_figi.values().filter_map(|rows| row_on(rows, day)) {
                by_day_ticker.insert((day, row.ticker.as_str()), row);
                spans
                    .entry((row.share_class_figi.as_str(), row.ticker.as_str()))
                    .and_modify(|span| span.1 = day)
                    .or_insert((day, day));
                latest_row.insert(row.share_class_figi.as_str(), row);
            }
        }
        let mut segments: BTreeMap<&str, Vec<_>> = BTreeMap::new();
        for ((figi, ticker), (first, last)) in spans {
            segments
                .entry(figi)
                .or_default()
                .push((ticker, first, last));
        }
        Membership {
            by_day_ticker,
            segments,
            latest_row,
            days,
        }
    }

    fn ticker_requests(&self) -> usize {
        self.segments.values().map(Vec::len).sum()
    }

    fn units(&self, mode: FetchMode) -> Vec<String> {
        match mode {
            FetchMode::TickerRange => self.segments.keys().map(|f| figi_unit(f)).collect(),
            FetchMode::GroupedDaily => self.days.iter().map(|d| date_unit(*d)).collect(),
        }
    }

    /// The row a bar belongs to, if its ticker was a listed symbol on its date.
    fn row_for(&self, bar: &DailyBar) -> Option<&'a SymbolRow> {
        self.by_day_ticker
            .get(&(bar.date, bar.ticker.as_str()))
            .copied()
    }
}

/// Ticker segments in a window: one per share class, plus one per ticker change. Per-ticker
/// mode makes one request for each.
pub fn ticker_segments(symbols: &SymbolTable, start: NaiveDate, end: NaiveDate) -> usize {
    Membership::new(symbols, start, end).ticker_requests()
}

fn figi_unit(figi: &str) -> String {
    format!("figi:{figi}")
}

fn date_unit(date: NaiveDate) -> String {
    format!("date:{date}")
}

/// Days still to load: in grouped mode the pending dates; per ticker, the window's trading days
/// times the share of tickers still pending, rounded up.
fn days_remaining(mode: FetchMode, trading_days: usize, units: usize, pending: usize) -> i64 {
    let days = match mode {
        FetchMode::GroupedDaily => pending,
        FetchMode::TickerRange if units == 0 => 0,
        FetchMode::TickerRange => (trading_days * pending).div_ceil(units),
    };
    i64::try_from(days).unwrap_or(i64::MAX)
}

/// One fetched unit, waiting for its batch's commit.
struct FetchedUnit<'a> {
    unit: String,
    rows: Vec<Observation>,
    symbols: Vec<&'a SymbolRow>,
}

/// Units fetched since the last commit.
#[derive(Default)]
struct Batch<'a> {
    units: Vec<FetchedUnit<'a>>,
    rows: usize,
}

/// Runs (or resumes) the backfill. See the [module docs](self).
pub async fn run(
    pool: &DatabasePool,
    catalog: &SqlCatalog,
    client: &dyn DailyBars,
    symbols: &SymbolTable,
    options: &BackfillOptions,
) -> anyhow::Result<BackfillReport> {
    let result = run_inner(pool, catalog, client, symbols, options).await;
    CRAWLER_SOURCE_HEALTH_METRICS.record_backfill_run(SourceId::Massive.as_str(), result.is_ok());
    result
}

async fn run_inner(
    pool: &DatabasePool,
    catalog: &SqlCatalog,
    client: &dyn DailyBars,
    symbols: &SymbolTable,
    options: &BackfillOptions,
) -> anyhow::Result<BackfillReport> {
    let storage = ObservationStorage::from_env()?;
    if storage.backend != ObservationBackend::Iceberg {
        bail!("the Massive backfill writes only to Iceberg; TIMESERIES_BACKEND must be iceberg");
    }
    if options.years == 0 {
        bail!("--years must be at least 1");
    }
    let batch_rows = effective_batch_rows(options.batch_rows)?;

    // Held for the whole run: a second backfill refuses to start rather than racing this one.
    let mut lock = pool
        .get()
        .await
        .context("connecting for the backfill lock")?;
    #[derive(QueryableByName)]
    struct Locked {
        #[diesel(sql_type = diesel::sql_types::Bool)]
        locked: bool,
    }
    let locked = diesel::sql_query("SELECT pg_try_advisory_lock($1) AS locked")
        .bind::<diesel::sql_types::BigInt, _>(RUN_LOCK_KEY)
        .get_result::<Locked>(&mut lock)
        .await?;
    if !locked.locked {
        bail!("another Massive backfill is running");
    }
    let result = run_locked(
        pool, catalog, client, symbols, options, batch_rows, &storage,
    )
    .await;
    // Unlock explicitly: pooled connections outlive this run.
    diesel::sql_query("SELECT pg_advisory_unlock($1)")
        .bind::<diesel::sql_types::BigInt, _>(RUN_LOCK_KEY)
        .execute(&mut lock)
        .await?;
    result
}

#[allow(clippy::too_many_lines)]
async fn run_locked(
    pool: &DatabasePool,
    catalog: &SqlCatalog,
    client: &dyn DailyBars,
    symbols: &SymbolTable,
    options: &BackfillOptions,
    batch_rows: usize,
    storage: &ObservationStorage,
) -> anyhow::Result<BackfillReport> {
    let source = SourceId::Massive.as_str();
    let mut requests = 0usize;
    let (run_id, window, resumed) = match open_run(pool).await? {
        Some((run_id, window)) => {
            check_resume(&window, options)?;
            (run_id, window, true)
        }
        None => {
            let mut end = match options.end {
                Some(end) => end,
                None => default_end(pool, options.today).await?,
            };
            let requested_start = window_start(end, options.years);
            // Finished runs' days are loaded: only older history is left to load.
            if let Some(loaded) = loaded_from(pool).await? {
                end = end.min(calendar::previous_trading_day(loaded));
            }
            if requested_start > end {
                tracing::info!(%requested_start, %end, "Massive backfill has nothing left to load");
                CRAWLER_SOURCE_HEALTH_METRICS.set_backfill_days_remaining(source, 0);
                return Ok(BackfillReport {
                    run_id: Uuid::nil(),
                    window: RunWindow {
                        start: requested_start,
                        end,
                        mode: options.mode.unwrap_or(FetchMode::TickerRange),
                    },
                    resumed: false,
                    units_total: 0,
                    units_skipped: 0,
                    units_done: 0,
                    rows_written: 0,
                    commits: 0,
                    requests: 0,
                });
            }
            let start = plan_start(client, symbols, requested_start, end, &mut requests).await?;
            let membership = Membership::new(symbols, start, end);
            let mode = options.mode.unwrap_or_else(|| {
                choose_mode(membership.ticker_requests(), membership.days.len())
            });
            let window = RunWindow { start, end, mode };
            (create_run(pool, &window).await?, window, false)
        }
    };
    let membership = Membership::new(symbols, window.start, window.end);
    let units = membership.units(window.mode);
    let done = done_units(pool, run_id).await?;
    let pending: Vec<&String> = units.iter().filter(|u| !done.contains(*u)).collect();
    let mut report = BackfillReport {
        run_id,
        window,
        resumed,
        units_total: units.len(),
        units_skipped: units.len() - pending.len(),
        units_done: 0,
        rows_written: 0,
        commits: 0,
        requests,
    };
    let trading_days = membership.days.len();
    let gauge = |pending: usize| {
        CRAWLER_SOURCE_HEALTH_METRICS.set_backfill_days_remaining(
            source,
            days_remaining(window.mode, trading_days, units.len(), pending),
        );
    };
    let mut left = pending.len();
    gauge(left);
    tracing::info!(%run_id, start = %window.start, end = %window.end, mode = window.mode.as_str(),
        resumed, units = units.len(), pending = left, "Massive backfill starting");

    let recorded_at = Utc::now();
    let mut batch = Batch::default();
    for unit in pending {
        let fetched = fetch_unit(client, &membership, unit, recorded_at, &mut report.requests)
            .await
            .with_context(|| format!("fetching {unit}"))?;
        if fetched.rows.len() > batch_rows {
            bail!(
                "{unit} has {} observations, more than --batch-rows {batch_rows}",
                fetched.rows.len()
            );
        }
        if batch.rows + fetched.rows.len() > batch_rows {
            let finished = std::mem::take(&mut batch);
            left -= finished.units.len();
            commit_batch(pool, catalog, storage, run_id, finished, &mut report).await?;
            gauge(left);
            log_progress(&report, left, trading_days, units.len(), window.mode);
        }
        batch.rows += fetched.rows.len();
        batch.units.push(fetched);
    }
    if !batch.units.is_empty() {
        left -= batch.units.len();
        commit_batch(pool, catalog, storage, run_id, batch, &mut report).await?;
        gauge(left);
        log_progress(&report, left, trading_days, units.len(), window.mode);
    }
    finish_run(pool, run_id).await?;
    tracing::info!(%run_id, rows = report.rows_written, commits = report.commits,
        requests = report.requests, "Massive backfill complete");
    Ok(report)
}

fn log_progress(
    report: &BackfillReport,
    left: usize,
    trading_days: usize,
    units: usize,
    mode: FetchMode,
) {
    tracing::info!(
        run_id = %report.run_id,
        units_done = report.units_skipped + report.units_done,
        units_remaining = left,
        days_remaining = days_remaining(mode, trading_days, units, left),
        rows_written = report.rows_written,
        "Massive backfill progress"
    );
}

/// Fails unless the requested options are compatible with the open run's fixed window.
fn check_resume(window: &RunWindow, options: &BackfillOptions) -> anyhow::Result<()> {
    if options.end.is_some_and(|end| end != window.end) {
        bail!(
            "an unfinished backfill run covers {} to {}; rerun with --end {} (or without --end) \
             to resume it",
            window.start,
            window.end,
            window.end
        );
    }
    if options.mode.is_some_and(|mode| mode != window.mode) {
        bail!(
            "an unfinished backfill run uses mode {}; rerun with that mode to resume it",
            window.mode.as_str()
        );
    }
    // The start may have been clamped to the plan's history, so only a deeper request differs.
    if window_start(window.end, options.years) > window.start {
        bail!(
            "an unfinished backfill run starts {}; --years {} asks for less history. Rerun with \
             the --years it started with to resume it",
            window.start,
            options.years
        );
    }
    Ok(())
}

/// The latest day the backfill may load: the trading day before the daily crawl's first stored
/// day for any Massive series, or before `today` when it has stored nothing.
async fn default_end(pool: &DatabasePool, today: NaiveDate) -> anyhow::Result<NaiveDate> {
    #[derive(QueryableByName)]
    struct First {
        #[diesel(sql_type = Nullable<Date>)]
        first: Option<NaiveDate>,
    }
    let mut conn = pool.get().await?;
    let first = diesel::sql_query(
        "SELECT MIN(s.start_date) AS first FROM economic_series s \
         JOIN data_sources d ON d.id = s.source_id \
         WHERE d.name = $1 AND s.start_date IS NOT NULL",
    )
    .bind::<Text, _>(crate::persist::data_source_template(SourceId::Massive).name)
    .get_result::<First>(&mut conn)
    .await?
    .first;
    let bound = first.map_or(today, |first| first.min(today));
    Ok(calendar::previous_trading_day(bound))
}

/// The oldest start the plan serves, from `requested` on. If the requested start is outside the
/// plan's history, moves forward a month at a time until a request is served, then adds
/// [`PLAN_WINDOW_MARGIN_MONTHS`].
async fn plan_start(
    client: &dyn DailyBars,
    symbols: &SymbolTable,
    requested: NaiveDate,
    end: NaiveDate,
    requests: &mut usize,
) -> anyhow::Result<NaiveDate> {
    // Any ticker: a date outside the plan's history is refused whatever the ticker.
    let probe_ticker = symbols
        .rows()
        .first()
        .map(|row| row.ticker.clone())
        .context("the symbol table is empty")?;
    let mut months = 0u32;
    loop {
        let start = requested
            .checked_add_months(chrono::Months::new(months))
            .context("date overflow")?;
        if start > end {
            bail!("the plan serves no history before {end}");
        }
        let probe_end = (start + chrono::Days::new(7)).min(end);
        *requests += 1;
        match with_rate_limit_retry(|| client.ticker_range(&probe_ticker, start, probe_end)).await {
            Ok(_) if months == 0 => return Ok(requested),
            Ok(_) => {
                let clamped = first_trading_day_from(
                    start
                        .checked_add_months(chrono::Months::new(PLAN_WINDOW_MARGIN_MONTHS))
                        .context("date overflow")?,
                );
                tracing::warn!(%requested, %clamped,
                    "the plan's history is shorter than --years; starting at the oldest date it serves");
                return Ok(clamped.min(end));
            }
            Err(ClientError::OutsidePlanWindow { .. }) => months += 1,
            Err(e) => {
                return Err(anyhow::Error::from(crate::CrawlError::from(e)))
                    .context("probing the plan's history")
            }
        }
    }
}

async fn with_rate_limit_retry<T, F, Fut>(mut call: F) -> Result<T, ClientError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ClientError>>,
{
    let mut attempt = 0;
    loop {
        match call().await {
            Err(ClientError::RateLimited { retry_after }) if attempt < MAX_RATE_LIMIT_RETRIES => {
                attempt += 1;
                let wait = retry_after.unwrap_or(DEFAULT_RATE_LIMIT_WAIT);
                tracing::warn!(
                    attempt,
                    wait_secs = wait.as_secs(),
                    "Massive rate limited; waiting"
                );
                tokio::time::sleep(wait).await;
            }
            other => return other,
        }
    }
}

async fn fetch_unit<'a>(
    client: &dyn DailyBars,
    membership: &Membership<'a>,
    unit: &str,
    recorded_at: chrono::DateTime<Utc>,
    requests: &mut usize,
) -> anyhow::Result<FetchedUnit<'a>> {
    let mut bars = Vec::new();
    if let Some(figi) = unit.strip_prefix("figi:") {
        for &(ticker, first, last) in membership.segments.get(figi).into_iter().flatten() {
            *requests += 1;
            let got = with_rate_limit_retry(|| client.ticker_range(ticker, first, last))
                .await
                .map_err(crate::CrawlError::from)?;
            bars.extend(got.into_iter().filter(|b| {
                membership
                    .row_for(b)
                    .is_some_and(|r| r.share_class_figi == figi)
            }));
        }
    } else if let Some(date) = unit.strip_prefix("date:") {
        let date: NaiveDate = date.parse()?;
        *requests += 1;
        let got = with_rate_limit_retry(|| client.grouped(date))
            .await
            .map_err(crate::CrawlError::from)?;
        bars.extend(got.into_iter().filter(|b| b.date == date));
    } else {
        bail!("unknown backfill unit {unit:?}");
    }

    // One bar per ticker and date (the last, if the vendor repeats one), grouped by symbol row.
    let mut by_row: BTreeMap<(&str, NaiveDate, &str), (&SymbolRow, DailyBar)> = BTreeMap::new();
    for bar in bars {
        if let Some(row) = membership.row_for(&bar) {
            by_row.insert(
                (row.share_class_figi.as_str(), bar.date, row.ticker.as_str()),
                (row, bar),
            );
        }
    }
    let mut grouped: BTreeMap<(&str, &str, NaiveDate), (&SymbolRow, Vec<DailyBar>)> =
        BTreeMap::new();
    for (row, bar) in by_row.into_values() {
        grouped
            .entry((
                row.share_class_figi.as_str(),
                row.ticker.as_str(),
                row.valid_from,
            ))
            .or_insert_with(|| (row, Vec::new()))
            .1
            .push(bar);
    }
    let mut rows = Vec::new();
    let mut seen_figis = BTreeSet::new();
    let mut symbols = Vec::new();
    for (row, bars) in grouped.into_values() {
        // Unadjusted daily bars are final when published: each is its own first release.
        for bar in &bars {
            let obs =
                dataset::observations(row, std::slice::from_ref(bar), bar.date, true, recorded_at)?;
            rows.extend(obs);
        }
        if seen_figis.insert(row.share_class_figi.as_str()) {
            let latest = membership
                .latest_row
                .get(row.share_class_figi.as_str())
                .copied()
                .unwrap_or(row);
            symbols.push(latest);
        }
    }
    Ok(FetchedUnit {
        unit: unit.to_string(),
        rows,
        symbols,
    })
}

/// The commit id of a batch: UUIDv5 of the run and its sorted units.
pub fn commit_id(run_id: Uuid, units: &[&str]) -> Uuid {
    let mut units = units.to_vec();
    units.sort_unstable();
    Uuid::new_v5(
        &COMMIT_ID_NAMESPACE,
        format!("{run_id}\n{}", units.join("\n")).as_bytes(),
    )
}

/// Publishes a batch's rows and records its units in one transaction.
async fn commit_batch(
    pool: &DatabasePool,
    catalog: &SqlCatalog,
    storage: &ObservationStorage,
    run_id: Uuid,
    batch: Batch<'_>,
    report: &mut BackfillReport,
) -> anyhow::Result<()> {
    let units: Vec<&str> = batch.units.iter().map(|u| u.unit.as_str()).collect();
    let commit = commit_id(run_id, &units);
    let mut symbols: BTreeMap<&str, &SymbolRow> = BTreeMap::new();
    let mut rows = Vec::with_capacity(batch.rows);
    for unit in &batch.units {
        for row in &unit.symbols {
            symbols.insert(row.share_class_figi.as_str(), row);
        }
        rows.extend(unit.rows.iter().cloned());
    }
    let mut conn = pool.get().await?;
    let published = conn
        .transaction::<bool, anyhow::Error, _>(async |conn| {
            storage.assert_write_allowed_conn(conn).await?;
            for row in symbols.values() {
                // Only when missing: re-upserting a series the daily crawl already wrote would
                // mirror its stored history to Iceberg again.
                if !series_exist_conn(conn, &row.share_class_figi).await? {
                    dataset::ensure_series_conn(conn, pool, row).await?;
                }
            }
            let published = if rows.is_empty() {
                false
            } else {
                let dataset_id = dataset::dataset_id_conn(conn).await?;
                let prepared = catalog
                    .prepare_append_conn(conn, commit, dataset_id, &rows)
                    .await
                    .context("staging the Iceberg commit")?;
                SqlCatalog::commit_prepared_conn(conn, &prepared)
                    .await
                    .context("publishing the Iceberg commit")?;
                true
            };
            for unit in &batch.units {
                let commit = (published && !unit.rows.is_empty()).then_some(commit);
                record_unit(conn, run_id, unit, commit).await?;
            }
            Ok(published)
        })
        .await?;
    report.units_done += batch.units.len();
    report.rows_written += rows.len();
    report.commits += usize::from(published);
    Ok(())
}

/// Whether all of a share class's series rows exist.
async fn series_exist_conn(conn: &mut AsyncPgConnection, figi: &str) -> anyhow::Result<bool> {
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        n: i64,
    }
    let ids: Vec<Uuid> = PriceField::ALL
        .iter()
        .map(|&field| dataset::series_id(figi, field))
        .collect();
    let count = diesel::sql_query("SELECT COUNT(*) AS n FROM economic_series WHERE id = ANY($1)")
        .bind::<diesel::sql_types::Array<SqlUuid>, _>(&ids)
        .get_result::<Count>(conn)
        .await?;
    Ok(usize::try_from(count.n)? == ids.len())
}

async fn record_unit(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
    unit: &FetchedUnit<'_>,
    commit: Option<Uuid>,
) -> anyhow::Result<()> {
    diesel::sql_query(
        "INSERT INTO massive_backfill_units (run_id, unit, rows_written, commit_id) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind::<SqlUuid, _>(run_id)
    .bind::<Text, _>(&unit.unit)
    .bind::<Integer, _>(i32::try_from(unit.rows.len())?)
    .bind::<Nullable<SqlUuid>, _>(commit)
    .execute(conn)
    .await?;
    Ok(())
}

#[derive(QueryableByName)]
struct RunRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Date)]
    window_start: NaiveDate,
    #[diesel(sql_type = Date)]
    window_end: NaiveDate,
    #[diesel(sql_type = Text)]
    mode: String,
}

async fn open_run(pool: &DatabasePool) -> anyhow::Result<Option<(Uuid, RunWindow)>> {
    use diesel::OptionalExtension as _;
    let mut conn = pool.get().await?;
    let row = diesel::sql_query(
        "SELECT id, window_start, window_end, mode FROM massive_backfill_runs \
         WHERE completed_at IS NULL",
    )
    .get_result::<RunRow>(&mut conn)
    .await
    .optional()?;
    row.map(|r| {
        Ok((
            r.id,
            RunWindow {
                start: r.window_start,
                end: r.window_end,
                mode: FetchMode::parse(&r.mode)?,
            },
        ))
    })
    .transpose()
}

/// The first day finished runs loaded, if any finished.
async fn loaded_from(pool: &DatabasePool) -> anyhow::Result<Option<NaiveDate>> {
    #[derive(QueryableByName)]
    struct First {
        #[diesel(sql_type = Nullable<Date>)]
        first: Option<NaiveDate>,
    }
    let mut conn = pool.get().await?;
    Ok(diesel::sql_query(
        "SELECT MIN(window_start) AS first FROM massive_backfill_runs \
         WHERE completed_at IS NOT NULL",
    )
    .get_result::<First>(&mut conn)
    .await?
    .first)
}

async fn create_run(pool: &DatabasePool, window: &RunWindow) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    let mut conn = pool.get().await?;
    diesel::sql_query(
        "INSERT INTO massive_backfill_runs (id, window_start, window_end, mode) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<Date, _>(window.start)
    .bind::<Date, _>(window.end)
    .bind::<Text, _>(window.mode.as_str())
    .execute(&mut conn)
    .await?;
    Ok(id)
}

async fn done_units(pool: &DatabasePool, run_id: Uuid) -> anyhow::Result<BTreeSet<String>> {
    #[derive(QueryableByName)]
    struct Unit {
        #[diesel(sql_type = Text)]
        unit: String,
    }
    let mut conn = pool.get().await?;
    Ok(
        diesel::sql_query("SELECT unit FROM massive_backfill_units WHERE run_id = $1")
            .bind::<SqlUuid, _>(run_id)
            .load::<Unit>(&mut conn)
            .await?
            .into_iter()
            .map(|u| u.unit)
            .collect(),
    )
}

async fn finish_run(pool: &DatabasePool, run_id: Uuid) -> anyhow::Result<()> {
    let mut conn = pool.get().await?;
    diesel::sql_query("UPDATE massive_backfill_runs SET completed_at = now() WHERE id = $1")
        .bind::<SqlUuid, _>(run_id)
        .execute(&mut conn)
        .await?;
    Ok(())
}

/// Serves `/metrics` (the default registry) on `addr` until the returned task is aborted, so
/// Prometheus can scrape a backfill Job. A minimal HTTP/1.0 responder: one request per
/// connection, no keep-alive.
pub async fn serve_metrics(
    addr: std::net::SocketAddr,
) -> anyhow::Result<(std::net::SocketAddr, tokio::task::JoinHandle<()>)> {
    use econ_graph_metrics::prometheus::{Encoder, TextEncoder};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                continue;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                let n = match tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buf))
                    .await
                {
                    Ok(Ok(n)) => n,
                    _ => return,
                };
                let request = String::from_utf8_lossy(&buf[..n]);
                let (status, body) = if request.starts_with("GET /metrics ") {
                    let mut out = Vec::new();
                    let encoded = TextEncoder::new()
                        .encode(&econ_graph_metrics::DEFAULT_REGISTRY.gather(), &mut out);
                    match encoded {
                        Ok(()) => ("200 OK", out),
                        Err(_) => ("500 Internal Server Error", Vec::new()),
                    }
                } else {
                    ("404 Not Found", Vec::new())
                };
                let head = format!(
                    "HTTP/1.0 {status}\r\ncontent-type: text/plain; version=0.0.4\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(&body).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    Ok((bound, task))
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn per_ticker_wins_unless_the_window_is_shorter_than_the_ticker_list() {
        assert_eq!(choose_mode(503, 1258), FetchMode::TickerRange);
        assert_eq!(choose_mode(503, 503), FetchMode::TickerRange);
        assert_eq!(choose_mode(503, 250), FetchMode::GroupedDaily);
    }

    #[test]
    fn window_start_is_the_first_trading_day_after_the_anniversary() {
        // 2026-11-06 is a Friday; 2021-11-06 a Saturday, so the window opens Monday 8 November.
        assert_eq!(window_start(d("2026-11-06"), 5), d("2021-11-08"));
        // 29 February falls back to 28 February; 2023-03-01 is a Wednesday.
        assert_eq!(window_start(d("2024-02-29"), 1), d("2023-03-01"));
    }

    #[test]
    fn days_remaining_scales_ticker_units_to_days() {
        assert_eq!(days_remaining(FetchMode::GroupedDaily, 252, 252, 10), 10);
        assert_eq!(days_remaining(FetchMode::TickerRange, 252, 10, 5), 126);
        assert_eq!(days_remaining(FetchMode::TickerRange, 252, 10, 1), 26);
        assert_eq!(days_remaining(FetchMode::TickerRange, 252, 0, 0), 0);
    }

    #[test]
    fn commit_ids_depend_on_the_run_and_the_set_of_units() {
        let run = Uuid::from_u128(1);
        let a = commit_id(run, &["figi:B", "figi:A"]);
        assert_eq!(a, commit_id(run, &["figi:A", "figi:B"]));
        assert_ne!(a, commit_id(run, &["figi:A"]));
        assert_ne!(a, commit_id(Uuid::from_u128(2), &["figi:A", "figi:B"]));
    }

    #[test]
    fn commit_id_namespace_is_derived_from_its_url() {
        assert_eq!(
            COMMIT_ID_NAMESPACE,
            Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                b"https://econgraph.com/ns/massive-backfill-commit"
            )
        );
    }

    fn row(figi: &str, ticker: &str, from: &str, to: Option<&str>) -> SymbolRow {
        SymbolRow {
            share_class_figi: figi.into(),
            cik10: "0000000001".into(),
            ticker: ticker.into(),
            exchange: "XNYS".into(),
            currency: "USD".into(),
            name: ticker.into(),
            valid_from: d(from),
            valid_to: to.map(d),
        }
    }

    #[test]
    fn history_before_the_first_row_belongs_to_it_and_departures_end_it() {
        let old = row("BBG000000001", "OLD", "2026-01-05", Some("2026-03-31"));
        let new = row("BBG000000001", "NEW", "2026-04-01", None);
        let rows = [&old, &new];
        assert_eq!(row_on(&rows, d("2020-06-01")).unwrap().ticker, "OLD");
        assert_eq!(row_on(&rows, d("2026-03-31")).unwrap().ticker, "OLD");
        assert_eq!(row_on(&rows, d("2026-04-01")).unwrap().ticker, "NEW");
        let gone = row("BBG000000002", "GONE", "2026-01-05", Some("2026-02-27"));
        assert_eq!(row_on(&[&gone], d("2025-01-02")).unwrap().ticker, "GONE");
        assert!(row_on(&[&gone], d("2026-03-02")).is_none());
        assert!(row_on(&[], d("2026-03-02")).is_none());
    }

    #[test]
    fn segments_split_a_share_class_at_its_ticker_change() {
        let table = SymbolTable::parse(
            "share_class_figi,cik10,ticker,exchange,currency,name,valid_from,valid_to\n\
             BBG000000001,0000000001,OLD,XNYS,USD,Old,2026-01-05,2026-03-31\n\
             BBG000000001,0000000001,NEW,XNYS,USD,New,2026-04-01,\n\
             BBG000000002,0000000002,KEEP,XNYS,USD,Keep,2026-11-09,\n",
        )
        .unwrap();
        let m = Membership::new(&table, d("2025-12-01"), d("2026-04-30"));
        assert_eq!(m.ticker_requests(), 3);
        let old = &m.segments["BBG000000001"];
        assert_eq!(
            old,
            &vec![
                ("NEW", d("2026-04-01"), d("2026-04-30")),
                ("OLD", d("2025-12-01"), d("2026-03-31")),
            ]
        );
        assert_eq!(m.latest_row["BBG000000001"].ticker, "NEW");
        assert_eq!(m.units(FetchMode::TickerRange).len(), 2);
    }

    #[test]
    fn resume_refuses_a_different_window() {
        let window = RunWindow {
            start: d("2021-11-08"),
            end: d("2026-11-06"),
            mode: FetchMode::TickerRange,
        };
        let options = BackfillOptions {
            years: 5,
            end: None,
            mode: None,
            batch_rows: None,
            today: d("2026-11-09"),
        };
        check_resume(&window, &options).unwrap();
        // Deeper is fine: the run's start may be clamped to the plan. Shallower is refused.
        check_resume(
            &window,
            &BackfillOptions {
                years: 10,
                ..options.clone()
            },
        )
        .unwrap();
        check_resume(
            &window,
            &BackfillOptions {
                years: 1,
                ..options.clone()
            },
        )
        .unwrap_err();
        check_resume(
            &window,
            &BackfillOptions {
                end: Some(d("2026-11-05")),
                ..options.clone()
            },
        )
        .unwrap_err();
        check_resume(
            &window,
            &BackfillOptions {
                mode: Some(FetchMode::GroupedDaily),
                ..options.clone()
            },
        )
        .unwrap_err();
    }
}
