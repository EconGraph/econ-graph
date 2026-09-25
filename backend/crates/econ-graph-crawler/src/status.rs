// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Crawler status derived from `crawl_queue`.
//!
//! Workers keep no state outside the queue, so the queue *is* the crawler status: a job being
//! processed means a worker is alive, `finished_at` records when work last finished, and
//! `scheduled_for` says when the next job becomes due. [`crawler_status`] reads all of it in two
//! aggregate queries, using the database clock throughout.

use chrono::{DateTime, Utc};
use diesel::sql_types::{BigInt, Bool, Nullable, Text, Timestamptz};
use diesel::QueryableByName;
use diesel_async::RunQueryDsl;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::DatabasePool;
use serde::Serialize;

/// A worker counts as recently active if any job finished within this many minutes.
pub const RECENT_ACTIVITY_MINUTES: i64 = 10;

/// Snapshot of the crawler, computed from `crawl_queue`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CrawlerStatusSnapshot {
    /// Distinct `locked_by` values among `processing` rows (workers with a job in hand).
    pub active_workers: i64,
    /// Any row is `processing`, or any row finished within [`RECENT_ACTIVITY_MINUTES`].
    pub is_running: bool,
    /// Latest `finished_at` of a `completed` row.
    pub last_crawl: Option<DateTime<Utc>>,
    /// When the next `pending` / `retrying` job becomes due: `now` if one is already due (no
    /// `scheduled_for`, or `scheduled_for` in the past), otherwise the earliest `scheduled_for`.
    /// `None` if nothing is waiting.
    pub next_scheduled_crawl: Option<DateTime<Utc>>,
    /// Per-source counts, one entry per `crawl_queue.source` value present, ordered by source.
    pub per_source: Vec<SourceQueueStatus>,
}

/// Queue counts for one `crawl_queue.source` value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceQueueStatus {
    /// The stored source string (e.g. `FRED`); not necessarily a known [`crate::SourceId`].
    pub source: String,
    /// Rows in `pending`.
    pub pending: i64,
    /// Rows in `processing`.
    pub processing: i64,
    /// Rows in `retrying`.
    pub retrying: i64,
    /// Rows that became `failed` in the last 24 hours (by `finished_at`, else `updated_at`).
    pub failed_24h: i64,
    /// Latest completion time of a `completed` row.
    pub last_success: Option<DateTime<Utc>>,
}

#[derive(QueryableByName)]
struct GlobalRow {
    #[diesel(sql_type = BigInt)]
    active_workers: i64,
    #[diesel(sql_type = Bool)]
    is_running: bool,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    last_crawl: Option<DateTime<Utc>>,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    next_scheduled_crawl: Option<DateTime<Utc>>,
}

#[derive(QueryableByName)]
struct SourceRow {
    #[diesel(sql_type = Text)]
    source: String,
    #[diesel(sql_type = BigInt)]
    pending: i64,
    #[diesel(sql_type = BigInt)]
    processing: i64,
    #[diesel(sql_type = BigInt)]
    retrying: i64,
    #[diesel(sql_type = BigInt)]
    failed_24h: i64,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    last_success: Option<DateTime<Utc>>,
}

const GLOBAL_SQL: &str = "\
SELECT
    COUNT(DISTINCT locked_by) FILTER (WHERE status = 'processing') AS active_workers,
    (COUNT(*) FILTER (WHERE status = 'processing') > 0
     OR COALESCE(BOOL_OR(finished_at >= NOW() - make_interval(mins => $1::int)), FALSE)) AS is_running,
    MAX(COALESCE(finished_at, updated_at)) FILTER (WHERE status = 'completed') AS last_crawl,
    CASE
        WHEN COUNT(*) FILTER (WHERE status IN ('pending', 'retrying')) = 0 THEN NULL
        ELSE GREATEST(
            NOW(),
            MIN(COALESCE(scheduled_for, NOW())) FILTER (WHERE status IN ('pending', 'retrying'))
        )
    END AS next_scheduled_crawl
FROM crawl_queue";

const PER_SOURCE_SQL: &str = "\
SELECT
    source::text AS source,
    COUNT(*) FILTER (WHERE status = 'pending') AS pending,
    COUNT(*) FILTER (WHERE status = 'processing') AS processing,
    COUNT(*) FILTER (WHERE status = 'retrying') AS retrying,
    COUNT(*) FILTER (WHERE status = 'failed'
                     AND COALESCE(finished_at, updated_at) >= NOW() - INTERVAL '24 hours') AS failed_24h,
    MAX(COALESCE(finished_at, updated_at)) FILTER (WHERE status = 'completed') AS last_success
FROM crawl_queue
GROUP BY source
ORDER BY source";

/// Computes the [`CrawlerStatusSnapshot`] from `crawl_queue`.
pub async fn crawler_status(pool: &DatabasePool) -> AppResult<CrawlerStatusSnapshot> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabaseError(format!("failed to get database connection: {e}")))?;
    let global: GlobalRow = diesel::sql_query(GLOBAL_SQL)
        .bind::<diesel::sql_types::Integer, _>(RECENT_ACTIVITY_MINUTES as i32)
        .get_result(&mut conn)
        .await?;
    let per_source = diesel::sql_query(PER_SOURCE_SQL)
        .load::<SourceRow>(&mut conn)
        .await?
        .into_iter()
        .map(|r| SourceQueueStatus {
            source: r.source,
            pending: r.pending,
            processing: r.processing,
            retrying: r.retrying,
            failed_24h: r.failed_24h,
            last_success: r.last_success,
        })
        .collect();
    Ok(CrawlerStatusSnapshot {
        active_workers: global.active_workers,
        is_running: global.is_running,
        last_crawl: global.last_crawl,
        next_scheduled_crawl: global.next_scheduled_crawl,
        per_source,
    })
}

#[cfg(test)]
mod tests {
    //! DB-backed; skipped when `DATABASE_URL` is unset. They empty `crawl_queue`, so run the
    //! crate's tests with `--test-threads=1`.

    use super::*;
    use chrono::Duration;

    async fn pool() -> Option<DatabasePool> {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            eprintln!("DATABASE_URL not set; skipping DB-backed status test");
            return None;
        };
        econ_graph_core::run_migrations(&url)
            .await
            .expect("running migrations");
        let pool = econ_graph_core::create_pool(&url).await.expect("pool");
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM crawl_queue")
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        Some(pool)
    }

    /// Inserts a row with explicit status/lock/timestamps (bypasses the queue API on purpose).
    #[allow(clippy::too_many_arguments)]
    async fn seed(
        pool: &DatabasePool,
        source: &str,
        series: &str,
        status: &str,
        locked_by: Option<&str>,
        scheduled_for: Option<DateTime<Utc>>,
        finished_at: Option<DateTime<Utc>>,
    ) {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query(
            "INSERT INTO crawl_queue (source, series_id, priority, status, max_retries, locked_by, \
             locked_at, scheduled_for, finished_at, kind) \
             VALUES ($1, $2, 5, $3, 3, $4, CASE WHEN $4 IS NULL THEN NULL ELSE NOW() END, $5, $6, \
             'fetch_series')",
        )
        .bind::<Text, _>(source)
        .bind::<Text, _>(series)
        .bind::<Text, _>(status)
        .bind::<Nullable<Text>, _>(locked_by)
        .bind::<Nullable<Timestamptz>, _>(scheduled_for)
        .bind::<Nullable<Timestamptz>, _>(finished_at)
        .execute(&mut conn)
        .await
        .unwrap();
    }

    fn close(a: Option<DateTime<Utc>>, b: DateTime<Utc>) -> bool {
        a.is_some_and(|a| (a - b).num_seconds().abs() <= 2)
    }

    #[tokio::test]
    async fn status_empty_queue() {
        let Some(pool) = pool().await else { return };
        let s = crawler_status(&pool).await.unwrap();
        assert_eq!(
            s,
            CrawlerStatusSnapshot {
                active_workers: 0,
                is_running: false,
                last_crawl: None,
                next_scheduled_crawl: None,
                per_source: vec![],
            }
        );
    }

    #[tokio::test]
    async fn status_from_seeded_rows() {
        let Some(pool) = pool().await else { return };
        let now = Utc::now();
        let done_old = now - Duration::hours(3);
        let done_recent = now - Duration::hours(1);
        // FRED: two processing rows on the same worker, one pending unscheduled, one completed.
        seed(&pool, "FRED", "A", "processing", Some("w1"), None, None).await;
        seed(&pool, "FRED", "B", "processing", Some("w1"), None, None).await;
        seed(&pool, "FRED", "C", "pending", None, None, None).await;
        seed(&pool, "FRED", "D", "completed", None, None, Some(done_old)).await;
        // BLS: processing on a second worker, retrying in the future, failures old and recent.
        seed(&pool, "BLS", "E", "processing", Some("w2"), None, None).await;
        let later = now + Duration::minutes(30);
        seed(&pool, "BLS", "F", "retrying", None, Some(later), None).await;
        seed(
            &pool,
            "BLS",
            "G",
            "failed",
            None,
            None,
            Some(now - Duration::hours(2)),
        )
        .await;
        seed(
            &pool,
            "BLS",
            "H",
            "failed",
            None,
            None,
            Some(now - Duration::hours(48)),
        )
        .await;
        seed(
            &pool,
            "BLS",
            "I",
            "completed",
            None,
            None,
            Some(done_recent),
        )
        .await;

        let s = crawler_status(&pool).await.unwrap();
        assert_eq!(s.active_workers, 2);
        assert!(s.is_running);
        assert!(close(s.last_crawl, done_recent), "{:?}", s.last_crawl);
        // FRED C is pending without a schedule, so the next crawl is due now.
        assert!(
            close(s.next_scheduled_crawl, now),
            "{:?}",
            s.next_scheduled_crawl
        );

        assert_eq!(s.per_source.len(), 2);
        let bls = &s.per_source[0];
        assert_eq!(bls.source, "BLS");
        assert_eq!(
            (bls.pending, bls.processing, bls.retrying, bls.failed_24h),
            (0, 1, 1, 1)
        );
        assert!(close(bls.last_success, done_recent));
        let fred = &s.per_source[1];
        assert_eq!(fred.source, "FRED");
        assert_eq!(
            (
                fred.pending,
                fred.processing,
                fred.retrying,
                fred.failed_24h
            ),
            (1, 2, 0, 0)
        );
        assert!(close(fred.last_success, done_old));
    }

    #[tokio::test]
    async fn status_idle_with_future_schedule() {
        let Some(pool) = pool().await else { return };
        let now = Utc::now();
        let later = now + Duration::minutes(45);
        seed(&pool, "FRED", "A", "retrying", None, Some(later), None).await;
        seed(
            &pool,
            "FRED",
            "B",
            "completed",
            None,
            None,
            Some(now - Duration::hours(1)),
        )
        .await;

        let s = crawler_status(&pool).await.unwrap();
        assert_eq!(s.active_workers, 0);
        assert!(
            !s.is_running,
            "nothing processing and nothing finished recently"
        );
        assert!(close(s.next_scheduled_crawl, later));
    }

    #[tokio::test]
    async fn status_running_when_recently_finished() {
        let Some(pool) = pool().await else { return };
        let now = Utc::now();
        seed(
            &pool,
            "BLS",
            "A",
            "failed",
            None,
            None,
            Some(now - Duration::minutes(3)),
        )
        .await;

        let s = crawler_status(&pool).await.unwrap();
        assert!(s.is_running);
        assert_eq!(s.active_workers, 0);
        assert_eq!(s.last_crawl, None, "failed rows don't count as a crawl");
        assert_eq!(s.next_scheduled_crawl, None);
    }
}
