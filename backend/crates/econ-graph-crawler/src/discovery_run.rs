// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Chunked catalog discovery: one cheap listing step, then short detail jobs, then one commit.
//!
//! A single `discover_catalog` job lists a whole catalog inside one lease. That doesn't work
//! for a source whose catalog takes one upstream request per entry: at the source's rate limit
//! the walk can outlast the worker's `stuck_after`, and one transient error anywhere fails all
//! of it. An adapter whose [`chunked_discovery`](crate::SourceAdapter::chunked_discovery)
//! returns `Some` is discovered in pieces instead:
//!
//! 1. **Enumerate.** The `discover_catalog` job refreshes reference data as usual, starts a run
//!    (a `discovery_runs` row in state `enumerating`) and calls
//!    [`enumerate_discovery`](crate::SourceAdapter::enumerate_discovery) for the list of
//!    [`DiscoveryUnit`]s. Its result transaction moves the run to `running`, stores the units in
//!    chunks of [`ChunkedDiscovery::units_per_chunk`] (`discovery_run_chunks`) and enqueues one
//!    `discover_chunk` job per chunk. An [`Enumeration::Unchanged`] catalog completes the run
//!    with no chunks, writing and retiring nothing, like an unchanged single-job discovery.
//! 2. **Chunks.** Each `discover_chunk` job (`series_id` `<run id>/<chunk index>`) calls
//!    [`discover_units`](crate::SourceAdapter::discover_units) for its units and stores the
//!    result on its chunk row. Chunk jobs are ordinary queue jobs: they run under the source's
//!    rate limiter, circuit breaker and retry policy, each retried on its own, at the priority of
//!    the `discover_catalog` job that started the run. A chunk whose job fails for good fails the
//!    run.
//! 3. **Finalize.** The chunk result transaction that stores the last chunk moves the run to
//!    `finalizing` and enqueues one `finalize_discovery` job (`series_id` the run id). That job
//!    concatenates the chunks' series in chunk order, and their curated entries missing upstream
//!    after enumeration's, and checks them exactly as a single-job discovery checks its list. Its
//!    result transaction moves the run to `complete`, deletes the chunk rows, and writes the
//!    series through the same discovery commit as a single-job discovery: the validator, the
//!    catalog metadata and, for a complete discovery, publication lifecycle reconciliation
//!    under the run's catalog scope or retirement.
//!
//! **The walk's facts live on the run.** A single-job discovery reads its catalog scope,
//! publication facts ([`CatalogLifecycle`]), reference-refresh result and observation time from
//! its adapter's memory right after the walk. A finalize may run on a worker that walked
//! nothing, so the run stores them: the scope ([`ChunkedDiscovery::catalog_scope`]), the refresh
//! result and the observation time when the run starts, enumeration's facts with its units, and
//! each chunk's facts with its result. Finalize merges them and passes them to the same write.
//!
//! **A partial run is never a catalog.** A run that fails or is superseded commits nothing: no
//! metadata, no validator, no miss counted, no retirement. Finalize itself ([`assemble`])
//! refuses, permanently, a run without every one of its chunks stored.
//!
//! **Exactly one finalize.** Every chunk commit locks its run row (`SELECT ... FOR UPDATE`) inside
//! the worker's lease-fenced result transaction, so chunk commits of one run serialize. Only the
//! commit that brings `chunks_done` up to `chunks_total` passes the conditional transition
//! `running -> finalizing`, and only that transition enqueues the finalize job. The finalize
//! commit is itself conditional on `finalizing -> complete`, so a re-run of a finalize job (after
//! a lost lease) commits nothing twice.
//!
//! **Supersede.** Starting a run marks every older `enumerating` or `running` run of the source
//! `superseded` and cancels its queued chunk jobs. A chunk of a superseded run that is already in
//! flight finds its run no longer `running` when it commits, and discards its result. A
//! `finalizing` run has every chunk stored, so it is never superseded: a `discover_catalog` job
//! that finds one starts no run, unless the run's finalize job has ended (failed or cancelled)
//! or is gone: such a run can never commit, so it is failed and a new run starts. Only the job's current claim can start a run (the claim row is
//! locked in the same transaction), so a worker whose lease expired cannot supersede the run of
//! the worker that re-claimed the job. At most one run per source is not final (a partial
//! unique index).
//!
//! **Restarts.** Runs keep no state outside the database. A worker that dies mid-chunk leaves
//! the chunk job `processing`; `release_stuck` returns it to the queue and another worker runs
//! it. A job whose lease expired and that commits late finds its claim gone and writes nothing.
//! The worker's maintenance loop also fails runs whose chunk or finalize jobs ended `failed` or
//! `cancelled` without reaching the worker's failure hook (e.g. `release_stuck` exhausting their
//! retries), `running` or `finalizing` runs left with no active chunk or finalize job at all,
//! and runs left `enumerating` with no active `discover_catalog` job.
//!
//! **Cleanup.** Finalize deletes the run's chunk rows. Final runs older than the worker's queue
//! retention are deleted, except the newest complete run of each source, and, for a source with
//! no complete run, its oldest run, so "time since the last complete run" never resets by purging.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use diesel::sql_types::{
    Array, Double, Integer, Jsonb, Nullable, Text, Timestamptz, Uuid as SqlUuid,
};
use diesel::QueryableByName;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::CrawlQueueItem;
use econ_graph_core::DatabasePool;
use econ_graph_metrics::crawler::discovery_runs::CRAWLER_DISCOVERY_RUN_METRICS;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapter::DiscoveredSeries;
use crate::dataset::SeriesDataset;
use crate::http::Validators;
use crate::lifecycle::CatalogLifecycle;
use crate::source::SourceId;

/// How an adapter discovers in chunks; see [`SourceAdapter::chunked_discovery`](crate::SourceAdapter::chunked_discovery).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkedDiscovery {
    /// Units per `discover_chunk` job (at least 1 is used). Size it so one chunk's upstream calls
    /// take a few minutes at the source's rate limit, well inside the worker's `stuck_after`.
    pub units_per_chunk: usize,
    /// The adapter's catalog scope: the fingerprint of its selection rules that
    /// [`SourceAdapter::catalog_scope`](crate::SourceAdapter::catalog_scope) reports after a
    /// single-job walk, computed here without one. `None` for an adapter without lifecycle
    /// tracking. It is recorded when a run starts and is the scope the run's finalize reconciles
    /// under; a chunk or finalize job whose adapter reports a different scope fails the run
    /// rather than commit a catalog listed under other rules.
    pub catalog_scope: Option<String>,
}

/// One work unit of a chunked discovery: a series id, a table, ... as the adapter's
/// [`enumerate_discovery`](crate::SourceAdapter::enumerate_discovery) lists it. Stored as JSON
/// with its chunk until the run completes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryUnit {
    /// The unit's identifier for the adapter.
    pub key: String,
    /// Anything the listing already returned for the unit, so the detail step needn't fetch it
    /// again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listing: Option<serde_json::Value>,
}

impl DiscoveryUnit {
    /// A unit with no listing data.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            listing: None,
        }
    }

    /// A unit carrying what the listing returned for it.
    pub fn with_listing(key: impl Into<String>, listing: serde_json::Value) -> Self {
        Self {
            key: key.into(),
            listing: Some(listing),
        }
    }
}

/// Outcome of [`SourceAdapter::enumerate_discovery`](crate::SourceAdapter::enumerate_discovery).
#[derive(Debug, Clone, PartialEq, Eq)]
// Returned once per discovery and moved straight into the run: boxing `Units` buys nothing.
#[allow(clippy::large_enum_variant)]
pub enum Enumeration {
    /// The catalog hasn't changed since the last discovery that stored a validator, as for
    /// [`Discovery::Unchanged`](crate::Discovery::Unchanged): the run completes with no chunks
    /// and writes and retires nothing.
    Unchanged,
    /// The units to discover in chunks.
    Units {
        /// Every unit, in the order their series should be listed.
        units: Vec<DiscoveryUnit>,
        /// `(url, validators)` to store with the discovered series once the run finalizes, as
        /// for [`Discovery::Changed`](crate::Discovery::Changed).
        validator: Option<(String, Validators)>,
        /// Curated or configured entries the listing found missing upstream, named as
        /// [`curated_missing`](crate::SourceAdapter::curated_missing) names them.
        curated_missing: Vec<String>,
        /// Publication facts the listing established (for example ids listed upstream outside
        /// the selection rules), as [`catalog_lifecycle`](crate::SourceAdapter::catalog_lifecycle)
        /// reports them after a single-job walk. Merged with every chunk's at finalize.
        lifecycle: CatalogLifecycle,
    },
}

/// Result of [`SourceAdapter::discover_units`](crate::SourceAdapter::discover_units) for one
/// chunk.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChunkDiscovery {
    /// The chunk's series, in unit order.
    pub found: Vec<DiscoveredSeries>,
    /// Curated or configured entries of this chunk the source no longer lists, named as
    /// [`curated_missing`](crate::SourceAdapter::curated_missing) names them. Any entry missing
    /// in any chunk stops the run's retirement, as for a single-job discovery.
    pub curated_missing: Vec<String>,
    /// Publication facts about this chunk's units (restricted, discontinued, confirmed
    /// missing, ...), as [`catalog_lifecycle`](crate::SourceAdapter::catalog_lifecycle) reports
    /// them for the whole catalog after a single-job walk.
    pub lifecycle: CatalogLifecycle,
}

/// `discovery_runs.state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// The `discover_catalog` job is listing units.
    Enumerating,
    /// Chunk jobs are queued or in progress.
    Running,
    /// Every chunk succeeded; the finalize job is queued or in progress.
    Finalizing,
    /// The run's results were committed, or the catalog was unchanged.
    Complete,
    /// Enumeration, a chunk or finalize failed; nothing was committed.
    Failed,
    /// A newer run of the source started first; nothing was committed.
    Superseded,
}

impl RunState {
    /// The string stored in `discovery_runs.state`.
    pub fn as_str(self) -> &'static str {
        match self {
            RunState::Enumerating => "enumerating",
            RunState::Running => "running",
            RunState::Finalizing => "finalizing",
            RunState::Complete => "complete",
            RunState::Failed => "failed",
            RunState::Superseded => "superseded",
        }
    }
}

/// `crawl_queue.series_id` of a run's `discover_chunk` job.
pub fn chunk_series_id(run_id: Uuid, chunk_index: i32) -> String {
    format!("{run_id}/{chunk_index}")
}

/// The run id and chunk index of a `discover_chunk` job's `series_id`.
pub fn parse_chunk_series_id(series_id: &str) -> Option<(Uuid, i32)> {
    let (run, index) = series_id.split_once('/')?;
    Some((run.parse().ok()?, index.parse().ok()?))
}

// ---------------------------------------------------------------------------
// Stored JSON
// ---------------------------------------------------------------------------

/// A [`DiscoveredSeries`] as stored in a chunk's result.
#[derive(Serialize, Deserialize)]
struct StoredSeries {
    external_id: String,
    title: String,
    description: Option<String>,
    units: Option<String>,
    frequency: Option<String>,
    data_url: Option<String>,
    dataset: String,
    dimensions: std::collections::BTreeMap<String, String>,
}

impl From<&DiscoveredSeries> for StoredSeries {
    fn from(d: &DiscoveredSeries) -> Self {
        Self {
            external_id: d.external_id.clone(),
            title: d.title.clone(),
            description: d.description.clone(),
            units: d.units.clone(),
            frequency: d.frequency.clone(),
            data_url: d.data_url.clone(),
            dataset: d.dataset.code.clone(),
            dimensions: d.dataset.dimensions.0.clone(),
        }
    }
}

impl From<StoredSeries> for DiscoveredSeries {
    fn from(s: StoredSeries) -> Self {
        Self {
            external_id: s.external_id,
            title: s.title,
            description: s.description,
            units: s.units,
            frequency: s.frequency,
            data_url: s.data_url,
            dataset: SeriesDataset::new(s.dataset, s.dimensions),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct StoredChunkResult {
    found: Vec<StoredSeries>,
    missing: Vec<String>,
    #[serde(default)]
    lifecycle: CatalogLifecycle,
}

/// Merges `more` into `facts`: every list concatenated, sorted and deduplicated.
pub(crate) fn merge_lifecycle(facts: &mut CatalogLifecycle, more: CatalogLifecycle) {
    fn add(into: &mut Vec<String>, more: Vec<String>) {
        into.extend(more);
        into.sort();
        into.dedup();
    }
    add(&mut facts.scope_ids, more.scope_ids);
    add(&mut facts.restricted_ids, more.restricted_ids);
    add(&mut facts.discontinued_ids, more.discontinued_ids);
    add(&mut facts.confirmed_missing_ids, more.confirmed_missing_ids);
    add(&mut facts.unselected_ids, more.unselected_ids);
    add(&mut facts.unselected_prefixes, more.unselected_prefixes);
}

#[derive(Serialize, Deserialize)]
struct StoredValidator {
    url: String,
    etag: Option<String>,
    last_modified: Option<String>,
    content_sha256: Option<String>,
    version: Option<String>,
}

fn to_json<T: Serialize>(value: &T) -> AppResult<serde_json::Value> {
    serde_json::to_value(value)
        .map_err(|e| AppError::InternalError(format!("encoding discovery run data: {e}")))
}

fn from_json<T: for<'de> Deserialize<'de>>(value: serde_json::Value) -> AppResult<T> {
    serde_json::from_value(value)
        .map_err(|e| AppError::InternalError(format!("decoding discovery run data: {e}")))
}

fn validator_json(
    validator: Option<&(String, Validators)>,
) -> AppResult<Option<serde_json::Value>> {
    validator
        .map(|(url, v)| {
            to_json(&StoredValidator {
                url: url.clone(),
                etag: v.etag.clone(),
                last_modified: v.last_modified.clone(),
                content_sha256: v.content_sha256.clone(),
                version: v.version.clone(),
            })
        })
        .transpose()
}

fn validator_from_json(
    value: Option<serde_json::Value>,
) -> AppResult<Option<(String, Validators)>> {
    value
        .map(|v| {
            let s: StoredValidator = from_json(v)?;
            Ok((
                s.url,
                Validators {
                    etag: s.etag,
                    last_modified: s.last_modified,
                    content_sha256: s.content_sha256,
                    version: s.version,
                },
            ))
        })
        .transpose()
}

// ---------------------------------------------------------------------------
// Metrics, published once the transaction that caused them has committed
// ---------------------------------------------------------------------------

/// Something a run or chunk did, for the run metrics.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RunEvent {
    Started,
    Completed { duration: Duration },
    Failed,
    Superseded(usize),
    ChunkCompleted { duration: Duration },
    ChunkFailed,
    ChunkDiscarded,
}

/// Run events collected inside a result transaction and published after it commits, so a
/// rolled-back transaction counts nothing.
#[derive(Clone, Default)]
pub(crate) struct RunEvents(Arc<Mutex<Vec<RunEvent>>>);

impl RunEvents {
    pub(crate) fn push(&self, event: RunEvent) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event);
    }

    /// Records the collected events for `source` and clears them.
    pub(crate) fn publish(&self, source: SourceId) {
        let events = std::mem::take(
            &mut *self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for event in events {
            publish(source, &event);
        }
    }
}

/// Records one event for `source` now (for writes outside a result transaction).
pub(crate) fn publish(source: SourceId, event: &RunEvent) {
    let m = &*CRAWLER_DISCOVERY_RUN_METRICS;
    let src = source.as_str();
    let runs = |outcome: &str, n: usize| {
        m.runs_total
            .with_label_values(&[src, outcome])
            .inc_by(u64::try_from(n).unwrap_or(u64::MAX));
    };
    match event {
        RunEvent::Started => runs("started", 1),
        RunEvent::Completed { duration } => {
            runs("complete", 1);
            m.run_duration_seconds
                .with_label_values(&[src])
                .observe(duration.as_secs_f64());
        }
        RunEvent::Failed => runs("failed", 1),
        RunEvent::Superseded(n) => runs("superseded", *n),
        RunEvent::ChunkCompleted { duration } => {
            m.chunks_total.with_label_values(&[src, "completed"]).inc();
            m.chunk_duration_seconds
                .with_label_values(&[src])
                .observe(duration.as_secs_f64());
        }
        RunEvent::ChunkFailed => m.chunks_total.with_label_values(&[src, "failed"]).inc(),
        RunEvent::ChunkDiscarded => m.chunks_total.with_label_values(&[src, "discarded"]).inc(),
    }
}

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

fn conn_err(e: impl std::fmt::Display) -> AppError {
    AppError::DatabaseError(format!("Failed to get database connection: {e}"))
}

/// A `discovery_runs` row, as chunk and finalize jobs read it.
#[derive(Debug, Clone, QueryableByName)]
pub(crate) struct RunRow {
    #[diesel(sql_type = SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type = Text)]
    pub state: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub catalog_scope: Option<String>,
    #[diesel(sql_type = Nullable<Jsonb>)]
    pub validator: Option<serde_json::Value>,
    #[diesel(sql_type = Jsonb)]
    pub curated_missing: serde_json::Value,
    #[diesel(sql_type = Jsonb)]
    pub lifecycle: serde_json::Value,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    pub reference_ok: bool,
    #[diesel(sql_type = Timestamptz)]
    pub observed_at: DateTime<Utc>,
    #[diesel(sql_type = Integer)]
    pub chunks_total: i32,
}

impl RunRow {
    /// The validator enumeration returned, to store when the run finalizes.
    pub(crate) fn validator(&self) -> AppResult<Option<(String, Validators)>> {
        validator_from_json(self.validator.clone())
    }

    /// Curated entries enumeration found missing.
    pub(crate) fn curated_missing(&self) -> AppResult<Vec<String>> {
        from_json(self.curated_missing.clone())
    }

    /// Publication facts enumeration established.
    pub(crate) fn lifecycle(&self) -> AppResult<CatalogLifecycle> {
        from_json(self.lifecycle.clone())
    }
}

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

/// Cancels the queued (not yet claimed) chunk and finalize jobs of `run_ids`. Jobs in flight are
/// left to discard their own results when they commit.
async fn cancel_jobs_conn(
    conn: &mut AsyncPgConnection,
    source: &str,
    run_ids: &[Uuid],
    reason: &str,
) -> AppResult<usize> {
    if run_ids.is_empty() {
        return Ok(0);
    }
    let ids: Vec<String> = run_ids.iter().map(Uuid::to_string).collect();
    Ok(diesel::sql_query(
        "UPDATE crawl_queue SET status = 'cancelled', finished_at = NOW(), updated_at = NOW(), \
             error_message = $3 \
         WHERE kind IN ('discover_chunk', 'finalize_discovery') \
           AND split_part(series_id, '/', 1) = ANY($2) \
           AND source = $1 AND status IN ('pending', 'retrying')",
    )
    .bind::<Text, _>(source)
    .bind::<Array<Text>, _>(&ids)
    .bind::<Text, _>(reason)
    .execute(conn)
    .await?)
}

/// SQL condition: run `r` has an active (pending, processing or retrying) finalize job.
const ACTIVE_FINALIZE: &str = "EXISTS (SELECT 1 FROM crawl_queue q \
     WHERE q.kind = 'finalize_discovery' AND split_part(q.series_id, '/', 1) = r.id::text \
       AND q.source = r.source AND q.status IN ('pending', 'processing', 'retrying'))";

/// Outcome of [`start_run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunStart {
    /// A new run in state `enumerating`, after superseding this many older runs and failing
    /// `abandoned` finalizing runs whose finalize job had ended or was gone.
    Started {
        run_id: Uuid,
        superseded: usize,
        abandoned: usize,
    },
    /// The `discover_catalog` claim is no longer current (its lease expired and the job may have
    /// been claimed again): nothing changed, and the caller must not start a run.
    ClaimLost,
    /// A run of the source has every chunk stored and is waiting for its finalize job: nothing
    /// changed. Superseding it would throw away a finished catalog, so this discovery starts no
    /// run and the finalizing one commits.
    Finalizing { run_id: Uuid },
}

/// Starts a run of `source` in state `enumerating` for the `discover_catalog` job `claim`, first
/// superseding any older run that is `enumerating` or `running` (and cancelling its queued
/// jobs). See [`RunStart`] for when it starts nothing.
///
/// `reference_ok` (whether the job's reference-data refresh succeeded) and `observed_at` (the
/// worker's clock as the walk starts) are stored on the run for its finalize, as a single-job
/// discovery holds them in memory.
pub(crate) async fn start_run(
    pool: &DatabasePool,
    source: SourceId,
    settings: &ChunkedDiscovery,
    claim: &CrawlQueueItem,
    reference_ok: bool,
    observed_at: DateTime<Utc>,
) -> AppResult<RunStart> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    let source = source.as_str();
    let fingerprint = settings.catalog_scope.clone();
    let priority = claim.priority;
    conn.transaction::<_, AppError, _>(async move |conn| {
        // The claim row stays locked until commit, so the lease can't be released and re-claimed
        // in between (`release_stuck` and re-claims update the row).
        if claim.locked_by.is_none() || !CrawlQueueItem::lock_current_claim(conn, claim).await? {
            return Ok(RunStart::ClaimLost);
        }
        // Serializes run starts of one source, so the partial unique index never rejects one.
        diesel::sql_query(
            "SELECT pg_advisory_xact_lock(hashtextextended('discovery_runs:' || $1, 0))",
        )
        .bind::<Text, _>(source)
        .execute(conn)
        .await?;
        // Lock the source's unfinished runs before deciding anything about them. A chunk commit
        // locks its run row too, so a last chunk that moves its run from running to finalizing
        // either commits before this lock is granted (and the statements below see the run
        // finalizing, with its finalize job) or waits until this transaction ends.
        diesel::sql_query(
            "SELECT id FROM discovery_runs \
             WHERE source = $1 AND state IN ('enumerating', 'running', 'finalizing') \
             FOR UPDATE",
        )
        .bind::<Text, _>(source)
        .execute(conn)
        .await?;
        // A finalizing run whose finalize job is still active will commit: leave it alone, and
        // let it settle this discovery job too. One whose finalize job ended (failed or
        // cancelled without the failure hook) or is gone can never commit, so fail it and start
        // a new run instead of waiting on it.
        let finalizing: Vec<IdRow> = diesel::sql_query(format!(
            "UPDATE discovery_runs r \
             SET deferred_discovery_job_ids = array_append(deferred_discovery_job_ids, $2) \
             WHERE r.source = $1 AND r.state = 'finalizing' AND {ACTIVE_FINALIZE} RETURNING id"
        ))
        .bind::<Text, _>(source)
        .bind::<SqlUuid, _>(claim.id)
        .load(conn)
        .await?;
        if let Some(run) = finalizing.into_iter().next() {
            return Ok(RunStart::Finalizing { run_id: run.id });
        }
        let abandoned = diesel::sql_query(format!(
            "UPDATE discovery_runs r SET state = 'failed', finished_at = NOW(), \
                 updated_at = NOW(), error_message = 'its finalize job ended without completing' \
             WHERE r.source = $1 AND r.state = 'finalizing' AND NOT {ACTIVE_FINALIZE}"
        ))
        .bind::<Text, _>(source)
        .execute(conn)
        .await?;
        let superseded: Vec<IdRow> = diesel::sql_query(
            "UPDATE discovery_runs SET state = 'superseded', finished_at = NOW(), \
                 updated_at = NOW(), error_message = 'superseded by a newer run' \
             WHERE source = $1 AND state IN ('enumerating', 'running') RETURNING id",
        )
        .bind::<Text, _>(source)
        .load(conn)
        .await?;
        let superseded: Vec<Uuid> = superseded.into_iter().map(|r| r.id).collect();
        cancel_jobs_conn(
            conn,
            source,
            &superseded,
            "discovery run superseded by a newer run",
        )
        .await?;
        let run: IdRow = diesel::sql_query(
            "INSERT INTO discovery_runs (source, state, catalog_scope, priority, \
                 discovery_job_id, reference_ok, observed_at) \
             VALUES ($1, 'enumerating', $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind::<Text, _>(source)
        .bind::<Nullable<Text>, _>(fingerprint)
        .bind::<Integer, _>(priority)
        .bind::<SqlUuid, _>(claim.id)
        .bind::<diesel::sql_types::Bool, _>(reference_ok)
        .bind::<Timestamptz, _>(observed_at)
        .get_result(conn)
        .await?;
        Ok(RunStart::Started {
            run_id: run.id,
            superseded: superseded.len(),
            abandoned,
        })
    })
    .await
}

/// Supersedes every run of `source` that is not final (finalizing included) and cancels its
/// queued jobs: a single-job discovery is about to commit the source's catalog, so no run may
/// commit after it. Returns how many runs were superseded.
pub(crate) async fn supersede_active_runs(
    pool: &DatabasePool,
    source: SourceId,
) -> AppResult<usize> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    let source = source.as_str();
    conn.transaction::<_, AppError, _>(async move |conn| {
        diesel::sql_query(
            "SELECT pg_advisory_xact_lock(hashtextextended('discovery_runs:' || $1, 0))",
        )
        .bind::<Text, _>(source)
        .execute(conn)
        .await?;
        let superseded: Vec<IdRow> = diesel::sql_query(
            "UPDATE discovery_runs SET state = 'superseded', finished_at = NOW(), \
                 updated_at = NOW(), error_message = 'superseded by a single-job discovery' \
             WHERE source = $1 AND state IN ('enumerating', 'running', 'finalizing') \
             RETURNING id",
        )
        .bind::<Text, _>(source)
        .load(conn)
        .await?;
        let superseded: Vec<Uuid> = superseded.into_iter().map(|r| r.id).collect();
        cancel_jobs_conn(
            conn,
            source,
            &superseded,
            "superseded by a single-job discovery",
        )
        .await?;
        Ok(superseded.len())
    })
    .await
}

/// Fails a run that isn't final yet (enumeration failed). Outside any result transaction: a
/// retried `discover_catalog` job starts a new run anyway. Returns whether the run was failed.
pub(crate) async fn fail_enumerating_run(
    pool: &DatabasePool,
    run_id: Uuid,
    error: &str,
) -> AppResult<bool> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    let n = diesel::sql_query(
        "UPDATE discovery_runs SET state = 'failed', finished_at = NOW(), updated_at = NOW(), \
             error_message = $2 \
         WHERE id = $1 AND state = 'enumerating'",
    )
    .bind::<SqlUuid, _>(run_id)
    .bind::<Text, _>(error)
    .execute(&mut conn)
    .await?;
    Ok(n > 0)
}

/// The run `run_id`, if it exists.
pub(crate) async fn load_run(pool: &DatabasePool, run_id: Uuid) -> AppResult<Option<RunRow>> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    let rows: Vec<RunRow> = diesel::sql_query(
        "SELECT id, state::text AS state, catalog_scope, validator, curated_missing, \
             lifecycle, reference_ok, observed_at, chunks_total \
         FROM discovery_runs WHERE id = $1",
    )
    .bind::<SqlUuid, _>(run_id)
    .load(&mut conn)
    .await?;
    Ok(rows.into_iter().next())
}

/// A chunk that is still `pending` in a `running` run; `None` when there is
/// nothing to do (the run or chunk is gone, finished, failed or superseded).
pub(crate) async fn pending_chunk_units(
    pool: &DatabasePool,
    run_id: Uuid,
    chunk_index: i32,
) -> AppResult<Option<PendingChunk>> {
    #[derive(QueryableByName)]
    struct Row {
        #[diesel(sql_type = Jsonb)]
        units: serde_json::Value,
        #[diesel(sql_type = Nullable<Text>)]
        catalog_scope: Option<String>,
    }
    let mut conn = pool.get().await.map_err(conn_err)?;
    let rows: Vec<Row> = diesel::sql_query(
        "SELECT c.units, r.catalog_scope \
         FROM discovery_run_chunks c JOIN discovery_runs r ON r.id = c.run_id \
         WHERE c.run_id = $1 AND c.chunk_index = $2 AND c.state = 'pending' \
           AND r.state = 'running'",
    )
    .bind::<SqlUuid, _>(run_id)
    .bind::<Integer, _>(chunk_index)
    .load(&mut conn)
    .await?;
    rows.into_iter()
        .next()
        .map(|r| {
            Ok(PendingChunk {
                units: from_json(r.units)?,
                catalog_scope: r.catalog_scope,
            })
        })
        .transpose()
}

/// A chunk still waiting for its details, with its run's catalog scope.
pub(crate) struct PendingChunk {
    pub units: Vec<DiscoveryUnit>,
    pub catalog_scope: Option<String>,
}

/// A run's catalog, assembled from its chunks for finalize.
pub(crate) struct AssembledCatalog {
    pub found: Vec<DiscoveredSeries>,
    pub missing: Vec<String>,
    pub lifecycle: CatalogLifecycle,
}

/// Every chunk's series in chunk order, every curated entry missing (enumeration's first, then
/// each chunk's in chunk order) and the merged publication facts.
///
/// This is the partial-catalog guard: it errors unless every one of the run's `chunks_total`
/// chunks is stored as done, so a catalog missing a chunk (failed, superseded, deleted) can
/// never be committed, reconciled or retired against as if it were complete. The error is a
/// validation error, so the finalize job fails for good (and fails its run) rather than retry.
pub(crate) async fn assemble(pool: &DatabasePool, run: &RunRow) -> AppResult<AssembledCatalog> {
    #[derive(QueryableByName)]
    struct Row {
        #[diesel(sql_type = Text)]
        state: String,
        #[diesel(sql_type = Nullable<Jsonb>)]
        result: Option<serde_json::Value>,
    }
    let mut conn = pool.get().await.map_err(conn_err)?;
    let rows: Vec<Row> = diesel::sql_query(
        "SELECT state::text AS state, result FROM discovery_run_chunks \
         WHERE run_id = $1 ORDER BY chunk_index",
    )
    .bind::<SqlUuid, _>(run.id)
    .load(&mut conn)
    .await?;
    if rows.len() != usize::try_from(run.chunks_total).unwrap_or(usize::MAX) {
        return Err(AppError::ValidationError(format!(
            "discovery run {} has {} of its {} chunks at finalize",
            run.id,
            rows.len(),
            run.chunks_total
        )));
    }
    let mut found = Vec::new();
    let mut missing = run.curated_missing()?;
    let mut lifecycle = run.lifecycle()?;
    for row in rows {
        let Some(result) = row.result.filter(|_| row.state == "done") else {
            return Err(AppError::ValidationError(format!(
                "discovery run {} has a chunk in state {} at finalize",
                run.id, row.state
            )));
        };
        let chunk: StoredChunkResult = from_json(result)?;
        found.extend(chunk.found.into_iter().map(DiscoveredSeries::from));
        missing.extend(chunk.missing);
        merge_lifecycle(&mut lifecycle, chunk.lifecycle);
    }
    Ok(AssembledCatalog {
        found,
        missing,
        lifecycle,
    })
}

/// Inserts the `finalize_discovery` job of `run_id`.
async fn enqueue_finalize_conn(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
    max_retries: i32,
) -> AppResult<()> {
    diesel::sql_query(
        "INSERT INTO crawl_queue (source, series_id, priority, max_retries, kind) \
         SELECT source, id::text, priority, $2, 'finalize_discovery' FROM discovery_runs \
         WHERE id = $1 \
         ON CONFLICT DO NOTHING",
    )
    .bind::<SqlUuid, _>(run_id)
    .bind::<Integer, _>(max_retries)
    .execute(conn)
    .await?;
    Ok(())
}

/// Moves a `running` run whose chunks are all done to `finalizing` and enqueues its finalize job.
/// The conditional transition is what makes finalize happen once: only one transaction can
/// apply it. Returns whether it applied.
async fn begin_finalize_conn(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
    max_retries: i32,
) -> AppResult<bool> {
    let moved: Vec<IdRow> = diesel::sql_query(
        "UPDATE discovery_runs SET state = 'finalizing', updated_at = NOW() \
         WHERE id = $1 AND state = 'running' AND chunks_done = chunks_total \
           AND chunks_failed = 0 RETURNING id",
    )
    .bind::<SqlUuid, _>(run_id)
    .load(conn)
    .await?;
    if moved.is_empty() {
        return Ok(false);
    }
    enqueue_finalize_conn(conn, run_id, max_retries).await?;
    Ok(true)
}

/// Enumeration's result transaction: moves the run from `enumerating` to `running`, stores its
/// chunks and enqueues their jobs (or, with no units, goes straight to `finalizing`). Returns
/// the number of chunks, or `None` when the run is no longer `enumerating` (superseded), in
/// which case nothing is written.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn commit_enumeration_conn(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
    units: &[DiscoveryUnit],
    units_per_chunk: usize,
    validator: Option<&(String, Validators)>,
    curated_missing: &[String],
    lifecycle: &CatalogLifecycle,
    max_retries: i32,
) -> AppResult<Option<usize>> {
    let chunks: Vec<&[DiscoveryUnit]> = units.chunks(units_per_chunk.max(1)).collect();
    let total = i32::try_from(chunks.len())
        .map_err(|_| AppError::ValidationError("too many discovery chunks".into()))?;
    let moved: Vec<IdRow> = diesel::sql_query(
        "UPDATE discovery_runs SET state = 'running', chunks_total = $2, validator = $3, \
             curated_missing = $4, lifecycle = $5, enumerated_at = NOW(), updated_at = NOW() \
         WHERE id = $1 AND state = 'enumerating' RETURNING id",
    )
    .bind::<SqlUuid, _>(run_id)
    .bind::<Integer, _>(total)
    .bind::<Nullable<Jsonb>, _>(validator_json(validator)?)
    .bind::<Jsonb, _>(to_json(&curated_missing)?)
    .bind::<Jsonb, _>(to_json(lifecycle)?)
    .load(conn)
    .await?;
    if moved.is_empty() {
        return Ok(None);
    }
    if chunks.is_empty() {
        begin_finalize_conn(conn, run_id, max_retries).await?;
        return Ok(Some(0));
    }
    diesel::sql_query(
        "INSERT INTO discovery_run_chunks (run_id, chunk_index, units) \
         SELECT $1, (t.ordinality - 1)::int, t.value \
         FROM jsonb_array_elements($2) WITH ORDINALITY AS t",
    )
    .bind::<SqlUuid, _>(run_id)
    .bind::<Jsonb, _>(to_json(&chunks)?)
    .execute(conn)
    .await?;
    diesel::sql_query(
        "INSERT INTO crawl_queue (source, series_id, priority, max_retries, kind) \
         SELECT r.source, r.id::text || '/' || g, r.priority, $3, 'discover_chunk' \
         FROM discovery_runs r, generate_series(0, $2 - 1) AS g \
         WHERE r.id = $1 \
         ORDER BY g \
         ON CONFLICT DO NOTHING",
    )
    .bind::<SqlUuid, _>(run_id)
    .bind::<Integer, _>(total)
    .bind::<Integer, _>(max_retries)
    .execute(conn)
    .await?;
    Ok(Some(chunks.len()))
}

/// Enumeration found the catalog unchanged: completes the run with no chunks. Returns whether
/// the run was still `enumerating`.
pub(crate) async fn commit_unchanged_conn(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
) -> AppResult<Option<Duration>> {
    #[derive(QueryableByName)]
    struct Row {
        #[diesel(sql_type = Double)]
        seconds: f64,
    }
    let done: Vec<Row> = diesel::sql_query(
        "UPDATE discovery_runs SET state = 'complete', catalog_unchanged = TRUE, \
             enumerated_at = NOW(), finished_at = NOW(), updated_at = NOW() \
         WHERE id = $1 AND state = 'enumerating' \
         RETURNING EXTRACT(EPOCH FROM (NOW() - started_at))::float8 AS seconds",
    )
    .bind::<SqlUuid, _>(run_id)
    .load(conn)
    .await?;
    Ok(done
        .into_iter()
        .next()
        .map(|d| Duration::from_secs_f64(d.seconds.max(0.0))))
}

/// What a chunk's result transaction did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChunkCommit {
    /// Stored; the run still has chunks to go.
    Stored,
    /// Stored, and it was the run's last chunk: the run is `finalizing` and its finalize job is
    /// queued.
    StoredLast,
    /// Discarded: the run is no longer `running` (failed or superseded) or the chunk was already
    /// done.
    Discarded,
}

/// A chunk's result transaction: locks the run, stores the chunk's result and counts it, and,
/// for the last chunk, begins finalize. Nothing is stored for a run that is no longer `running`.
pub(crate) async fn commit_chunk_conn(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
    chunk_index: i32,
    result: &ChunkDiscovery,
    max_retries: i32,
) -> AppResult<ChunkCommit> {
    #[derive(QueryableByName)]
    struct StateRow {
        #[diesel(sql_type = Text)]
        state: String,
    }
    // The run row lock serializes every chunk commit of this run.
    let state: Vec<StateRow> = diesel::sql_query(
        "SELECT state::text AS state FROM discovery_runs WHERE id = $1 FOR UPDATE",
    )
    .bind::<SqlUuid, _>(run_id)
    .load(conn)
    .await?;
    if state.into_iter().next().map(|s| s.state).as_deref() != Some(RunState::Running.as_str()) {
        return Ok(ChunkCommit::Discarded);
    }
    let stored = StoredChunkResult {
        found: result.found.iter().map(StoredSeries::from).collect(),
        missing: result.curated_missing.clone(),
        lifecycle: result.lifecycle.clone(),
    };
    let n = diesel::sql_query(
        "UPDATE discovery_run_chunks SET state = 'done', result = $3, finished_at = NOW() \
         WHERE run_id = $1 AND chunk_index = $2 AND state = 'pending'",
    )
    .bind::<SqlUuid, _>(run_id)
    .bind::<Integer, _>(chunk_index)
    .bind::<Jsonb, _>(to_json(&stored)?)
    .execute(conn)
    .await?;
    if n == 0 {
        return Ok(ChunkCommit::Discarded);
    }
    diesel::sql_query(
        "UPDATE discovery_runs SET chunks_done = chunks_done + 1, updated_at = NOW() WHERE id = $1",
    )
    .bind::<SqlUuid, _>(run_id)
    .execute(conn)
    .await?;
    Ok(if begin_finalize_conn(conn, run_id, max_retries).await? {
        ChunkCommit::StoredLast
    } else {
        ChunkCommit::Stored
    })
}

/// Finalize's result transaction, before the discovery write: moves the run from `finalizing`
/// to `complete` and deletes its chunk rows. Returns how long the run took, or `None` when the
/// run is no longer `finalizing` (superseded, failed, or already complete), in which case the
/// caller must write nothing.
pub(crate) async fn complete_run_conn(
    conn: &mut AsyncPgConnection,
    run_id: Uuid,
) -> AppResult<Option<Duration>> {
    #[derive(QueryableByName)]
    struct Row {
        #[diesel(sql_type = Double)]
        seconds: f64,
    }
    let done: Vec<Row> = diesel::sql_query(
        "UPDATE discovery_runs SET state = 'complete', finished_at = NOW(), updated_at = NOW() \
         WHERE id = $1 AND state = 'finalizing' \
           AND chunks_done = chunks_total AND chunks_failed = 0 \
         RETURNING EXTRACT(EPOCH FROM (NOW() - started_at))::float8 AS seconds",
    )
    .bind::<SqlUuid, _>(run_id)
    .load(conn)
    .await?;
    let Some(done) = done.into_iter().next() else {
        return Ok(None);
    };
    diesel::sql_query("DELETE FROM discovery_run_chunks WHERE run_id = $1")
        .bind::<SqlUuid, _>(run_id)
        .execute(conn)
        .await?;
    Ok(Some(Duration::from_secs_f64(done.seconds.max(0.0))))
}

/// A chunk or finalize job of `run_id` failed for good (in that job's result transaction):
/// marks the chunk failed, fails the run if it is still `running` or `finalizing`, and cancels
/// the run's queued jobs. `chunk_index` is `None` for the finalize job. Returns whether the run
/// was failed by this call.
pub(crate) async fn fail_run_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    run_id: Uuid,
    chunk_index: Option<i32>,
    error: &str,
) -> AppResult<bool> {
    let mut chunk_failed = 0;
    if let Some(index) = chunk_index {
        chunk_failed = diesel::sql_query(
            "UPDATE discovery_run_chunks SET state = 'failed', error_message = $3, \
                 finished_at = NOW() \
             WHERE run_id = $1 AND chunk_index = $2 AND state = 'pending'",
        )
        .bind::<SqlUuid, _>(run_id)
        .bind::<Integer, _>(index)
        .bind::<Text, _>(error)
        .execute(&mut *conn)
        .await?;
    }
    let failed: Vec<IdRow> = diesel::sql_query(
        "UPDATE discovery_runs SET state = 'failed', finished_at = NOW(), updated_at = NOW(), \
             chunks_failed = chunks_failed + $2, error_message = $3 \
         WHERE id = $1 AND state IN ('running', 'finalizing') RETURNING id",
    )
    .bind::<SqlUuid, _>(run_id)
    .bind::<Integer, _>(i32::try_from(chunk_failed).unwrap_or(0))
    .bind::<Text, _>(error)
    .load(&mut *conn)
    .await?;
    if failed.is_empty() {
        return Ok(false);
    }
    cancel_jobs_conn(conn, source.as_str(), &[run_id], "discovery run failed").await?;
    Ok(true)
}

/// Fails runs whose jobs ended without the worker's failure hook: a `running` or `finalizing`
/// run with a chunk or finalize job that is `failed` or `cancelled` (e.g. `release_stuck`
/// exhausted its retries, or an operator cancelled it); a `running` or `finalizing` run with no
/// active chunk or finalize job at all (its rows were deleted), which nothing would ever finish;
/// and an `enumerating` run whose source has no active `discover_catalog` job. A run's jobs are
/// enqueued in the same transaction that moves it to `running` or `finalizing`, so a run in
/// progress always has one. Returns the failed runs' sources, one entry per run.
pub async fn fail_abandoned_runs(pool: &DatabasePool) -> AppResult<Vec<String>> {
    #[derive(QueryableByName)]
    struct Row {
        #[diesel(sql_type = SqlUuid)]
        id: Uuid,
        #[diesel(sql_type = Text)]
        source: String,
    }
    let mut conn = pool.get().await.map_err(conn_err)?;
    conn.transaction::<_, AppError, _>(async |conn| {
        let rows: Vec<Row> = diesel::sql_query(
            "UPDATE discovery_runs r SET state = 'failed', finished_at = NOW(), updated_at = NOW(), \
                 error_message = CASE WHEN r.state = 'enumerating' \
                     THEN 'its discover_catalog job ended without finishing enumeration' \
                     ELSE 'its chunk or finalize jobs ended without completing' END \
             WHERE (r.state IN ('running', 'finalizing') AND EXISTS ( \
                     SELECT 1 FROM crawl_queue q \
                     WHERE q.kind IN ('discover_chunk', 'finalize_discovery') \
                       AND split_part(q.series_id, '/', 1) = r.id::text \
                       AND q.source = r.source AND q.status IN ('failed', 'cancelled'))) \
                OR (r.state IN ('running', 'finalizing') AND NOT EXISTS ( \
                     SELECT 1 FROM crawl_queue q \
                     WHERE q.kind IN ('discover_chunk', 'finalize_discovery') \
                       AND split_part(q.series_id, '/', 1) = r.id::text \
                       AND q.source = r.source \
                       AND q.status IN ('pending', 'processing', 'retrying'))) \
                OR (r.state = 'enumerating' AND NOT EXISTS ( \
                     SELECT 1 FROM crawl_queue q \
                     WHERE q.source = r.source AND q.kind = 'discover_catalog' \
                       AND q.status IN ('pending', 'processing', 'retrying'))) \
             RETURNING r.id, r.source::text AS source",
        )
        .load(&mut *conn)
        .await?;
        for row in &rows {
            cancel_jobs_conn(conn, &row.source, &[row.id], "discovery run failed").await?;
        }
        Ok(rows.into_iter().map(|r| r.source).collect())
    })
    .await
}

/// Deletes final runs (and their chunks) that finished at least `older_than` ago, keeping each
/// source's newest complete run and, for a source with no complete run, its oldest run. Returns
/// the number of runs deleted.
pub async fn purge_finished_runs(pool: &DatabasePool, older_than: Duration) -> AppResult<usize> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    Ok(diesel::sql_query(
        "DELETE FROM discovery_runs r \
         WHERE r.state IN ('complete', 'failed', 'superseded') \
           AND r.finished_at <= NOW() - make_interval(secs => $1) \
           AND r.id NOT IN ( \
               SELECT DISTINCT ON (source) id FROM discovery_runs \
               WHERE state = 'complete' ORDER BY source, finished_at DESC, id DESC) \
           AND r.id NOT IN ( \
               SELECT DISTINCT ON (source) id FROM discovery_runs d \
               WHERE NOT EXISTS (SELECT 1 FROM discovery_runs c \
                                 WHERE c.source = d.source AND c.state = 'complete') \
               ORDER BY source, started_at, id)",
    )
    .bind::<Double, _>(older_than.as_secs_f64())
    .execute(&mut conn)
    .await?)
}

/// One source's newest run, for `crawler status` and the run gauges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, QueryableByName)]
pub struct DiscoveryRunProgress {
    /// `discovery_runs.source`.
    #[diesel(sql_type = Text)]
    pub source: String,
    /// The newest run's id.
    #[diesel(sql_type = SqlUuid)]
    pub run_id: Uuid,
    /// The newest run's state ([`RunState::as_str`]).
    #[diesel(sql_type = Text)]
    pub state: String,
    /// When the newest run started.
    #[diesel(sql_type = Timestamptz)]
    pub started_at: DateTime<Utc>,
    /// When the newest run finished, if it is final.
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub finished_at: Option<DateTime<Utc>>,
    /// The newest run's chunks.
    #[diesel(sql_type = Integer)]
    pub chunks_total: i32,
    /// Chunks stored.
    #[diesel(sql_type = Integer)]
    pub chunks_done: i32,
    /// Chunks that failed.
    #[diesel(sql_type = Integer)]
    pub chunks_failed: i32,
    /// When the source's newest complete run finished.
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub last_complete_at: Option<DateTime<Utc>>,
    /// When the source's oldest retained run started.
    #[diesel(sql_type = Timestamptz)]
    pub first_started_at: DateTime<Utc>,
}

impl DiscoveryRunProgress {
    /// Whether the newest run is still enumerating, running or finalizing.
    pub fn is_active(&self) -> bool {
        matches!(
            self.state.as_str(),
            "enumerating" | "running" | "finalizing"
        )
    }
}

/// Each source's newest run, ordered by source.
pub(crate) const PROGRESS_SQL: &str = "\
SELECT DISTINCT ON (r.source)
    r.source::text AS source, r.id AS run_id, r.state::text AS state, r.started_at, r.finished_at,
    r.chunks_total, r.chunks_done, r.chunks_failed,
    (SELECT max(c.finished_at) FROM discovery_runs c
     WHERE c.source = r.source AND c.state = 'complete') AS last_complete_at,
    (SELECT min(f.started_at) FROM discovery_runs f WHERE f.source = r.source) AS first_started_at
FROM discovery_runs r
ORDER BY r.source, r.started_at DESC, r.id DESC";

/// Each source's newest run, ordered by source.
pub async fn run_progress(pool: &DatabasePool) -> AppResult<Vec<DiscoveryRunProgress>> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    Ok(diesel::sql_query(PROGRESS_SQL).load(&mut conn).await?)
}

/// The newest run of each of `sources` whose `data_sources` row is enabled or missing, ordered
/// by source: what the run gauges and their alert cover. Pass the sources whose adapter
/// currently discovers in chunks, so a source that stops opting in, or is disabled, drops out.
pub async fn alerting_run_progress(
    pool: &DatabasePool,
    sources: &[SourceId],
) -> AppResult<Vec<DiscoveryRunProgress>> {
    #[derive(QueryableByName)]
    struct Code {
        #[diesel(sql_type = Text)]
        code: String,
    }
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    let codes: Vec<String> = sources.iter().map(|s| s.as_str().to_string()).collect();
    let names: Vec<String> = sources
        .iter()
        .map(|&s| crate::persist::data_source_template(s).name)
        .collect();
    let mut conn = pool.get().await.map_err(conn_err)?;
    let enabled: Vec<Code> = diesel::sql_query(
        "SELECT m.code FROM unnest($1::text[], $2::text[]) AS m(code, ds_name) \
         LEFT JOIN data_sources ds ON ds.name = m.ds_name \
         WHERE ds.id IS NULL OR ds.is_enabled",
    )
    .bind::<Array<Text>, _>(&codes)
    .bind::<Array<Text>, _>(&names)
    .load(&mut conn)
    .await?;
    drop(conn);
    let enabled: std::collections::HashSet<String> = enabled.into_iter().map(|c| c.code).collect();
    Ok(run_progress(pool)
        .await?
        .into_iter()
        .filter(|r| enabled.contains(&r.source))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_series_ids_round_trip() {
        let run = Uuid::new_v4();
        assert_eq!(
            parse_chunk_series_id(&chunk_series_id(run, 7)),
            Some((run, 7))
        );
        assert_eq!(parse_chunk_series_id("not-a-run/1"), None);
        assert_eq!(parse_chunk_series_id(&run.to_string()), None);
    }

    #[test]
    fn stored_series_round_trips() {
        let series = DiscoveredSeries {
            external_id: "t5_x".into(),
            title: "X".into(),
            description: Some("d".into()),
            units: None,
            frequency: Some("Monthly".into()),
            data_url: None,
            dataset: SeriesDataset::new("t5_ds", [("indicator", "a"), ("area", "b")]),
        };
        let json = serde_json::to_value(StoredSeries::from(&series)).unwrap();
        let back: StoredSeries = serde_json::from_value(json).unwrap();
        assert_eq!(DiscoveredSeries::from(back), series);
    }

    #[test]
    fn validators_round_trip() {
        let v = (
            "https://example.test/catalog".to_string(),
            Validators {
                etag: Some("\"e\"".into()),
                last_modified: None,
                content_sha256: Some("abc".into()),
                version: Some("1".into()),
            },
        );
        let json = validator_json(Some(&v)).unwrap();
        assert_eq!(validator_from_json(json).unwrap(), Some(v));
        assert_eq!(validator_from_json(None).unwrap(), None);
    }
}
