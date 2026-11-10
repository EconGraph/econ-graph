// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The Massive [`SourceAdapter`](crate::SourceAdapter) implementation: unadjusted daily bars of
//! the tickers in the symbol table, from Massive, a market data vendor.
//!
//! # Key
//!
//! `MASSIVE_API_KEY` ([`ApiKeys::massive`](crate::adapter::ApiKeys::massive)). Massive is in
//! [`SKIPPED_WITHOUT_KEY`](crate::sources::SKIPPED_WITHOUT_KEY): without the key the worker
//! leaves the source out of its registry, logging the skip once, so nothing is discovered,
//! scheduled or counted in coverage. Called without a key anyway, discovery and fetching fail
//! with `Auth` before any request.
//!
//! # Series
//!
//! Discovery reads the symbol table ([`SymbolTable`], loaded from `MASSIVE_SYMBOLS_PATH`, or
//! `massive/sp500.csv` in the crawler's data directory) and lists six series for every security
//! active on the newest settled trading day: `{share_class_figi}:{field}` for `open`, `high`,
//! `low`, `close`, `volume` and `vwap`. No request is made. Series are keyed by the share-class
//! FIGI, so a renamed ticker keeps its series; which ticker to ask Massive for on each day comes
//! from the table's validity dates.
//!
//! # Refresh
//!
//! The worker's ordinary per-series path: the scheduler makes each series due once per trading
//! day, the worker batches due series ([`SourceAdapter::batch_key`] is the same for every
//! Massive series), and [`SourceAdapter::fetch_batch`] returns their points, which the worker
//! writes to the observation store (Iceberg).
//!
//! The window is from the batch's `since` (or [`FIRST_FETCH_DAYS`] back, for a series with
//! nothing stored), and never later than two trading days before the end, so a late correction
//! of either of the two previous trading days is fetched again and stored as a revision. It ends
//! at the newest trading day whose bars have settled ([`calendar::latest_settled_trading_day`]).
//!
//! Each batch is fetched in whichever of two ways needs fewer requests (see
//! [`plan_requests`]):
//!
//! - **Grouped daily**, one request per trading day in the window, each returning every US
//!   stock's bar for that day. The daily refresh fetches three trading days for every ticker,
//!   so this is three requests against about 500 per-ticker ones. Grouped responses are kept in
//!   memory for [`GROUPED_CACHE_TTL`], so the further batches of the same refresh (the worker
//!   batches at most a couple of thousand series at a time) don't fetch the same days again.
//!   Bars are matched to securities through the symbol rows active on each day, so a ticker
//!   that changed hands in the window is credited to the right FIGI on each side of the change.
//! - **Per ticker**, one request per security (per ticker it traded under in the window), for a
//!   long window over few securities: a first fetch, or a single series fetched on its own.
//!
//! A day whose grouped response is empty is a day the market was closed (a holiday the
//! calendar didn't know): no bars, and not a failure. A security missing from a grouped day the
//! market was open is retried once with a per-ticker request over its missing days.
//!
//! # Failures
//!
//! A series fails as `Absent`, counted as a failed series in coverage, when it still has no bar
//! for the newest trading day of the window on which its security was listed. Missing bars on
//! earlier days are gaps (a halted stock has no bar), not failures. Auth and rate-limit errors,
//! and a failed grouped request, fail the whole batch: they are about the source, not one
//! series. A per-ticker request that fails otherwise fails only that security's series.
//!
//! Every point's `revision_date` is the day it was fetched (UTC), so a corrected bar fetched
//! again becomes a new revision beside the original; persistence writes nothing for a value
//! that hasn't changed. Series, points and metadata are built by [`dataset`](super::dataset).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};

use super::calendar;
use super::client::{ClientError, DailyBar, DailyBars, MassiveClient, DEFAULT_BASE_URL};
use super::dataset::{self as ds, DATASET_CODE};
use super::symbols::{self, SymbolRow, SymbolTable};
use crate::adapter::{BatchFetch, CrawlCtx, DiscoveredSeries, FetchedSeries, SourceAdapter};
use crate::error::CrawlError;
use crate::policy::SourcePolicy;
use crate::source::SourceId;

/// The one batch key: one grouped request serves every Massive series.
const BATCH_KEY: &str = "daily";

/// Most series fetched in one batch: every field of every S&P 500 security, with room for
/// index changes. The worker's own scan cap may make batches smaller; the grouped cache covers
/// the difference.
pub const MAX_BATCH: usize = 3_200;

/// How far back the first fetch of a series with nothing stored goes. Deeper history is the
/// backfill job's.
pub const FIRST_FETCH_DAYS: u64 = 30;

/// Previous trading days every refresh fetches again, for late corrections.
pub const CORRECTION_DAYS: usize = 2;

/// How long a grouped response is reused across the batches of one refresh.
pub const GROUPED_CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// Massive daily bars adapter. See the module docs.
#[derive(Clone)]
pub struct MassiveAdapter {
    base_url: String,
    /// The symbol table file; `None` is [`symbols::default_path`], read on each use.
    symbols_path: Option<PathBuf>,
    /// Fixed "now", for tests; `None` is the clock.
    now: Option<DateTime<Utc>>,
    /// Grouped responses by day, with when they were fetched. Shared by every clone.
    grouped_cache: Arc<Mutex<GroupedCache>>,
}

/// Grouped responses by day, each with when it was fetched.
type GroupedCache = HashMap<NaiveDate, (Instant, Arc<Vec<DailyBar>>)>;

impl std::fmt::Debug for MassiveAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MassiveAdapter")
            .field("base_url", &self.base_url)
            .field("symbols_path", &self.symbols_path)
            .finish()
    }
}

impl Default for MassiveAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

impl MassiveAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            symbols_path: None,
            now: None,
            grouped_cache: Arc::default(),
        }
    }

    /// Reads the symbol table from `path` instead of [`symbols::default_path`].
    pub fn with_symbols_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.symbols_path = Some(path.into());
        self
    }

    /// Uses `now` as the current time, for tests.
    pub fn with_now(mut self, now: DateTime<Utc>) -> Self {
        self.now = Some(now);
        self
    }

    fn symbols_path(&self) -> PathBuf {
        self.symbols_path
            .clone()
            .unwrap_or_else(symbols::default_path)
    }

    fn symbols(&self) -> Result<SymbolTable, CrawlError> {
        let path = self.symbols_path();
        SymbolTable::load(&path).map_err(|e| {
            CrawlError::Permanent(format!("Massive symbol table {}: {e:#}", path.display()))
        })
    }

    fn now(&self) -> DateTime<Utc> {
        self.now.unwrap_or_else(Utc::now)
    }

    /// The newest trading day whose bars have settled.
    fn end_day(&self) -> NaiveDate {
        calendar::latest_settled_trading_day(self.now())
    }

    /// `date`'s grouped bars, from the cache when fetched within [`GROUPED_CACHE_TTL`].
    async fn grouped(
        &self,
        client: &dyn DailyBars,
        date: NaiveDate,
    ) -> Result<Arc<Vec<DailyBar>>, ClientError> {
        let cached = self
            .grouped_cache
            .lock()
            .unwrap()
            .get(&date)
            .filter(|(at, _)| at.elapsed() < GROUPED_CACHE_TTL)
            .map(|(_, bars)| bars.clone());
        if let Some(bars) = cached {
            return Ok(bars);
        }
        let bars = Arc::new(client.grouped(date).await?);
        let mut cache = self.grouped_cache.lock().unwrap();
        cache.retain(|_, (at, _)| at.elapsed() < GROUPED_CACHE_TTL);
        cache.insert(date, (Instant::now(), bars.clone()));
        Ok(bars)
    }

    /// The bars of every requested security over `days`, by FIGI and day. Only `Err` for a
    /// failure of the whole source; a security whose own per-ticker request failed has its error
    /// in the second map.
    async fn fetch_bars(
        &self,
        client: &dyn DailyBars,
        table: &SymbolTable,
        figis: &BTreeSet<&str>,
        days: &[NaiveDate],
    ) -> Result<(Bars, HashMap<String, CrawlError>), CrawlError> {
        let tickers = ticker_runs(table, figis, days);
        let mut bars = Bars::new();
        let mut failed = HashMap::new();
        // Days the market was open: every day with a grouped bar, or, fetching per ticker, every
        // calendar trading day.
        let mut open: BTreeSet<NaiveDate> = BTreeSet::new();
        // Days an earlier batch of this refresh already fetched cost no request.
        let uncached = {
            let cache = self.grouped_cache.lock().unwrap();
            days.iter()
                .filter(|d| {
                    !cache
                        .get(d)
                        .is_some_and(|(at, _)| at.elapsed() < GROUPED_CACHE_TTL)
                })
                .count()
        };
        match plan_requests(uncached, tickers.values().map(Vec::len).sum()) {
            Plan::Grouped => {
                for &day in days {
                    let grouped = self.grouped(client, day).await.map_err(CrawlError::from)?;
                    if grouped.is_empty() {
                        tracing::info!(%day, "Massive: no bars for this day; the market was closed");
                        continue;
                    }
                    open.insert(day);
                    let by_ticker: HashMap<&str, &str> = table
                        .active_on(day)
                        .filter(|r| figis.contains(r.share_class_figi.as_str()))
                        .map(|r| (r.ticker.as_str(), r.share_class_figi.as_str()))
                        .collect();
                    for bar in grouped.iter() {
                        if let Some(figi) = by_ticker.get(bar.ticker.as_str()) {
                            bars.entry((*figi).to_string())
                                .or_default()
                                .insert(day, bar.clone());
                        }
                    }
                }
                // A security missing from an open day: one per-ticker retry over its missing days.
                let mut retry: BTreeMap<&str, Vec<(String, NaiveDate, NaiveDate)>> =
                    BTreeMap::new();
                for (figi, runs) in &tickers {
                    let have = bars.get(*figi);
                    for (ticker, from, to) in runs {
                        let missing: Vec<NaiveDate> = open
                            .range(*from..=*to)
                            .filter(|d| !have.is_some_and(|h| h.contains_key(*d)))
                            .copied()
                            .collect();
                        if let (Some(first), Some(last)) = (missing.first(), missing.last()) {
                            retry
                                .entry(figi)
                                .or_default()
                                .push((ticker.clone(), *first, *last));
                        }
                    }
                }
                for (figi, runs) in retry {
                    tracing::info!(
                        figi,
                        "Massive: missing from a grouped day; retrying per ticker"
                    );
                    self.fetch_runs(client, figi, &runs, &mut bars, &mut failed)
                        .await?;
                }
            }
            Plan::PerTicker => {
                open.extend(days.iter().copied());
                for (figi, runs) in &tickers {
                    self.fetch_runs(client, figi, runs, &mut bars, &mut failed)
                        .await?;
                }
            }
        }
        // A security's newest open day on which it was listed must have a bar.
        for (figi, runs) in &tickers {
            if failed.contains_key(*figi) {
                continue;
            }
            let newest = runs
                .iter()
                .filter_map(|(_, from, to)| open.range(*from..=*to).next_back())
                .max();
            if let Some(newest) = newest
                && !bars.get(*figi).is_some_and(|b| b.contains_key(newest))
            {
                failed.insert(
                    (*figi).to_string(),
                    CrawlError::Absent(format!(
                        "Massive has no bar for {figi} on {newest}, a trading day"
                    )),
                );
            }
        }
        Ok((bars, failed))
    }

    /// Per-ticker requests for `figi`'s `runs`, adding the bars to `bars`. Auth and rate-limit
    /// errors are the whole source's, and returned; others fail only `figi`.
    async fn fetch_runs(
        &self,
        client: &dyn DailyBars,
        figi: &str,
        runs: &[(String, NaiveDate, NaiveDate)],
        bars: &mut Bars,
        failed: &mut HashMap<String, CrawlError>,
    ) -> Result<(), CrawlError> {
        for (ticker, from, to) in runs {
            match client.ticker_range(ticker, *from, *to).await {
                Ok(got) => {
                    let entry = bars.entry(figi.to_string()).or_default();
                    // Only the run's own days: the ticker may name another security outside it.
                    for bar in got.into_iter().filter(|b| (*from..=*to).contains(&b.date)) {
                        entry.insert(bar.date, bar);
                    }
                }
                Err(
                    e @ (ClientError::AuthRejected(_)
                    | ClientError::RateLimited { .. }
                    | ClientError::MissingKey),
                ) => return Err(e.into()),
                Err(e) => {
                    failed.insert(figi.to_string(), e.into());
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

/// Bars by FIGI, then by day.
type Bars = HashMap<String, BTreeMap<NaiveDate, DailyBar>>;

/// How a batch's bars are fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// One grouped-daily request per trading day.
    Grouped,
    /// One per-ticker range request per security and ticker.
    PerTicker,
}

/// Which way needs fewer requests: `days` grouped requests (the window's days not already
/// cached), or `ticker_requests` per-ticker ones. Grouped wins a tie: its responses are cached
/// for the next batch.
///
/// A daily refresh asks for three trading days (the newest and [`CORRECTION_DAYS`] before it)
/// for every listed security, so grouped is three requests against about 500. A first fetch of
/// [`FIRST_FETCH_DAYS`] for a few new securities, or a lone series, goes per ticker.
pub fn plan_requests(days: usize, ticker_requests: usize) -> Plan {
    if days <= ticker_requests {
        Plan::Grouped
    } else {
        Plan::PerTicker
    }
}

/// For each of `figis`, the tickers it traded under over `days`, as `(ticker, first, last day)`
/// runs in date order. A FIGI listed on none of the days has no runs.
fn ticker_runs<'a>(
    table: &SymbolTable,
    figis: &BTreeSet<&'a str>,
    days: &[NaiveDate],
) -> BTreeMap<&'a str, Vec<(String, NaiveDate, NaiveDate)>> {
    let mut runs: BTreeMap<&str, Vec<(String, NaiveDate, NaiveDate)>> = BTreeMap::new();
    for &day in days {
        for row in table.active_on(day) {
            let Some(figi) = figis.get(row.share_class_figi.as_str()) else {
                continue;
            };
            let entry = runs.entry(figi).or_default();
            match entry.last_mut() {
                Some((ticker, _, last)) if *ticker == row.ticker => *last = day,
                _ => entry.push((row.ticker.clone(), day, day)),
            }
        }
    }
    runs
}

/// The newest row of `figi` active on any of `days` (newest first), else on `fallback`.
fn row_for<'t>(
    table: &'t SymbolTable,
    figi: &str,
    days: &[NaiveDate],
    fallback: NaiveDate,
) -> Option<&'t SymbolRow> {
    days.iter()
        .rev()
        .chain(std::iter::once(&fallback))
        .find_map(|d| table.active_on(*d).find(|r| r.share_class_figi == figi))
}

/// `ctx`'s Massive client, or `Auth` naming `MASSIVE_API_KEY`.
fn client(ctx: &CrawlCtx, base_url: &str) -> Result<MassiveClient, CrawlError> {
    Ok(MassiveClient::from_ctx(ctx, base_url)?)
}

#[async_trait]
impl SourceAdapter for MassiveAdapter {
    fn id(&self) -> SourceId {
        SourceId::Massive
    }

    fn policy(&self) -> SourcePolicy {
        SourcePolicy {
            max_batch: MAX_BATCH,
            ..SourcePolicy::default_for(SourceId::Massive)
        }
    }

    fn datasets(&self) -> &[&str] {
        &[DATASET_CODE]
    }

    /// Six series per security active on the newest settled trading day; no request.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        client(ctx, &self.base_url)?;
        let table = self.symbols()?;
        let day = self.end_day();
        let found: Vec<DiscoveredSeries> = table
            .active_on(day)
            .flat_map(ds::discovered_series)
            .collect();
        tracing::info!(series = found.len(), %day, "Massive discovery finished");
        Ok(found)
    }

    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let mut out = self
            .fetch_batch(ctx, &[external_id.to_string()], since)
            .await?;
        out.remove(external_id).unwrap_or_else(|| {
            Err(CrawlError::NotFound(format!(
                "Massive {external_id}: not in the symbol table"
            )))
        })
    }

    fn batch_key(&self, external_id: &str) -> Option<String> {
        ds::parse_external_id(external_id).map(|_| BATCH_KEY.to_string())
    }

    /// Fetches every requested series' bars in one go, grouped or per ticker; see the module
    /// docs.
    async fn fetch_batch(
        &self,
        ctx: &CrawlCtx,
        external_ids: &[String],
        since: Option<NaiveDate>,
    ) -> Result<BatchFetch, CrawlError> {
        let client = client(ctx, &self.base_url)?;
        let table = self.symbols()?;
        let end = self.end_day();
        let mut correction_start = end;
        for _ in 0..CORRECTION_DAYS {
            correction_start = calendar::previous_trading_day(correction_start);
        }
        let start = since
            .unwrap_or_else(|| end - chrono::Days::new(FIRST_FETCH_DAYS))
            .min(correction_start);
        let days = calendar::trading_days(start, end);

        let mut out = BatchFetch::with_capacity(external_ids.len());
        let mut wanted: Vec<(&str, &str)> = Vec::new();
        for id in external_ids {
            match ds::parse_external_id(id) {
                Some((figi, _)) => wanted.push((id, figi)),
                None => {
                    out.insert(
                        id.clone(),
                        Err(CrawlError::NotFound(format!(
                            "Massive {id}: not a {{share_class_figi}}:{{field}} id"
                        ))),
                    );
                }
            }
        }
        let figis: BTreeSet<&str> = wanted.iter().map(|(_, figi)| *figi).collect();
        let (bars, failed) = self.fetch_bars(&client, &table, &figis, &days).await?;
        let revision_date = self.now().date_naive();

        // Every field series of every requested security, from its bars.
        let mut series: HashMap<String, Result<FetchedSeries, CrawlError>> = HashMap::new();
        for figi in &figis {
            if let Some(e) = failed.get(*figi) {
                for field in ds::PriceField::ALL {
                    series.insert(ds::external_id(figi, field), Err(e.clone()));
                }
                continue;
            }
            let figi_bars = bars
                .get(*figi)
                .map(|b| b.values().cloned().collect::<Vec<_>>());
            match security_series(
                &table,
                figi,
                figi_bars.unwrap_or_default(),
                &days,
                end,
                revision_date,
            ) {
                Ok(Some(fetched)) => series.extend(fetched.into_iter().map(|(id, f)| (id, Ok(f)))),
                Ok(None) => {}
                Err(e) => {
                    for field in ds::PriceField::ALL {
                        series.insert(ds::external_id(figi, field), Err(e.clone()));
                    }
                }
            }
        }
        for (id, figi) in wanted {
            let fetched = series.remove(id).unwrap_or_else(|| {
                Err(CrawlError::NotFound(format!(
                    "Massive {id}: {figi} is not in the symbol table for {start}..{end}"
                )))
            });
            out.insert(id.to_string(), fetched);
        }
        Ok(out)
    }
}

/// The six series of `figi` from `bars` (in date order), each bar credited to the symbol row it
/// traded under, and the series' metadata and dataset from the newest row. `None` when the
/// symbol table doesn't list `figi` on any of `days` or on `end`.
fn security_series(
    table: &SymbolTable,
    figi: &str,
    bars: Vec<DailyBar>,
    days: &[NaiveDate],
    end: NaiveDate,
    revision_date: NaiveDate,
) -> Result<Option<Vec<(String, FetchedSeries)>>, CrawlError> {
    // Consecutive bars under the same row, oldest first.
    let mut groups: Vec<(&SymbolRow, Vec<DailyBar>)> = Vec::new();
    for bar in bars {
        let row = table
            .active_on(bar.date)
            .find(|r| r.share_class_figi == figi && r.ticker == bar.ticker)
            .ok_or_else(|| {
                CrawlError::Parse(format!(
                    "Massive: {figi} did not trade as {} on {}",
                    bar.ticker, bar.date
                ))
            })?;
        match groups.last_mut() {
            Some((r, group)) if std::ptr::eq(*r, row) => group.push(bar),
            _ => groups.push((row, vec![bar])),
        }
    }
    if groups.is_empty() {
        let Some(row) = row_for(table, figi, days, end) else {
            return Ok(None);
        };
        groups.push((row, Vec::new()));
    }
    let mut merged: BTreeMap<String, FetchedSeries> = BTreeMap::new();
    for (row, group) in groups {
        for (id, fetched) in ds::fetched_series(row, &group, revision_date)? {
            match merged.get_mut(&id) {
                // A later row: its metadata and dimensions are the current ones.
                Some(prev) => {
                    prev.points.extend(fetched.points);
                    prev.metadata = fetched.metadata;
                    prev.dataset = fetched.dataset;
                }
                None => {
                    merged.insert(id, fetched);
                }
            }
        }
    }
    Ok(Some(merged.into_iter().collect()))
}

#[cfg(test)]
mod tests;
