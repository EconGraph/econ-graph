// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The queue worker: the one consumer of `crawl_queue`.
//!
//! A [`Worker`] runs `concurrency` claim loops. Each loop claims the next due item with
//! [`CrawlQueueItem::claim_next`], dispatches it, persists the result through [`crate::persist`]
//! and moves the item to its next state:
//!
//! | job result                          | queue transition                                                        |
//! |-------------------------------------|-------------------------------------------------------------------------|
//! | `Ok`                                | `complete`                                                              |
//! | `RateLimited { retry_after }`       | `retry_later(retry_after or policy.backoff(retry_count), count = false)` |
//! | `Transient` (incl. panics, DB errors while persisting) | `retry_later(policy.backoff(retry_count), count = true)` (fails once `max_retries` is reached) |
//! | `NotFound` / `Auth` / `Parse` / `Permanent` | `fail`                                                          |
//! | unknown `source` / `kind`, no adapter or handler | `fail`                                                 |
//!
//! A claim is a lease. `complete` / `retry_later` / `fail` are passed the worker id and only apply
//! while the item is still `processing` and locked by this worker. If the job outlived
//! `stuck_after`, the maintenance loop (of any worker) has released the item and another worker
//! may be running it; the transition then reports a lost lease, the worker logs a warning and
//! returns [`JobOutcome::LeaseLost`] without touching the item again.
//!
//! Dispatch by `kind`: a [`JobHandler`] registered for `(source, kind)` wins; otherwise
//! `fetch_series` calls [`SourceAdapter::fetch_series`](crate::SourceAdapter::fetch_series) with
//! `since` = the latest stored observation date minus the source's
//! [`revision_lookback`](crate::SourcePolicy::revision_lookback) (so recent revisions are
//! re-fetched; `None` for a series with no stored points) and `discover_catalog` calls
//! [`SourceAdapter::discover`](crate::SourceAdapter::discover). Adapter and handler calls run in
//! their own task, so a panic becomes a `Transient` error instead of killing the worker.
//!
//! Per-source circuit breaker: after `pause_after_consecutive` consecutive `RateLimited` or `Auth`
//! results for a source, the worker stops claiming that source for `pause_for` (via
//! `claim_next`'s source filter). While any source is paused and no `source_filter` is set, the
//! filter lists every other known [`SourceId`], so rows with unrecognised `source` strings wait
//! until the pause ends.
//!
//! A maintenance loop calls [`CrawlQueueItem::release_stuck`] every `stuck_after / 2`
//! (clamped to 1 s ..= 5 min) so items held by crashed workers become claimable again (each
//! release counts as an attempt). The same loop purges `completed` / `failed` rows older than
//! `queue_retention` with [`CrawlQueueItem::purge_finished`], at start-up and then hourly.
//!
//! [`Worker::run`] stops claiming when its `shutdown` future resolves and returns once in-flight
//! jobs have finished.

use std::collections::HashMap;
use std::future::Future;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use econ_graph_core::models::{CrawlQueueItem, JobKind, LeaseOutcome, QueueTransition};
use tokio::sync::watch;
use uuid::Uuid;

use crate::adapter::{AdapterRegistry, CrawlCtx};
use crate::error::CrawlError;
use crate::persist::{self, AttemptRecord};
use crate::source::SourceId;

/// Worker settings.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerConfig {
    /// Stored in `crawl_queue.locked_by`. Must be unique per worker process.
    pub worker_id: String,
    /// Number of concurrent claim loops (jobs in flight). At least 1 is used.
    pub concurrency: usize,
    /// Sleep between claim attempts when the queue has nothing due.
    pub poll_interval: Duration,
    /// `processing` items locked longer than this are returned to `pending`.
    pub stuck_after: Duration,
    /// Only claim these sources (`None` = all).
    pub source_filter: Option<Vec<SourceId>>,
    /// Consecutive `RateLimited`/`Auth` results that pause a source (0 disables the breaker).
    pub pause_after_consecutive: u32,
    /// How long a tripped source stays paused.
    pub pause_for: Duration,
    /// `completed` / `failed` rows older than this (by `finished_at`) are deleted by the
    /// maintenance loop. `None` disables purging.
    pub queue_retention: Option<Duration>,
}

/// Default for [`WorkerConfig::queue_retention`]: 14 days.
pub const DEFAULT_QUEUE_RETENTION: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// How often the maintenance loop purges finished queue rows.
const PURGE_INTERVAL: Duration = Duration::from_secs(60 * 60);

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            worker_id: default_worker_id(),
            concurrency: 4,
            poll_interval: Duration::from_secs(5),
            stuck_after: Duration::from_secs(30 * 60),
            source_filter: None,
            pause_after_consecutive: 5,
            pause_for: Duration::from_secs(5 * 60),
            queue_retention: Some(DEFAULT_QUEUE_RETENTION),
        }
    }
}

/// `<hostname>-<pid>` (hostname from `$HOSTNAME`, then `/etc/hostname`, else `worker`).
pub fn default_worker_id() -> String {
    let host = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "worker".to_string());
    format!("{host}-{}", std::process::id())
}

/// What a successful job wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobStats {
    /// `economic_series` row written (fetch jobs).
    pub series_id: Option<Uuid>,
    /// Data points inserted or updated.
    pub points_written: usize,
    /// Data points that did not exist before.
    pub new_points: usize,
    /// Latest observation date fetched.
    pub latest_date: Option<NaiveDate>,
    /// `series_metadata` rows written (discovery jobs).
    pub metadata_written: usize,
}

/// Result of processing one queue item.
#[derive(Debug, Clone, PartialEq)]
pub enum JobOutcome {
    /// The item is `completed`.
    Completed(JobStats),
    /// The item is `retrying` and becomes due at `at`.
    Retrying {
        /// Why the attempt failed.
        error: CrawlError,
        /// When the item becomes claimable again.
        at: DateTime<Utc>,
    },
    /// The item is `failed` (permanent error, or retry budget exhausted).
    Failed {
        /// The error recorded on the item.
        error: String,
    },
    /// The worker's lease expired before it finished (`release_stuck` re-queued the item, and
    /// another worker may be processing it). The result was discarded and the item left alone.
    LeaseLost {
        /// Result of the discarded attempt: `None` on success, else the error.
        error: Option<String>,
    },
}

/// Extension point for job kinds the built-in dispatch doesn't cover (e.g. SEC `fetch_filing`).
///
/// Register with [`Worker::register_handler`] / [`Worker::with_handler`] for a
/// `(SourceId, JobKind)` pair. A handler does its own persistence. It should return
/// `Ok(JobOutcome::Completed(stats))` on success or `Err(CrawlError)` on failure (the worker
/// applies the usual transition table). For convenience, `Ok(JobOutcome::Retrying { error, .. })`
/// is treated like `Err(error)` and `Ok(JobOutcome::Failed { error })` like
/// `Err(CrawlError::Permanent(error))`.
#[async_trait]
pub trait JobHandler: Send + Sync {
    /// Process `item` (already claimed by the worker).
    async fn handle(&self, ctx: &CrawlCtx, item: &CrawlQueueItem)
        -> Result<JobOutcome, CrawlError>;
}

#[derive(Debug, Default)]
struct Breaker {
    consecutive: HashMap<SourceId, u32>,
    paused_until: HashMap<SourceId, Instant>,
}

impl Breaker {
    /// Records a job result; returns true if this result tripped the breaker.
    fn record(
        &mut self,
        source: SourceId,
        error: Option<&CrawlError>,
        threshold: u32,
        pause_for: Duration,
    ) -> bool {
        let trips = matches!(
            error,
            Some(CrawlError::RateLimited { .. } | CrawlError::Auth(_))
        );
        if !trips || threshold == 0 {
            self.consecutive.remove(&source);
            return false;
        }
        let n = self.consecutive.entry(source).or_insert(0);
        *n += 1;
        if *n >= threshold {
            self.consecutive.remove(&source);
            self.paused_until.insert(source, Instant::now() + pause_for);
            true
        } else {
            false
        }
    }

    /// Currently paused sources (expired pauses are dropped).
    fn paused(&mut self, now: Instant) -> Vec<SourceId> {
        self.paused_until.retain(|_, until| *until > now);
        let mut v: Vec<_> = self.paused_until.keys().copied().collect();
        v.sort_unstable();
        v
    }
}

/// The crawl queue worker. See the [module docs](self).
pub struct Worker {
    /// Shared HTTP client, pool and keys.
    pub ctx: CrawlCtx,
    /// Source adapters for `fetch_series` / `discover_catalog`.
    pub registry: AdapterRegistry,
    /// Settings.
    pub config: WorkerConfig,
    handlers: HashMap<(SourceId, JobKind), Arc<dyn JobHandler>>,
    breaker: Mutex<Breaker>,
}

type Dispatched = Result<JobStats, CrawlError>;

impl Worker {
    /// A worker with no extension handlers.
    pub fn new(ctx: CrawlCtx, registry: AdapterRegistry, config: WorkerConfig) -> Self {
        Self {
            ctx,
            registry,
            config,
            handlers: HashMap::new(),
            breaker: Mutex::new(Breaker::default()),
        }
    }

    /// Registers `handler` for `(source, kind)`, replacing any previous one. Handlers take
    /// precedence over the built-in `fetch_series` / `discover_catalog` dispatch.
    pub fn register_handler(
        &mut self,
        source: SourceId,
        kind: JobKind,
        handler: Arc<dyn JobHandler>,
    ) -> Option<Arc<dyn JobHandler>> {
        self.handlers.insert((source, kind), handler)
    }

    /// Builder form of [`register_handler`](Self::register_handler).
    #[must_use]
    pub fn with_handler(
        mut self,
        source: SourceId,
        kind: JobKind,
        handler: Arc<dyn JobHandler>,
    ) -> Self {
        self.register_handler(source, kind, handler);
        self
    }

    /// Sources the circuit breaker currently has paused.
    pub fn paused_sources(&self) -> Vec<SourceId> {
        self.breaker_lock().paused(Instant::now())
    }

    fn breaker_lock(&self) -> std::sync::MutexGuard<'_, Breaker> {
        // The breaker holds plain counters; a poisoned lock is still usable.
        self.breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Runs until `shutdown` resolves, then stops claiming and waits for in-flight jobs.
    pub async fn run<F: Future>(&self, shutdown: F) {
        let concurrency = self.config.concurrency.max(1);
        tracing::info!(
            worker_id = %self.config.worker_id,
            concurrency,
            sources = ?self.config.source_filter,
            adapters = ?self.registry.ids(),
            "crawl worker starting"
        );
        let (stop_tx, stop_rx) = watch::channel(false);
        let loops =
            futures::future::join_all((0..concurrency).map(|_| self.claim_loop(stop_rx.clone())));
        let work = futures::future::join(loops, self.maintenance_loop(stop_rx));
        tokio::pin!(work);
        tokio::pin!(shutdown);
        tokio::select! {
            _ = &mut work => {}
            _ = &mut shutdown => {
                tracing::info!(worker_id = %self.config.worker_id, "shutdown requested; finishing in-flight jobs");
                let _ = stop_tx.send(true);
                work.await;
            }
        }
        tracing::info!(worker_id = %self.config.worker_id, "crawl worker stopped");
    }

    async fn claim_loop(&self, mut stop: watch::Receiver<bool>) {
        loop {
            if *stop.borrow() {
                return;
            }
            if self.run_once().await.is_some() {
                continue;
            }
            tokio::select! {
                _ = tokio::time::sleep(self.config.poll_interval) => {}
                changed = stop.changed() => if changed.is_err() { return; },
            }
        }
    }

    async fn maintenance_loop(&self, mut stop: watch::Receiver<bool>) {
        let period = (self.config.stuck_after / 2)
            .clamp(Duration::from_secs(1), Duration::from_secs(5 * 60));
        let mut last_purge: Option<Instant> = None;
        loop {
            if *stop.borrow() {
                return;
            }
            match CrawlQueueItem::release_stuck(&self.ctx.pool, self.config.stuck_after).await {
                Ok(0) => {}
                Ok(n) => tracing::warn!(released = n, "released stuck crawl_queue items"),
                Err(e) => tracing::warn!(error = %e, "release_stuck failed"),
            }
            if let Some(retention) = self.config.queue_retention {
                if last_purge.is_none_or(|t| t.elapsed() >= PURGE_INTERVAL) {
                    last_purge = Some(Instant::now());
                    match CrawlQueueItem::purge_finished(&self.ctx.pool, retention).await {
                        Ok(0) => {}
                        Ok(n) => tracing::info!(
                            deleted = n,
                            retention_days = retention.as_secs() / 86_400,
                            "purged finished crawl_queue items"
                        ),
                        Err(e) => tracing::warn!(error = %e, "purging crawl_queue failed"),
                    }
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(period) => {}
                changed = stop.changed() => if changed.is_err() { return; },
            }
        }
    }

    /// The `claim_next` source filter: `None` = nothing claimable right now,
    /// `Some(None)` = any source, `Some(Some(list))` = only these.
    fn claim_filter(&self) -> Option<Option<Vec<String>>> {
        let paused = self.paused_sources();
        let allowed = |s: &SourceId| !paused.contains(s);
        match &self.config.source_filter {
            None if paused.is_empty() => Some(None),
            None => Some(Some(
                SourceId::ALL
                    .iter()
                    .filter(|s| allowed(s))
                    .map(|s| s.as_str().to_string())
                    .collect(),
            )),
            Some(list) => {
                let v: Vec<String> = list
                    .iter()
                    .filter(|s| allowed(s))
                    .map(|s| s.as_str().to_string())
                    .collect();
                (!v.is_empty()).then_some(Some(v))
            }
        }
    }

    /// Claims and processes one item. `None` if nothing was due (or the claim failed).
    pub async fn run_once(&self) -> Option<JobOutcome> {
        let filter = self.claim_filter()?;
        let item = match CrawlQueueItem::claim_next(
            &self.ctx.pool,
            &self.config.worker_id,
            filter.as_deref(),
        )
        .await
        {
            Ok(Some(item)) => item,
            Ok(None) => return None,
            Err(e) => {
                tracing::warn!(worker_id = %self.config.worker_id, error = %e, "claim_next failed");
                return None;
            }
        };
        let (source, kind) = (item.source.clone(), item.kind.clone());
        let outcome = self.process(item).await;
        let label = match &outcome {
            JobOutcome::Completed(_) => "completed",
            JobOutcome::Retrying { .. } => "retrying",
            JobOutcome::Failed { .. } => "failed",
            JobOutcome::LeaseLost { .. } => "lease_lost",
        };
        econ_graph_metrics::crawler::CRAWLER_QUEUE_METRICS.record_job(&source, &kind, label);
        Some(outcome)
    }

    async fn process(&self, item: CrawlQueueItem) -> JobOutcome {
        let started = Instant::now();
        let source = match SourceId::from_str(&item.source) {
            Ok(s) => s,
            Err(_) => {
                return self
                    .fail(&item, format!("unknown source {:?}", item.source))
                    .await
            }
        };
        let kind = match JobKind::from_str(&item.kind) {
            Ok(k) => k,
            Err(_) => {
                return self
                    .fail(&item, format!("unknown job kind {:?}", item.kind))
                    .await
            }
        };
        tracing::debug!(id = %item.id, %source, %kind, series_id = %item.series_id, "processing");

        let result = self.dispatch(source, kind, &item).await;

        let tripped = self.breaker_lock().record(
            source,
            result.as_ref().err(),
            self.config.pause_after_consecutive,
            self.config.pause_for,
        );
        if tripped {
            tracing::warn!(
                %source,
                consecutive = self.config.pause_after_consecutive,
                pause_secs = self.config.pause_for.as_secs(),
                "circuit breaker tripped: pausing source"
            );
        }

        if kind == JobKind::FetchSeries {
            self.record_attempt(source, &item, &result, started.elapsed())
                .await;
        }
        self.transition(source, &item, result, started.elapsed())
            .await
    }

    async fn dispatch(&self, source: SourceId, kind: JobKind, item: &CrawlQueueItem) -> Dispatched {
        if let Some(handler) = self.handlers.get(&(source, kind)).cloned() {
            let ctx = self.ctx.clone();
            let item = item.clone();
            let out = guarded(async move { handler.handle(&ctx, &item).await }).await?;
            return match out {
                JobOutcome::Completed(stats) => Ok(stats),
                JobOutcome::Retrying { error, .. } => Err(error),
                JobOutcome::Failed { error } => Err(CrawlError::Permanent(error)),
                JobOutcome::LeaseLost { .. } => Err(CrawlError::Transient(
                    "handler returned LeaseLost (only the worker reports lost leases)".into(),
                )),
            };
        }
        match kind {
            JobKind::FetchSeries => self.fetch_series(source, &item.series_id).await,
            JobKind::DiscoverCatalog => self.discover(source).await,
            other => Err(CrawlError::Permanent(format!(
                "no handler registered for {source} {other}"
            ))),
        }
    }

    async fn fetch_series(&self, source: SourceId, external_id: &str) -> Dispatched {
        let adapter = self
            .registry
            .get(source)
            .ok_or_else(|| CrawlError::Permanent(format!("no adapter registered for {source}")))?;
        let latest = persist::latest_point_date(&self.ctx.pool, source, external_id)
            .await
            .map_err(db_error)?;
        let since = latest.and_then(|d| incremental_since(d, self.ctx.http.policy(source)));
        let ctx = self.ctx.clone();
        let id = external_id.to_string();
        let fetched = guarded(async move { adapter.fetch_series(&ctx, &id, since).await }).await?;
        let write = persist::persist_series(&self.ctx.pool, source, external_id, &fetched)
            .await
            .map_err(db_error)?;
        Ok(JobStats {
            series_id: Some(write.series_id),
            points_written: write.points_upserted,
            new_points: write.points_new,
            latest_date: write.latest_date,
            metadata_written: 0,
        })
    }

    async fn discover(&self, source: SourceId) -> Dispatched {
        let adapter = self
            .registry
            .get(source)
            .ok_or_else(|| CrawlError::Permanent(format!("no adapter registered for {source}")))?;
        let ctx = self.ctx.clone();
        let found = guarded(async move { adapter.discover(&ctx).await }).await?;
        let written = persist::persist_discovered(&self.ctx.pool, source, &found)
            .await
            .map_err(db_error)?;
        Ok(JobStats {
            metadata_written: written,
            ..JobStats::default()
        })
    }

    async fn record_attempt(
        &self,
        source: SourceId,
        item: &CrawlQueueItem,
        result: &Dispatched,
        duration: Duration,
    ) {
        let series_id = match result {
            Ok(stats) => stats.series_id,
            Err(_) => persist::find_series_id(&self.ctx.pool, source, &item.series_id)
                .await
                .ok()
                .flatten(),
        };
        // crawl_attempts.series_id references economic_series; nothing to attach to yet.
        let Some(series_id) = series_id else { return };
        let record = match result {
            Ok(stats) => AttemptRecord {
                success: true,
                error_kind: None,
                error_message: None,
                points: stats.points_written,
                new_points: stats.new_points,
                latest_date: stats.latest_date,
                duration,
                retry_count: item.retry_count,
            },
            Err(e) => AttemptRecord {
                success: false,
                error_kind: Some(e.kind().to_string()),
                error_message: Some(e.to_string()),
                points: 0,
                new_points: 0,
                latest_date: None,
                duration,
                retry_count: item.retry_count,
            },
        };
        if let Err(e) = persist::record_attempt(&self.ctx.pool, series_id, &record).await {
            tracing::warn!(id = %item.id, error = %e, "recording crawl attempt failed");
        }
    }

    async fn transition(
        &self,
        source: SourceId,
        item: &CrawlQueueItem,
        result: Dispatched,
        elapsed: Duration,
    ) -> JobOutcome {
        let pool = &self.ctx.pool;
        let error = match result {
            Ok(stats) => {
                match CrawlQueueItem::complete(pool, item).await {
                    Ok(LeaseOutcome::Applied) => {}
                    Ok(LeaseOutcome::LostLease) => {
                        self.warn_lost_lease(item, "complete", elapsed);
                        return JobOutcome::LeaseLost { error: None };
                    }
                    Err(e) => {
                        tracing::error!(id = %item.id, error = %e, "marking item completed failed");
                    }
                }
                tracing::info!(
                    id = %item.id,
                    %source,
                    kind = %item.kind,
                    series_id = %item.series_id,
                    points = stats.points_written,
                    new_points = stats.new_points,
                    metadata = stats.metadata_written,
                    elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
                    "job completed"
                );
                return JobOutcome::Completed(stats);
            }
            Err(e) => e,
        };

        let policy = self.ctx.http.policy(source);
        let attempt = u32::try_from(item.retry_count.max(0)).unwrap_or(0);
        let retry = match &error {
            CrawlError::RateLimited { retry_after } => Some((
                retry_after.unwrap_or_else(|| policy.backoff(attempt)),
                false,
            )),
            CrawlError::Transient(_) => Some((policy.backoff(attempt), true)),
            CrawlError::NotFound(_)
            | CrawlError::Auth(_)
            | CrawlError::Parse(_)
            | CrawlError::Permanent(_) => None,
        };
        let message = error.to_string();

        let Some((delay, count_attempt)) = retry else {
            tracing::warn!(id = %item.id, %source, series_id = %item.series_id, kind = error.kind(), error = %message, "job failed permanently");
            return self.fail(item, message).await;
        };
        match CrawlQueueItem::retry_later(pool, item, &message, delay, count_attempt).await {
            Ok(QueueTransition::Rescheduled { at }) => {
                tracing::warn!(
                    id = %item.id,
                    %source,
                    series_id = %item.series_id,
                    kind = error.kind(),
                    error = %message,
                    delay_secs = delay.as_secs(),
                    counted = count_attempt,
                    "job will be retried"
                );
                JobOutcome::Retrying { error, at }
            }
            Ok(QueueTransition::Failed) => {
                tracing::warn!(id = %item.id, %source, series_id = %item.series_id, error = %message, "retry budget exhausted; job failed");
                JobOutcome::Failed { error: message }
            }
            Ok(QueueTransition::LostLease) => {
                self.warn_lost_lease(item, "retry_later", elapsed);
                JobOutcome::LeaseLost {
                    error: Some(message),
                }
            }
            Err(e) => {
                tracing::error!(id = %item.id, error = %e, "rescheduling item failed");
                JobOutcome::Failed {
                    error: format!("{message}; rescheduling failed: {e}"),
                }
            }
        }
    }

    async fn fail(&self, item: &CrawlQueueItem, error: String) -> JobOutcome {
        match CrawlQueueItem::fail(&self.ctx.pool, item, &error).await {
            Ok(LeaseOutcome::Applied) => {}
            Ok(LeaseOutcome::LostLease) => {
                let held_for = item
                    .locked_at
                    .and_then(|t| (Utc::now() - t).to_std().ok())
                    .unwrap_or_default();
                self.warn_lost_lease(item, "fail", held_for);
                return JobOutcome::LeaseLost { error: Some(error) };
            }
            Err(e) => tracing::error!(id = %item.id, error = %e, "marking item failed failed"),
        }
        JobOutcome::Failed { error }
    }

    /// A transition found the item no longer `processing` under this worker's lock.
    fn warn_lost_lease(&self, item: &CrawlQueueItem, transition: &str, elapsed: Duration) {
        tracing::warn!(
            id = %item.id,
            source = %item.source,
            kind = %item.kind,
            series_id = %item.series_id,
            worker_id = %self.config.worker_id,
            transition,
            elapsed_secs = elapsed.as_secs(),
            stuck_after_secs = self.config.stuck_after.as_secs(),
            "lost lease on crawl_queue item (job outlived stuck_after and was released); result discarded"
        );
    }
}

/// `latest - policy.revision_lookback` (whole days), or `None` (full fetch) if that would
/// underflow the calendar.
pub(crate) fn incremental_since(
    latest: NaiveDate,
    policy: crate::policy::SourcePolicy,
) -> Option<NaiveDate> {
    let lookback = chrono::Duration::from_std(policy.revision_lookback).ok()?;
    latest.checked_sub_signed(lookback)
}

/// Database failures while reading/persisting are infrastructure problems: retry later.
fn db_error(e: econ_graph_core::AppError) -> CrawlError {
    CrawlError::Transient(format!("database: {e}"))
}

/// Runs `fut` in its own task so a panic becomes `CrawlError::Transient`.
async fn guarded<T, F>(fut: F) -> Result<T, CrawlError>
where
    T: Send + 'static,
    F: Future<Output = Result<T, CrawlError>> + Send + 'static,
{
    match tokio::spawn(fut).await {
        Ok(r) => r,
        Err(e) if e.is_panic() => {
            let payload = e.into_panic();
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic>".to_string());
            tracing::error!(panic = %msg, "job panicked");
            Err(CrawlError::Transient(format!("job panicked: {msg}")))
        }
        Err(e) => Err(CrawlError::Transient(format!("job task failed: {e}"))),
    }
}

#[cfg(test)]
mod breaker_tests {
    use super::*;

    #[test]
    fn incremental_since_subtracts_lookback() {
        let latest = NaiveDate::from_ymd_opt(2024, 3, 1).unwrap();
        let mut p = crate::policy::SourcePolicy::default_for(SourceId::Fred);
        p.revision_lookback = Duration::ZERO;
        assert_eq!(incremental_since(latest, p), Some(latest));
        p.revision_lookback = Duration::from_secs(30 * 86_400);
        assert_eq!(
            incremental_since(latest, p),
            NaiveDate::from_ymd_opt(2024, 1, 31)
        );
        p.revision_lookback = Duration::from_secs(u64::MAX / 2);
        assert_eq!(incremental_since(latest, p), None);
    }

    #[test]
    fn trips_after_k_consecutive_and_resets_on_other_results() {
        let mut b = Breaker::default();
        let rl = CrawlError::RateLimited { retry_after: None };
        let pause = Duration::from_secs(60);
        assert!(!b.record(SourceId::Fred, Some(&rl), 3, pause));
        assert!(!b.record(SourceId::Fred, Some(&rl), 3, pause));
        // A success resets the streak.
        assert!(!b.record(SourceId::Fred, None, 3, pause));
        assert!(!b.record(SourceId::Fred, Some(&rl), 3, pause));
        assert!(!b.record(
            SourceId::Fred,
            Some(&CrawlError::Auth("x".into())),
            3,
            pause
        ));
        assert!(b.paused(Instant::now()).is_empty());
        assert!(b.record(SourceId::Fred, Some(&rl), 3, pause));
        assert_eq!(b.paused(Instant::now()), vec![SourceId::Fred]);
        assert!(b.paused(Instant::now() + pause * 2).is_empty());
        // Threshold 0 disables.
        assert!(!b.record(SourceId::Bls, Some(&rl), 0, pause));
        // Transient errors don't count.
        let t = CrawlError::Transient("t".into());
        assert!(!b.record(SourceId::Bls, Some(&t), 1, pause));
    }
}

#[cfg(test)]
mod tests;
