// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The worker's side of chunked catalog discovery (see [`crate::discovery_run`]): dispatch of
//! enumeration, `discover_chunk` and `finalize_discovery` jobs, and their writes inside the
//! worker's lease-fenced result transaction.

use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use diesel_async::AsyncPgConnection;
use econ_graph_core::models::{CrawlQueueItem, JobKind};
use econ_graph_core::AppResult;
use uuid::Uuid;

use super::{db_error, guarded, Fetched, JobOutcome, JobStats, JobWrite, Worker};
use crate::adapter::SourceAdapter;
use crate::discovery_run::{
    self, ChunkCommit, ChunkDiscovery, ChunkedDiscovery, DiscoveryUnit, Enumeration, RunEvent,
    RunEvents, RunStart,
};
use crate::error::CrawlError;
use crate::http::Validators;
use crate::source::SourceId;

/// A chunked-discovery job's upstream work is done; its writes happen in the result transaction.
#[derive(Clone)]
pub(super) enum RunWrite {
    /// Enumeration listed `units` for `run_id`.
    Enumerated {
        run_id: Uuid,
        units: Vec<DiscoveryUnit>,
        units_per_chunk: usize,
        validator: Option<(String, Validators)>,
        curated_missing: Vec<String>,
    },
    /// Enumeration found the catalog unchanged.
    Unchanged { run_id: Uuid },
    /// One chunk's details.
    Chunk {
        run_id: Uuid,
        chunk_index: i32,
        result: ChunkDiscovery,
        elapsed: Duration,
    },
    /// The run's results, assembled and checked, to commit as a discovery once the run moves
    /// from `finalizing` to `complete`.
    Finalize {
        run_id: Uuid,
        discovery: Box<JobWrite>,
    },
    /// Nothing to do: the job's run is no longer waiting for it.
    Skipped,
}

impl Worker {
    /// The adapter for `source` and its chunked discovery settings, if it discovers in chunks.
    pub(super) fn chunked_discovery(
        &self,
        source: SourceId,
    ) -> Option<(Arc<dyn SourceAdapter>, ChunkedDiscovery)> {
        let adapter = self.registry.get(source)?;
        let settings = adapter.chunked_discovery()?;
        Some((adapter, settings))
    }

    fn max_retries(&self, source: SourceId) -> i32 {
        i32::try_from(self.ctx.http.policy(source).max_retries.min(10)).unwrap_or(10)
    }

    /// A `discover_catalog` job of an adapter that discovers in chunks: refresh reference data,
    /// start a run and list its units.
    pub(super) async fn enumerate_run(
        &self,
        source: SourceId,
        adapter: Arc<dyn SourceAdapter>,
        settings: ChunkedDiscovery,
        item: &CrawlQueueItem,
    ) -> Fetched {
        {
            let adapter = adapter.clone();
            let ctx = self.ctx.clone();
            if let Err(error) =
                guarded(async move { adapter.refresh_reference_data(&ctx).await }).await
            {
                tracing::warn!(
                    %source,
                    %error,
                    "refresh_reference_data failed; discovering with existing reference data"
                );
            }
        }
        let (run_id, superseded) =
            match discovery_run::start_run(&self.ctx.pool, source, &settings, item)
                .await
                .map_err(db_error)?
            {
                RunStart::Started { run_id, superseded } => (run_id, superseded),
                // The result commit finds the claim gone too and reports the lost lease.
                RunStart::ClaimLost => {
                    return Err(CrawlError::Transient(
                        "discover_catalog claim lost before starting a discovery run".into(),
                    ))
                }
                RunStart::Finalizing { run_id } => {
                    tracing::info!(%source, %run_id,
                        "a discovery run is finalizing; this discovery starts no new run");
                    return Ok(JobWrite::DiscoveryUnchanged);
                }
            };
        discovery_run::publish(source, &RunEvent::Started);
        if superseded > 0 {
            discovery_run::publish(source, &RunEvent::Superseded(superseded));
            tracing::info!(%source, %run_id, superseded, "discovery run superseded older runs");
        }
        let ctx = self.ctx.clone();
        let listing = guarded(async move { adapter.enumerate_discovery(&ctx).await }).await;
        match listing {
            Err(error) => {
                match discovery_run::fail_enumerating_run(
                    &self.ctx.pool,
                    run_id,
                    &error.to_string(),
                )
                .await
                {
                    Ok(true) => discovery_run::publish(source, &RunEvent::Failed),
                    Ok(false) => {}
                    Err(e) => {
                        tracing::warn!(%source, %run_id, error = %e, "marking discovery run failed failed")
                    }
                }
                Err(error)
            }
            Ok(Enumeration::Unchanged) => {
                tracing::info!(%source, %run_id, "catalog unchanged since last discovery");
                Ok(JobWrite::DiscoveryRun(RunWrite::Unchanged { run_id }))
            }
            Ok(Enumeration::Units {
                units,
                validator,
                curated_missing,
            }) => Ok(JobWrite::DiscoveryRun(RunWrite::Enumerated {
                run_id,
                units,
                units_per_chunk: settings.units_per_chunk.max(1),
                validator,
                curated_missing,
            })),
        }
    }

    /// A `discover_chunk` job: the details of one chunk of units, unless its run no longer
    /// waits for it.
    pub(super) async fn discover_chunk(&self, source: SourceId, item: &CrawlQueueItem) -> Fetched {
        let (run_id, chunk_index) = discovery_run::parse_chunk_series_id(&item.series_id)
            .ok_or_else(|| {
                CrawlError::Permanent(format!(
                    "discover_chunk job with malformed series_id {:?}",
                    item.series_id
                ))
            })?;
        let Some((adapter, settings)) = self.chunked_discovery(source) else {
            return Err(CrawlError::Permanent(format!(
                "{source} has no adapter that discovers in chunks"
            )));
        };
        let Some(chunk) = discovery_run::pending_chunk_units(&self.ctx.pool, run_id, chunk_index)
            .await
            .map_err(db_error)?
        else {
            tracing::info!(%source, %run_id, chunk_index, "discovery chunk no longer needed; skipping");
            return Ok(JobWrite::DiscoveryRun(RunWrite::Skipped));
        };
        // Chunks discovered under another selection must never finalize as this run's.
        if settings.selection_fingerprint != chunk.selection_fingerprint {
            return Err(CrawlError::Permanent(format!(
                "{source} catalog selection changed since discovery run {run_id} started; \
                 failing the run"
            )));
        }
        let units = chunk.units;
        let started = Instant::now();
        let ctx = self.ctx.clone();
        let result = guarded(async move { adapter.discover_units(&ctx, &units).await }).await?;
        let elapsed = started.elapsed();
        self.datasets.check_all(
            source,
            result
                .found
                .iter()
                .map(|d| (d.external_id.as_str(), &d.dataset)),
        )?;
        Ok(JobWrite::DiscoveryRun(RunWrite::Chunk {
            run_id,
            chunk_index,
            result,
            elapsed,
        }))
    }

    /// A `finalize_discovery` job: the run's chunks assembled into one discovery write, built
    /// exactly as a single-job discovery builds its own.
    pub(super) async fn finalize_run(&self, source: SourceId, item: &CrawlQueueItem) -> Fetched {
        let run_id = Uuid::from_str(&item.series_id).map_err(|_| {
            CrawlError::Permanent(format!(
                "finalize_discovery job with malformed series_id {:?}",
                item.series_id
            ))
        })?;
        let run = discovery_run::load_run(&self.ctx.pool, run_id)
            .await
            .map_err(db_error)?;
        let Some(run) = run.filter(|r| r.state == discovery_run::RunState::Finalizing.as_str())
        else {
            tracing::info!(%source, %run_id, "discovery run no longer finalizing; skipping");
            return Ok(JobWrite::DiscoveryRun(RunWrite::Skipped));
        };
        let Some((adapter, settings)) = self.chunked_discovery(source) else {
            return Err(CrawlError::Permanent(format!(
                "{source} no longer discovers in chunks; not finalizing run {run_id}"
            )));
        };
        if settings.selection_fingerprint != run.selection_fingerprint {
            return Err(CrawlError::Permanent(format!(
                "{source} catalog selection changed since discovery run {run_id} started; not \
                 finalizing it"
            )));
        }
        let (found, missing) = discovery_run::assemble(&self.ctx.pool, &run)
            .await
            .map_err(db_error)?;
        let validator = run.validator().map_err(db_error)?;
        let discovery =
            self.discovery_write(source, adapter.as_ref(), found, validator, missing)?;
        Ok(JobWrite::DiscoveryRun(RunWrite::Finalize {
            run_id,
            discovery: Box::new(discovery),
        }))
    }

    /// Inside the result transaction, before the built-in writes: applies a chunked-discovery
    /// write and returns what the built-in writes should do next. Finalize hands back its
    /// discovery write only once the run has moved to `complete`; every other run write is
    /// done here and hands back a no-op. Other results pass through unchanged.
    pub(super) async fn commit_discovery_run_conn(
        &self,
        conn: &mut AsyncPgConnection,
        source: SourceId,
        result: Fetched,
        events: &RunEvents,
    ) -> AppResult<Fetched> {
        let Ok(JobWrite::DiscoveryRun(write)) = result else {
            return Ok(result);
        };
        let max_retries = self.max_retries(source);
        let done = Ok(JobWrite::Handler(JobStats::default()));
        match write {
            RunWrite::Enumerated {
                run_id,
                units,
                units_per_chunk,
                validator,
                curated_missing,
            } => {
                match discovery_run::commit_enumeration_conn(
                    conn,
                    run_id,
                    &units,
                    units_per_chunk,
                    validator.as_ref(),
                    &curated_missing,
                    max_retries,
                )
                .await?
                {
                    Some(chunks) => tracing::info!(%source, %run_id, units = units.len(), chunks,
                        "discovery run enumerated; chunk jobs queued"),
                    None => tracing::info!(%source, %run_id,
                        "discovery run superseded during enumeration; discarding its units"),
                }
                Ok(done)
            }
            RunWrite::Unchanged { run_id } => {
                if let Some(duration) = discovery_run::commit_unchanged_conn(conn, run_id).await? {
                    events.push(RunEvent::Completed { duration });
                }
                Ok(Ok(JobWrite::DiscoveryUnchanged))
            }
            RunWrite::Chunk {
                run_id,
                chunk_index,
                result,
                elapsed,
            } => {
                match discovery_run::commit_chunk_conn(
                    conn,
                    run_id,
                    chunk_index,
                    &result,
                    max_retries,
                )
                .await?
                {
                    ChunkCommit::Stored => {
                        events.push(RunEvent::ChunkCompleted { duration: elapsed });
                    }
                    ChunkCommit::StoredLast => {
                        events.push(RunEvent::ChunkCompleted { duration: elapsed });
                        tracing::info!(%source, %run_id, "every discovery chunk stored; finalize queued");
                    }
                    ChunkCommit::Discarded => {
                        events.push(RunEvent::ChunkDiscarded);
                        tracing::info!(%source, %run_id, chunk_index,
                            "discovery run no longer running; chunk result discarded");
                    }
                }
                Ok(done)
            }
            RunWrite::Finalize { run_id, discovery } => {
                match discovery_run::complete_run_conn(conn, run_id).await? {
                    Some(duration) => {
                        events.push(RunEvent::Completed { duration });
                        tracing::info!(%source, %run_id, "discovery run complete; committing its catalog");
                        Ok(Ok(*discovery))
                    }
                    None => {
                        tracing::info!(%source, %run_id,
                            "discovery run no longer finalizing; nothing committed");
                        Ok(Ok(JobWrite::DiscoveryUnchanged))
                    }
                }
            }
            RunWrite::Skipped => Ok(Ok(JobWrite::DiscoveryUnchanged)),
        }
    }

    /// Inside the result transaction, after the queue transition: a chunk or finalize job that
    /// ended `failed` fails its run.
    pub(super) async fn record_discovery_run_outcome_conn(
        &self,
        conn: &mut AsyncPgConnection,
        source: SourceId,
        item: &CrawlQueueItem,
        outcome: &JobOutcome,
        events: &RunEvents,
    ) -> AppResult<()> {
        let JobOutcome::Failed { error } = outcome else {
            return Ok(());
        };
        let (run_id, chunk_index) = match item.job_kind() {
            JobKind::DiscoverChunk => match discovery_run::parse_chunk_series_id(&item.series_id) {
                Some((run, index)) => (run, Some(index)),
                None => return Ok(()),
            },
            JobKind::FinalizeDiscovery => match Uuid::from_str(&item.series_id) {
                Ok(run) => (run, None),
                Err(_) => return Ok(()),
            },
            _ => return Ok(()),
        };
        if chunk_index.is_some() {
            events.push(RunEvent::ChunkFailed);
        }
        if discovery_run::fail_run_conn(conn, source, run_id, chunk_index, error).await? {
            events.push(RunEvent::Failed);
            tracing::warn!(%source, %run_id, ?chunk_index, %error,
                "discovery run failed; it commits and retires nothing");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
