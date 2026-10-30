// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Forced full-history re-fetches: one `refetch_series` queue job per series of a source, and
//! the per-source progress of those jobs.
//!
//! A `refetch_series` job is processed like `fetch_series` (same queue, same worker, same
//! per-source rate limits and retries, same persistence), except that the worker passes no
//! `since` and no known vintage and sets [`CrawlCtx::bypass_validators`](crate::CrawlCtx), so the
//! adapter reads the series' whole history from the source even when its stored validators
//! (`Last-Modified`, content hashes, source versions such as FRED's `last_updated`) say nothing
//! changed.
//!
//! The re-fetch never deletes anything. Points are upserted on the `data_points` key `(series_id,
//! date, revision_date, is_original_release)` like any fetch, so:
//!
//! - series keep their ids (the stable id of `(source, external_id)`), metadata the fetch doesn't
//!   carry, annotations and relationships;
//! - a stored point whose value differs from the source's now (for example one rounded by an
//!   older column type) is rewritten in place, and an identical one is left untouched;
//! - a vintage the source still serves is rewritten under its own revision date, and a vintage
//!   it no longer serves (World Bank database releases before the current one, for example) keeps
//!   its stored value: it is never replaced by the current estimate.
//!
//! Which series: every series the refresh scheduler refreshes for the source (active
//! `economic_series` rows, and active discovered `series_metadata` rows not fetched yet), with
//! the scheduler's own exclusions (the `legacy` dataset, Census ids outside
//! [`fetchable_id_regex`](crate::sources::census::fetchable_id_regex)).
//!
//! Enqueuing is idempotent: a series with an active (pending, processing or retrying)
//! `refetch_series` job is skipped, and with `completed_since` so is one whose re-fetch already
//! completed since then. Re-running the same command after an interruption, a worker restart or
//! failures therefore enqueues only what is left, failed series included.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Array, BigInt, Bool, Integer, Nullable, Text, Timestamptz};
use diesel_async::RunQueryDsl;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::JobKind;
use econ_graph_core::DatabasePool;
use serde::Serialize;

use crate::persist::data_source_template;
use crate::policy::SourcePolicy;
use crate::source::SourceId;

/// Default queue priority of `refetch_series` jobs: below scheduled refreshes
/// ([`DEFAULT_PRIORITY`](crate::cli::DEFAULT_PRIORITY)), so a re-fetch campaign drains in the
/// background without delaying them.
pub const DEFAULT_REFETCH_PRIORITY: i32 = 3;

/// What to enqueue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefetchOptions {
    /// Queue priority, 1 (lowest) to 10 (highest).
    pub priority: i32,
    /// Also re-fetch series marked inactive (retired by their source's catalog).
    pub include_inactive: bool,
    /// Skip series whose `refetch_series` job completed at or after this time.
    pub completed_since: Option<DateTime<Utc>>,
}

impl Default for RefetchOptions {
    fn default() -> Self {
        Self {
            priority: DEFAULT_REFETCH_PRIORITY,
            include_inactive: false,
            completed_since: None,
        }
    }
}

/// Series counts of one source for [`plan`] / [`enqueue`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SourcePlan {
    /// Source code, e.g. `FRED`.
    pub source: String,
    /// Series selected for re-fetch.
    pub series: i64,
    /// Of those, series that already have an active `refetch_series` job.
    pub already_queued: i64,
    /// Of those, series whose re-fetch completed since `completed_since` (none without it).
    pub already_completed: i64,
    /// Jobs enqueued by this call (0 for [`plan`]).
    pub enqueued: i64,
}

impl SourcePlan {
    /// Series that [`enqueue`] would enqueue (or did).
    pub fn to_enqueue(&self) -> i64 {
        self.series - self.already_queued - self.already_completed
    }
}

/// Per-source progress of `refetch_series` jobs: for each series, the state of its latest job.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SourceProgress {
    /// Source code.
    pub source: String,
    /// Waiting to be claimed.
    pub pending: i64,
    /// Being fetched now.
    pub processing: i64,
    /// Waiting to retry after a transient failure or a rate limit.
    pub retrying: i64,
    /// Re-fetched and written.
    pub completed: i64,
    /// Failed for good (not found, auth, parse or permanent errors, or retries exhausted).
    pub failed: i64,
    /// Cancelled by an operator.
    pub cancelled: i64,
}

impl SourceProgress {
    /// Series with a job in any state.
    pub fn total(&self) -> i64 {
        self.pending
            + self.processing
            + self.retrying
            + self.completed
            + self.failed
            + self.cancelled
    }

    /// Whether no job of the source is still waiting or running.
    pub fn finished(&self) -> bool {
        self.pending + self.processing + self.retrying == 0
    }
}

/// One failed re-fetch, for [`failures`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, QueryableByName)]
pub struct RefetchFailure {
    /// Source code.
    #[diesel(sql_type = Text)]
    pub source: String,
    /// External series id.
    #[diesel(sql_type = Text)]
    pub series_id: String,
    /// The error recorded on the job.
    #[diesel(sql_type = Nullable<Text>)]
    pub error_message: Option<String>,
}

/// The series selected for `$1` (data_sources name), `$2` (Census id pattern, or empty),
/// `$3` (include inactive), `$4` (source code), `$5` (completed since), each with whether it
/// already has an active job and whether its re-fetch completed since `$5`.
const CLASSIFIED_SQL: &str = "\
    WITH cand AS ( \
        SELECT es.external_id::text AS external_id \
        FROM economic_series es \
        JOIN data_sources ds ON ds.id = es.source_id \
        WHERE ds.name = $1 AND ($3 OR es.is_active) \
          AND ($2 = '' OR es.external_id ~ $2) \
          AND NOT EXISTS (SELECT 1 FROM datasets d \
                          WHERE d.id = es.dataset_id AND d.code = 'legacy') \
        UNION \
        SELECT sm.external_id::text \
        FROM series_metadata sm \
        JOIN data_sources ds ON ds.id = sm.source_id \
        WHERE ds.name = $1 AND sm.is_active \
          AND ($2 = '' OR sm.external_id ~ $2) \
          AND NOT EXISTS (SELECT 1 FROM economic_series es \
                          WHERE es.source_id = sm.source_id \
                            AND es.external_id = sm.external_id) \
    ), classified AS ( \
        SELECT c.external_id, \
               EXISTS (SELECT 1 FROM crawl_queue q \
                       WHERE q.source = $4 AND q.series_id = c.external_id \
                         AND q.kind = 'refetch_series' \
                         AND q.status IN ('pending', 'processing', 'retrying')) AS queued, \
               ($5::timestamptz IS NOT NULL AND EXISTS ( \
                       SELECT 1 FROM crawl_queue q \
                       WHERE q.source = $4 AND q.series_id = c.external_id \
                         AND q.kind = 'refetch_series' AND q.status = 'completed' \
                         AND q.finished_at >= $5)) AS done \
        FROM cand c \
    )";

#[derive(QueryableByName)]
struct Counts {
    #[diesel(sql_type = BigInt)]
    series: i64,
    #[diesel(sql_type = BigInt)]
    queued: i64,
    #[diesel(sql_type = BigInt)]
    done: i64,
}

#[derive(QueryableByName)]
struct Inserted {
    #[diesel(sql_type = BigInt)]
    n: i64,
}

#[derive(QueryableByName)]
struct StatusRow {
    #[diesel(sql_type = Text)]
    source: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = BigInt)]
    n: i64,
}

fn conn_err(e: impl std::fmt::Display) -> AppError {
    AppError::DatabaseError(format!("Failed to get database connection: {e}"))
}

/// The Census id pattern for `source`, or empty (no filter).
fn id_pattern(source: SourceId) -> String {
    if source == SourceId::Census {
        crate::sources::census::fetchable_id_regex()
    } else {
        String::new()
    }
}

async fn counts(
    conn: &mut diesel_async::AsyncPgConnection,
    source: SourceId,
    options: &RefetchOptions,
) -> AppResult<SourcePlan> {
    let row: Counts = diesel::sql_query(format!(
        "{CLASSIFIED_SQL} \
         SELECT count(*) AS series, \
                count(*) FILTER (WHERE queued) AS queued, \
                count(*) FILTER (WHERE done AND NOT queued) AS done \
         FROM classified"
    ))
    .bind::<Text, _>(data_source_template(source).name)
    .bind::<Text, _>(id_pattern(source))
    .bind::<Bool, _>(options.include_inactive)
    .bind::<Text, _>(source.as_str())
    .bind::<Nullable<Timestamptz>, _>(options.completed_since)
    .get_result(conn)
    .await?;
    Ok(SourcePlan {
        source: source.as_str().to_string(),
        series: row.series,
        already_queued: row.queued,
        already_completed: row.done,
        enqueued: 0,
    })
}

/// What [`enqueue`] would do for each of `sources`, without writing anything.
pub async fn plan(
    pool: &DatabasePool,
    sources: &[SourceId],
    options: &RefetchOptions,
) -> AppResult<Vec<SourcePlan>> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    let mut out = Vec::with_capacity(sources.len());
    for &source in sources {
        out.push(counts(&mut conn, source, options).await?);
    }
    Ok(out)
}

/// Enqueues one `refetch_series` job per selected series of each of `sources` that has no
/// active one (and, with `completed_since`, didn't complete since then). Returns the counts,
/// with `enqueued` set.
///
/// Each source's jobs are inserted in external-id order with increasing `created_at`, so the
/// worker claims them in that order and batching adapters (World Bank by indicator, FHFA)
/// find their batch mates next to each other.
pub async fn enqueue(
    pool: &DatabasePool,
    sources: &[SourceId],
    options: &RefetchOptions,
) -> AppResult<Vec<SourcePlan>> {
    if !(1..=10).contains(&options.priority) {
        return Err(AppError::Validation(format!(
            "priority {} is not between 1 and 10",
            options.priority
        )));
    }
    let mut conn = pool.get().await.map_err(conn_err)?;
    let mut out = Vec::with_capacity(sources.len());
    for &source in sources {
        let mut plan = counts(&mut conn, source, options).await?;
        let max_retries = SourcePolicy::default_for(source).max_retries.min(10) as i32;
        let inserted: Inserted = diesel::sql_query(format!(
            "{CLASSIFIED_SQL}, \
             todo AS ( \
                 SELECT external_id, row_number() OVER (ORDER BY external_id) AS rn \
                 FROM classified WHERE NOT queued AND NOT done \
             ), ins AS ( \
                 INSERT INTO crawl_queue \
                     (source, series_id, priority, max_retries, kind, created_at, updated_at) \
                 SELECT $4, t.external_id, $6, $7, $8, \
                        NOW() + t.rn * INTERVAL '1 microsecond', NOW() \
                 FROM todo t \
                 ORDER BY t.rn \
                 ON CONFLICT DO NOTHING \
                 RETURNING 1 \
             ) \
             SELECT count(*) AS n FROM ins"
        ))
        .bind::<Text, _>(data_source_template(source).name)
        .bind::<Text, _>(id_pattern(source))
        .bind::<Bool, _>(options.include_inactive)
        .bind::<Text, _>(source.as_str())
        .bind::<Nullable<Timestamptz>, _>(options.completed_since)
        .bind::<Integer, _>(options.priority)
        .bind::<Integer, _>(max_retries)
        .bind::<Text, _>(JobKind::RefetchSeries.as_str())
        .get_result(&mut conn)
        .await?;
        plan.enqueued = inserted.n;
        out.push(plan);
    }
    Ok(out)
}

/// Progress of the `refetch_series` jobs of each of `sources` created at or after `since`
/// (all of them without it), counting each series once, by its latest job. Finished jobs are
/// purged from the queue after the worker's retention (14 days by default), so pass a `since`
/// within it.
pub async fn progress(
    pool: &DatabasePool,
    sources: &[SourceId],
    since: Option<DateTime<Utc>>,
) -> AppResult<Vec<SourceProgress>> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    let codes: Vec<String> = sources.iter().map(|s| s.as_str().to_string()).collect();
    let rows: Vec<StatusRow> = diesel::sql_query(
        "SELECT source, status, count(*) AS n FROM ( \
             SELECT DISTINCT ON (source, series_id) source, series_id, status \
             FROM crawl_queue \
             WHERE kind = $1 AND source = ANY($2) \
               AND ($3::timestamptz IS NULL OR created_at >= $3) \
             ORDER BY source, series_id, created_at DESC \
         ) latest \
         GROUP BY source, status",
    )
    .bind::<Text, _>(JobKind::RefetchSeries.as_str())
    .bind::<Array<Text>, _>(&codes)
    .bind::<Nullable<Timestamptz>, _>(since)
    .load(&mut conn)
    .await?;
    let mut by_source: BTreeMap<String, SourceProgress> = codes
        .iter()
        .map(|c| {
            (
                c.clone(),
                SourceProgress {
                    source: c.clone(),
                    ..SourceProgress::default()
                },
            )
        })
        .collect();
    for row in rows {
        let Some(p) = by_source.get_mut(&row.source) else {
            continue;
        };
        match row.status.as_str() {
            "pending" => p.pending += row.n,
            "processing" => p.processing += row.n,
            "retrying" => p.retrying += row.n,
            "completed" => p.completed += row.n,
            "failed" => p.failed += row.n,
            _ => p.cancelled += row.n,
        }
    }
    Ok(by_source.into_values().collect())
}

/// Up to `limit` series of `sources` whose latest `refetch_series` job (created at or after
/// `since`) failed, with the recorded error, ordered by source and series id.
pub async fn failures(
    pool: &DatabasePool,
    sources: &[SourceId],
    since: Option<DateTime<Utc>>,
    limit: i64,
) -> AppResult<Vec<RefetchFailure>> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    let codes: Vec<String> = sources.iter().map(|s| s.as_str().to_string()).collect();
    Ok(diesel::sql_query(
        "SELECT source, series_id, error_message FROM ( \
             SELECT DISTINCT ON (source, series_id) source, series_id, status, error_message \
             FROM crawl_queue \
             WHERE kind = $1 AND source = ANY($2) \
               AND ($3::timestamptz IS NULL OR created_at >= $3) \
             ORDER BY source, series_id, created_at DESC \
         ) latest \
         WHERE status = 'failed' \
         ORDER BY source, series_id \
         LIMIT $4",
    )
    .bind::<Text, _>(JobKind::RefetchSeries.as_str())
    .bind::<Array<Text>, _>(&codes)
    .bind::<Nullable<Timestamptz>, _>(since)
    .bind::<BigInt, _>(limit)
    .load(&mut conn)
    .await?)
}

#[cfg(test)]
mod tests;
