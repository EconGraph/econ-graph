// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Crawler status derived from `crawl_queue`.
//!
//! Workers keep no state outside the queue, so the queue *is* the crawler status: a job being
//! processed means a worker is alive, `finished_at` records when work last finished, and
//! `scheduled_for` says when the next job becomes due. [`crawler_status`] reads all of it in two
//! aggregate queries, using the database clock throughout. The two queries run inside one
//! read-only `REPEATABLE READ` transaction, so a job transitioning between them can't make the
//! global and per-source halves of the snapshot describe different queue states.

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
    /// Latest time a row became `failed` (by `finished_at`, else `updated_at`).
    pub last_failure: Option<DateTime<Utc>>,
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
    #[diesel(sql_type = Nullable<Timestamptz>)]
    last_failure: Option<DateTime<Utc>>,
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
    MAX(COALESCE(finished_at, updated_at)) FILTER (WHERE status = 'completed') AS last_success,
    MAX(COALESCE(finished_at, updated_at)) FILTER (WHERE status = 'failed') AS last_failure
FROM crawl_queue
GROUP BY source
ORDER BY source";

/// Computes the [`CrawlerStatusSnapshot`] from `crawl_queue`.
///
/// `GLOBAL_SQL` and `PER_SOURCE_SQL` run inside one read-only `REPEATABLE READ` transaction on
/// the same connection, so both see the same snapshot of the queue: an ordinary autocommit
/// (READ COMMITTED) pair of statements would let a job finish between the two reads, leaving the
/// global fields and the per-source counts describing different queue states.
pub async fn crawler_status(pool: &DatabasePool) -> AppResult<CrawlerStatusSnapshot> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabaseError(format!("failed to get database connection: {e}")))?;

    diesel::sql_query("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut conn)
        .await?;

    let outcome: AppResult<(GlobalRow, Vec<SourceRow>)> = async {
        let global: GlobalRow = diesel::sql_query(GLOBAL_SQL)
            .bind::<diesel::sql_types::Integer, _>(RECENT_ACTIVITY_MINUTES as i32)
            .get_result(&mut conn)
            .await?;
        let per_source = diesel::sql_query(PER_SOURCE_SQL)
            .load::<SourceRow>(&mut conn)
            .await?;
        Ok((global, per_source))
    }
    .await;

    // Always end the transaction we opened, whichever way the reads went, then propagate.
    diesel::sql_query(if outcome.is_ok() {
        "COMMIT"
    } else {
        "ROLLBACK"
    })
    .execute(&mut conn)
    .await?;
    let (global, per_source) = outcome?;

    Ok(CrawlerStatusSnapshot {
        active_workers: global.active_workers,
        is_running: global.is_running,
        last_crawl: global.last_crawl,
        next_scheduled_crawl: global.next_scheduled_crawl,
        per_source: per_source
            .into_iter()
            .map(|r| SourceQueueStatus {
                source: r.source,
                pending: r.pending,
                processing: r.processing,
                retrying: r.retrying,
                failed_24h: r.failed_24h,
                last_success: r.last_success,
                last_failure: r.last_failure,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    //! DB-backed; skipped when `DATABASE_URL` is unset. They empty `crawl_queue` and hold the
    //! crate-wide DB test lock ([`crate::testkit::lock_test_db`]) while they run.

    use super::*;
    use chrono::Duration;

    /// Pool plus the DB test lock guard; keep the guard alive for the whole test.
    async fn pool() -> Option<(DatabasePool, tokio::sync::MutexGuard<'static, ()>)> {
        let (url, guard) = crate::testkit::lock_test_db("status").await?;
        let pool = econ_graph_core::create_pool(&url).await.expect("pool");
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM crawl_queue")
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        Some((pool, guard))
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
        let Some((pool, _guard)) = pool().await else {
            return;
        };
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
        let Some((pool, _guard)) = pool().await else {
            return;
        };
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
        assert!(close(bls.last_failure, now - Duration::hours(2)));
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
        assert_eq!(fred.last_failure, None);
    }

    #[tokio::test]
    async fn status_idle_with_future_schedule() {
        let Some((pool, _guard)) = pool().await else {
            return;
        };
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
        let Some((pool, _guard)) = pool().await else {
            return;
        };
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

    /// Two connections: one holds `crawler_status`'s `REPEATABLE READ` transaction open between
    /// its global and per-source reads, the other commits a job transition in between. The
    /// snapshot must describe one consistent point in time, not a mix of before/after states.
    #[tokio::test]
    async fn status_snapshot_is_consistent_across_reads() {
        let Some((pool, _guard)) = pool().await else {
            return;
        };
        seed(&pool, "FRED", "A", "processing", Some("w1"), None, None).await;

        let mut conn_a = pool.get().await.unwrap();
        diesel::sql_query("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut conn_a)
            .await
            .unwrap();
        let global: GlobalRow = diesel::sql_query(GLOBAL_SQL)
            .bind::<diesel::sql_types::Integer, _>(RECENT_ACTIVITY_MINUTES as i32)
            .get_result(&mut conn_a)
            .await
            .unwrap();

        // Committed on a second connection while conn_a's transaction is still open. Under plain
        // autocommit statements (the bug this guards against) this would already be visible to
        // the per-source read below; under REPEATABLE READ it must not be.
        let mut conn_b = pool.get().await.unwrap();
        diesel::sql_query(
            "UPDATE crawl_queue SET status = 'completed', locked_by = NULL, locked_at = NULL, \
             finished_at = NOW() WHERE source = 'FRED' AND series_id = 'A'",
        )
        .execute(&mut conn_b)
        .await
        .unwrap();
        drop(conn_b);

        let per_source: Vec<SourceRow> = diesel::sql_query(PER_SOURCE_SQL)
            .load(&mut conn_a)
            .await
            .unwrap();
        diesel::sql_query("COMMIT")
            .execute(&mut conn_a)
            .await
            .unwrap();

        assert_eq!(
            global.active_workers, 1,
            "global read's own snapshot: the row was still processing"
        );
        let fred = per_source.iter().find(|r| r.source == "FRED").unwrap();
        assert_eq!(
            (fred.processing, fred.pending, fred.retrying),
            (1, 0, 0),
            "per-source read must agree with the global read's snapshot, not the concurrent \
             completion committed by conn_b between the two reads"
        );
        assert_eq!(
            fred.last_success, None,
            "the concurrent completion must not leak into this snapshot"
        );
    }
    /// Cancel the real status query after its transaction starts, then reuse the only pooled
    /// connection. A raw SQL BEGIN is invisible to Diesel's pool cleanup and leaks READ ONLY
    /// into the next borrow; a tracked transaction must discard that connection on cancellation.
    #[tokio::test]
    async fn cancelled_status_does_not_leak_transaction_to_pool() {
        use diesel::sql_types::Integer;
        use diesel_async::pooled_connection::{bb8::Pool, AsyncDieselConnectionManager};
        use diesel_async::AsyncPgConnection;
        use std::time::Duration as StdDuration;

        let Some((pool, _guard)) = pool().await else {
            return;
        };
        let url = std::env::var("DATABASE_URL").unwrap();
        let singleton = Pool::builder()
            .max_size(1)
            .connection_timeout(StdDuration::from_secs(10))
            .build(AsyncDieselConnectionManager::<AsyncPgConnection>::new(url))
            .await
            .unwrap();
        let mut status_conn = singleton.get().await.unwrap();
        let pid = diesel::select(diesel::dsl::sql::<Integer>("pg_backend_pid()"))
            .get_result::<i32>(&mut status_conn)
            .await
            .unwrap();
        drop(status_conn);

        // This lock blocks GLOBAL_SQL, so observing that query waiting proves that the real
        // crawler_status has finished BEGIN and is still inside its transaction.
        let mut locker = pool.get().await.unwrap();
        diesel::sql_query("BEGIN").execute(&mut locker).await.unwrap();
        diesel::sql_query("LOCK TABLE crawl_queue IN ACCESS EXCLUSIVE MODE")
            .execute(&mut locker)
            .await
            .unwrap();
        let mut observer = pool.get().await.unwrap();
        let query_pool = singleton.clone();
        let task = tokio::spawn(async move { crawler_status(&query_pool).await });
        let blocked_sql = format!(
            "EXISTS (SELECT 1 FROM pg_stat_activity WHERE pid = {pid} \
             AND state = 'active' AND wait_event_type = 'Lock' \
             AND xact_start IS NOT NULL AND query LIKE '%FROM crawl_queue%')"
        );
        let blocked = tokio::time::timeout(StdDuration::from_secs(10), async {
            loop {
                let waiting = diesel::select(diesel::dsl::sql::<Bool>(&blocked_sql))
                    .get_result::<bool>(&mut observer)
                    .await
                    .unwrap();
                if waiting {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;

        task.abort();
        let cancelled = tokio::time::timeout(StdDuration::from_secs(10), task)
            .await
            .expect("status task must stop after abort");
        // Release the table lock before any assertions or pool validation: a driver may finish
        // sending the cancelled SELECT, and the next SELECT 1 must not queue behind this lock.
        diesel::sql_query("ROLLBACK")
            .execute(&mut locker)
            .await
            .unwrap();
        blocked.expect("the production status query must reach the locked table");
        assert!(cancelled.unwrap_err().is_cancelled());

        let mut reused = tokio::time::timeout(StdDuration::from_secs(10), singleton.get())
            .await
            .expect("the singleton pool must remain usable")
            .unwrap();
        let read_only = diesel::select(diesel::dsl::sql::<Bool>(
            "current_setting('transaction_read_only')::boolean",
        ))
        .get_result::<bool>(&mut reused)
        .await
        .unwrap();
        let write = diesel::sql_query("UPDATE crawl_queue SET priority = priority WHERE FALSE")
            .execute(&mut reused)
            .await;
        assert!(
            !read_only,
            "cancelled crawler_status leaked its read-only transaction into the next pool borrow"
        );
        write.expect("a writer borrowing after cancelled crawler_status must succeed");
    }
}
