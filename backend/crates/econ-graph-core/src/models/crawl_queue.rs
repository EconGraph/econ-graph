//! Postgres-backed crawl job queue.
//!
//! State machine (column `status`):
//!
//! ```text
//!   enqueue ──► pending ──claim_next──► processing ──complete──► completed
//!                  ▲                        │  │
//!                  │ release_stuck          │  └──fail──► failed
//!                  └────────────────────────┤
//!                                           └──retry_later──► retrying ──claim_next──► processing
//!                                                 (or failed once retry_count reaches max_retries)
//! ```
//!
//! Invariants:
//! - `pending` / `retrying` rows never carry a lock; every transition out of `processing` clears
//!   `locked_by` / `locked_at` (the DB check constraint requires both or neither).
//! - At most one *active* (pending | processing | retrying) row exists per `(source, series_id, kind)`
//!   (partial unique index `uq_crawl_queue_active_item`); finished rows don't block re-enqueueing.
//! - Claiming is a single `UPDATE ... WHERE id = (SELECT ... FOR UPDATE SKIP LOCKED LIMIT 1)` statement,
//!   so two workers can never claim the same row.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Array, Bool, Double, Nullable, Text, Uuid as SqlUuid};
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::schema::crawl_queue;

/// Kind of work a queue item represents (column `crawl_queue.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// Fetch observations (and metadata) for one series.
    #[default]
    FetchSeries,
    /// Discover the catalog of series a source offers.
    DiscoverCatalog,
    /// Fetch one filing (SEC).
    FetchFiling,
}

impl JobKind {
    /// The exact string stored in `crawl_queue.kind`.
    pub fn as_str(&self) -> &'static str {
        match self {
            JobKind::FetchSeries => "fetch_series",
            JobKind::DiscoverCatalog => "discover_catalog",
            JobKind::FetchFiling => "fetch_filing",
        }
    }
}

impl fmt::Display for JobKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for JobKind {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "fetch_series" => Ok(JobKind::FetchSeries),
            "discover_catalog" => Ok(JobKind::DiscoverCatalog),
            "fetch_filing" => Ok(JobKind::FetchFiling),
            other => Err(AppError::Validation(format!("unknown job kind: {other}"))),
        }
    }
}

/// Outcome of [`CrawlQueueItem::retry_later`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueTransition {
    /// The item is `retrying` and becomes claimable again at `at`.
    Rescheduled { at: DateTime<Utc> },
    /// The attempt budget is exhausted; the item is now `failed`.
    Failed,
}

/// Crawl queue item for managing data collection jobs
#[derive(Debug, Clone, Queryable, QueryableByName, Selectable, Serialize, Deserialize)]
#[diesel(table_name = crawl_queue)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CrawlQueueItem {
    pub id: Uuid,
    pub source: String,
    pub series_id: String,
    pub priority: i32,
    pub status: String,
    pub retry_count: i32,
    pub max_retries: i32,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub scheduled_for: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub locked_at: Option<DateTime<Utc>>,
    /// One of `fetch_series` | `discover_catalog` | `fetch_filing`; see [`JobKind`].
    pub kind: String,
}

/// New crawl queue item for insertion
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = crawl_queue)]
pub struct NewCrawlQueueItem {
    #[validate(length(min = 1, max = 50))]
    pub source: String,
    #[validate(length(min = 1, max = 255))]
    pub series_id: String,
    #[validate(range(min = 1, max = 10))]
    pub priority: i32,
    #[validate(range(min = 0, max = 10))]
    pub max_retries: i32,
    pub scheduled_for: Option<DateTime<Utc>>,
    /// Job kind string (default `"fetch_series"`); see [`JobKind::as_str`].
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    JobKind::FetchSeries.as_str().to_string()
}

/// Crawl queue item update model.
///
/// Nullable columns use `Option<Option<T>>`: `None` leaves the column unchanged,
/// `Some(None)` sets it to NULL (e.g. to clear a lock), `Some(Some(v))` sets it to `v`.
#[derive(Debug, Clone, AsChangeset, Deserialize)]
#[diesel(table_name = crawl_queue)]
pub struct UpdateCrawlQueueItem {
    pub status: Option<String>,
    pub retry_count: Option<i32>,
    pub error_message: Option<Option<String>>,
    pub updated_at: DateTime<Utc>,
    pub scheduled_for: Option<Option<DateTime<Utc>>>,
    pub locked_by: Option<Option<String>>,
    pub locked_at: Option<Option<DateTime<Utc>>>,
}

impl UpdateCrawlQueueItem {
    /// Changeset fields that clear the worker lock.
    pub fn clearing_lock(mut self) -> Self {
        self.locked_by = Some(None);
        self.locked_at = Some(None);
        self
    }
}

/// Queue item status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueueStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Retrying,
    Cancelled,
}

impl std::fmt::Display for QueueStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueStatus::Pending => write!(f, "pending"),
            QueueStatus::Processing => write!(f, "processing"),
            QueueStatus::Completed => write!(f, "completed"),
            QueueStatus::Failed => write!(f, "failed"),
            QueueStatus::Retrying => write!(f, "retrying"),
            QueueStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl From<String> for QueueStatus {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "pending" => QueueStatus::Pending,
            "processing" => QueueStatus::Processing,
            "completed" => QueueStatus::Completed,
            "failed" => QueueStatus::Failed,
            "retrying" => QueueStatus::Retrying,
            "cancelled" => QueueStatus::Cancelled,
            _ => QueueStatus::Pending,
        }
    }
}

/// Priority levels for queue items
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueuePriority {
    Low = 1,
    Normal = 5,
    High = 8,
    Critical = 10,
}

impl From<i32> for QueuePriority {
    fn from(value: i32) -> Self {
        match value {
            1..=3 => QueuePriority::Low,
            4..=6 => QueuePriority::Normal,
            7..=9 => QueuePriority::High,
            _ => QueuePriority::Critical,
        }
    }
}

impl From<QueuePriority> for i32 {
    fn from(priority: QueuePriority) -> Self {
        priority as i32
    }
}

/// Queue statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatistics {
    pub total_items: i64,
    pub pending_items: i64,
    pub processing_items: i64,
    pub completed_items: i64,
    pub failed_items: i64,
    pub retrying_items: i64,
    pub oldest_pending: Option<DateTime<Utc>>,
    pub average_processing_time: Option<f64>, // in seconds
}

/// Queue item with processing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItemWithProcessingInfo {
    pub id: Uuid,
    pub source: String,
    pub series_id: String,
    pub priority: i32,
    pub status: String,
    pub retry_count: i32,
    pub max_retries: i32,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub scheduled_for: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub locked_at: Option<DateTime<Utc>>,
    pub processing_duration: Option<i64>, // in seconds
    pub time_since_created: i64,          // in seconds
}

/// Statuses that count as "active" for the uniqueness rule.
const ACTIVE_STATUSES: [&str; 3] = ["pending", "processing", "retrying"];

async fn get_conn(pool: &DatabasePool) -> AppResult<crate::database::PooledConn<'_>> {
    pool.get()
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to get database connection: {}", e)))
}

fn not_found(id: Uuid) -> AppError {
    AppError::NotFound(format!("crawl_queue item {id}"))
}

impl CrawlQueueItem {
    /// Parsed job kind (unknown strings fall back to `FetchSeries`; the DB check constraint prevents them).
    pub fn job_kind(&self) -> JobKind {
        self.kind.parse().unwrap_or_default()
    }

    /// Check if the item can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
            && matches!(
                QueueStatus::from(self.status.clone()),
                QueueStatus::Failed | QueueStatus::Retrying
            )
    }

    /// Check if the item is locked
    pub fn is_locked(&self) -> bool {
        self.locked_by.is_some() && self.locked_at.is_some()
    }

    /// Check if the item is ready for processing (mirrors the `claim_next` filter)
    pub fn is_ready_for_processing(&self) -> bool {
        matches!(
            QueueStatus::from(self.status.clone()),
            QueueStatus::Pending | QueueStatus::Retrying
        ) && !self.is_locked()
            && self
                .scheduled_for
                .is_none_or(|scheduled| scheduled <= Utc::now())
    }

    /// Calculate processing duration if locked
    pub fn processing_duration(&self) -> Option<i64> {
        self.locked_at
            .map(|locked_at| (Utc::now() - locked_at).num_seconds())
    }

    // ------------------------------------------------------------------
    // Queue API
    // ------------------------------------------------------------------

    /// Insert unless an active (pending|processing|retrying) row exists for
    /// `(source, series_id, kind)`. Returns `None` on conflict.
    pub async fn enqueue(pool: &DatabasePool, item: &NewCrawlQueueItem) -> AppResult<Option<Self>> {
        let mut conn = get_conn(pool).await?;
        let inserted = diesel::insert_into(crawl_queue::table)
            .values(item)
            .on_conflict_do_nothing()
            .get_result::<Self>(&mut conn)
            .await
            .optional()?;
        Ok(inserted)
    }

    /// Atomically claim the highest-priority due item (status pending|retrying, `scheduled_for`
    /// null or <= now) in a single statement. `sources` optionally restricts `crawl_queue.source`.
    pub async fn claim_next(
        pool: &DatabasePool,
        worker_id: &str,
        sources: Option<&[String]>,
    ) -> AppResult<Option<Self>> {
        let mut conn = get_conn(pool).await?;
        let item = diesel::sql_query(
            "UPDATE crawl_queue \
             SET status = 'processing', locked_by = $1, locked_at = NOW(), updated_at = NOW() \
             WHERE id = ( \
                 SELECT id FROM crawl_queue \
                 WHERE status IN ('pending', 'retrying') \
                   AND (scheduled_for IS NULL OR scheduled_for <= NOW()) \
                   AND ($2::text[] IS NULL OR source = ANY($2::text[])) \
                 ORDER BY priority DESC, created_at ASC \
                 FOR UPDATE SKIP LOCKED \
                 LIMIT 1 \
             ) \
             RETURNING *",
        )
        .bind::<Text, _>(worker_id)
        .bind::<Nullable<Array<Text>>, _>(sources)
        .get_result::<Self>(&mut conn)
        .await
        .optional()?;
        Ok(item)
    }

    /// Claim one specific item if it is claimable (pending|retrying). Returns `None` otherwise.
    /// Does not check `scheduled_for` (explicit claims override the schedule).
    pub async fn claim_by_id(
        pool: &DatabasePool,
        id: Uuid,
        worker_id: &str,
    ) -> AppResult<Option<Self>> {
        let mut conn = get_conn(pool).await?;
        let item = diesel::sql_query(
            "UPDATE crawl_queue \
             SET status = 'processing', locked_by = $2, locked_at = NOW(), updated_at = NOW() \
             WHERE id = $1 AND status IN ('pending', 'retrying') \
             RETURNING *",
        )
        .bind::<SqlUuid, _>(id)
        .bind::<Text, _>(worker_id)
        .get_result::<Self>(&mut conn)
        .await
        .optional()?;
        Ok(item)
    }

    /// Mark completed and clear the lock.
    pub async fn complete(pool: &DatabasePool, id: Uuid) -> AppResult<()> {
        Self::finish(pool, id, QueueStatus::Completed, None).await?;
        Ok(())
    }

    /// Reschedule: status `retrying`, lock cleared, `scheduled_for = now + delay`.
    /// If `count_attempt`, `retry_count += 1`, and if that reaches `max_retries` the item becomes
    /// `failed` instead. Rate limiting should pass `count_attempt = false`.
    /// Only applies to active items; errors with `NotFound` otherwise.
    pub async fn retry_later(
        pool: &DatabasePool,
        id: Uuid,
        error: &str,
        delay: Duration,
        count_attempt: bool,
    ) -> AppResult<QueueTransition> {
        let mut conn = get_conn(pool).await?;
        // All SET expressions see the pre-update row, so the decision is made atomically.
        let item = diesel::sql_query(
            "UPDATE crawl_queue SET \
                 retry_count = retry_count + CASE WHEN $2 THEN 1 ELSE 0 END, \
                 status = CASE WHEN $2 AND retry_count + 1 >= max_retries \
                               THEN 'failed' ELSE 'retrying' END, \
                 scheduled_for = CASE WHEN $2 AND retry_count + 1 >= max_retries \
                                      THEN scheduled_for \
                                      ELSE NOW() + make_interval(secs => $3) END, \
                 error_message = $4, \
                 locked_by = NULL, \
                 locked_at = NULL, \
                 updated_at = NOW() \
             WHERE id = $1 AND status IN ('pending', 'processing', 'retrying') \
             RETURNING *",
        )
        .bind::<SqlUuid, _>(id)
        .bind::<Bool, _>(count_attempt)
        .bind::<Double, _>(delay.as_secs_f64())
        .bind::<Text, _>(error)
        .get_result::<Self>(&mut conn)
        .await
        .optional()?
        .ok_or_else(|| not_found(id))?;

        if item.status == "failed" {
            Ok(QueueTransition::Failed)
        } else {
            let at = item.scheduled_for.ok_or_else(|| {
                AppError::InternalError("retry_later produced no scheduled_for".to_string())
            })?;
            Ok(QueueTransition::Rescheduled { at })
        }
    }

    /// Permanent failure: status `failed`, error recorded, lock cleared.
    pub async fn fail(pool: &DatabasePool, id: Uuid, error: &str) -> AppResult<()> {
        Self::finish(pool, id, QueueStatus::Failed, Some(error.to_string())).await?;
        Ok(())
    }

    /// Return items stuck in `processing` (locked longer than `older_than`, e.g. crashed workers)
    /// to `pending` with the lock cleared. Returns the number of items released.
    pub async fn release_stuck(pool: &DatabasePool, older_than: Duration) -> AppResult<i64> {
        let mut conn = get_conn(pool).await?;
        let n = diesel::sql_query(
            "UPDATE crawl_queue \
             SET status = 'pending', locked_by = NULL, locked_at = NULL, updated_at = NOW() \
             WHERE status = 'processing' \
               AND locked_at < NOW() - make_interval(secs => $1)",
        )
        .bind::<Double, _>(older_than.as_secs_f64())
        .execute(&mut conn)
        .await?;
        Ok(n as i64)
    }

    /// Put an active item back to `pending` and clear its lock. Returns false if the item
    /// doesn't exist or is already finished.
    pub async fn release(pool: &DatabasePool, id: Uuid) -> AppResult<bool> {
        use crate::schema::crawl_queue::dsl;
        let mut conn = get_conn(pool).await?;
        let update = UpdateCrawlQueueItem {
            status: Some(QueueStatus::Pending.to_string()),
            ..Default::default()
        }
        .clearing_lock();
        let n = diesel::update(
            dsl::crawl_queue
                .filter(dsl::id.eq(id))
                .filter(dsl::status.eq_any(ACTIVE_STATUSES)),
        )
        .set(&update)
        .execute(&mut conn)
        .await?;
        Ok(n > 0)
    }

    /// Move to a terminal status, clearing the lock. `error` (if any) replaces `error_message`.
    async fn finish(
        pool: &DatabasePool,
        id: Uuid,
        status: QueueStatus,
        error: Option<String>,
    ) -> AppResult<Self> {
        let update = UpdateCrawlQueueItem {
            status: Some(status.to_string()),
            error_message: error.map(Some),
            ..Default::default()
        }
        .clearing_lock();
        Self::update(pool, id, &update).await.map_err(|e| match e {
            AppError::Database(diesel::result::Error::NotFound) => not_found(id),
            other => other,
        })
    }

    // ------------------------------------------------------------------
    // Legacy helpers (kept for existing callers; implemented on the API above)
    // ------------------------------------------------------------------

    /// Plain insert (errors on an active duplicate). Prefer [`CrawlQueueItem::enqueue`].
    pub async fn create(pool: &DatabasePool, new_item: &NewCrawlQueueItem) -> AppResult<Self> {
        let mut conn = get_conn(pool).await?;
        let item = diesel::insert_into(crawl_queue::table)
            .values(new_item)
            .get_result::<Self>(&mut conn)
            .await?;
        Ok(item)
    }

    /// Alias for [`CrawlQueueItem::claim_next`] with no source filter.
    pub async fn get_next_for_processing(
        pool: &DatabasePool,
        worker_id: &str,
    ) -> AppResult<Option<Self>> {
        Self::claim_next(pool, worker_id, None).await
    }

    /// Update crawl queue item
    pub async fn update(
        pool: &DatabasePool,
        id: Uuid,
        update_data: &UpdateCrawlQueueItem,
    ) -> AppResult<Self> {
        use crate::schema::crawl_queue::dsl;
        let mut conn = get_conn(pool).await?;
        let item = diesel::update(dsl::crawl_queue.filter(dsl::id.eq(id)))
            .set(update_data)
            .get_result::<Self>(&mut conn)
            .await?;
        Ok(item)
    }

    /// Mark item as completed (see [`CrawlQueueItem::complete`])
    pub async fn mark_completed(pool: &DatabasePool, id: Uuid) -> AppResult<Self> {
        Self::finish(pool, id, QueueStatus::Completed, None).await
    }

    /// Mark item as failed (see [`CrawlQueueItem::fail`])
    pub async fn mark_failed(
        pool: &DatabasePool,
        id: Uuid,
        error_message: String,
    ) -> AppResult<Self> {
        Self::finish(pool, id, QueueStatus::Failed, Some(error_message)).await
    }
}

impl Default for NewCrawlQueueItem {
    fn default() -> Self {
        Self {
            source: String::new(),
            series_id: String::new(),
            priority: QueuePriority::Normal.into(),
            max_retries: 3,
            scheduled_for: None,
            kind: default_kind(),
        }
    }
}

impl Default for UpdateCrawlQueueItem {
    fn default() -> Self {
        Self {
            status: None,
            retry_count: None,
            error_message: None,
            updated_at: Utc::now(),
            scheduled_for: None,
            locked_by: None,
            locked_at: None,
        }
    }
}

#[cfg(test)]
mod _inline_tests {
    use super::*;

    #[test]
    fn test_queue_status_conversion() {
        // REQUIREMENT: The queue system should track job status for monitoring and retry logic
        // PURPOSE: Verify that status strings are correctly parsed into enum types
        // This ensures queue status updates from the database are properly handled

        // Test standard status parsing - required for queue processing
        assert_eq!(
            QueueStatus::from("pending".to_string()),
            QueueStatus::Pending
        );
        assert_eq!(
            QueueStatus::from("completed".to_string()),
            QueueStatus::Completed
        );

        // Test case-insensitive parsing - handles database variations
        assert_eq!(
            QueueStatus::from("PROCESSING".to_string()),
            QueueStatus::Processing
        );

        // Test unknown status defaults to Pending - safe fallback behavior
        assert_eq!(
            QueueStatus::from("unknown".to_string()),
            QueueStatus::Pending
        );
    }

    #[test]
    fn test_job_kind_round_trip() {
        for kind in [
            JobKind::FetchSeries,
            JobKind::DiscoverCatalog,
            JobKind::FetchFiling,
        ] {
            assert_eq!(kind.as_str().parse::<JobKind>().unwrap(), kind);
            assert_eq!(kind.to_string(), kind.as_str());
            assert_eq!(
                serde_json::to_string(&kind).unwrap(),
                format!("\"{}\"", kind.as_str())
            );
        }
        assert_eq!(
            "FETCH_SERIES".parse::<JobKind>().unwrap(),
            JobKind::FetchSeries
        );
        assert!("bogus".parse::<JobKind>().is_err());
        assert_eq!(NewCrawlQueueItem::default().kind, "fetch_series");
    }

    #[test]
    fn test_queue_priority_conversion() {
        // REQUIREMENT: The crawler should process high-priority items first
        // PURPOSE: Verify that priority values are correctly mapped to priority levels
        // This ensures critical data updates are processed before routine updates

        // Test priority level mapping - required for proper queue ordering
        assert_eq!(QueuePriority::from(1), QueuePriority::Low);
        assert_eq!(QueuePriority::from(5), QueuePriority::Normal);
        assert_eq!(QueuePriority::from(8), QueuePriority::High);
        assert_eq!(QueuePriority::from(10), QueuePriority::Critical);

        // Test reverse conversion for database storage
        assert_eq!(i32::from(QueuePriority::Normal), 5);
        assert_eq!(i32::from(QueuePriority::High), 8);
    }

    #[test]
    fn test_crawl_queue_item_methods() {
        // REQUIREMENT: The queue should use SKIP LOCKED for concurrent processing
        // PURPOSE: Verify that queue item state methods work correctly for lock management
        // This ensures multiple workers can process the queue without conflicts

        let mut item = CrawlQueueItem {
            id: Uuid::new_v4(),
            source: "FRED".to_string(),
            series_id: "GDP".to_string(),
            priority: 5,
            status: "failed".to_string(),
            retry_count: 1,
            max_retries: 3,
            error_message: Some("API error".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            scheduled_for: None,
            locked_by: None,
            locked_at: None,
            kind: "fetch_series".to_string(),
        };

        // Test retry logic - required for handling transient failures
        assert!(
            item.can_retry(),
            "Failed item should be retryable when under max retries"
        );
        assert!(
            !item.is_locked(),
            "Unlocked item should report as not locked"
        );
        assert!(
            !item.is_ready_for_processing(),
            "Failed item should not be ready for processing"
        );

        // Test pending status processing readiness
        item.status = "pending".to_string();
        assert!(
            item.is_ready_for_processing(),
            "Pending item should be ready for processing"
        );

        // Retrying items are claimable too
        item.status = "retrying".to_string();
        assert!(item.is_ready_for_processing());
        item.status = "pending".to_string();

        // Test locking mechanism - prevents concurrent processing of same item
        item.locked_by = Some("worker-1".to_string());
        item.locked_at = Some(Utc::now());
        assert!(item.is_locked(), "Locked item should report as locked");
        assert!(
            !item.is_ready_for_processing(),
            "Locked item should not be ready for processing"
        );
        assert_eq!(item.job_kind(), JobKind::FetchSeries);
    }

    #[test]
    fn test_new_crawl_queue_item_validation() {
        // REQUIREMENT: Queue items should be validated to prevent processing failures
        // PURPOSE: Verify that queue item validation prevents invalid crawl requests
        // This ensures crawlers receive valid data source and series identifiers

        let valid_item = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "GDP".to_string(),
            priority: 5,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        // Verify valid queue items pass validation
        assert!(
            valid_item.validate().is_ok(),
            "Valid queue item should pass validation"
        );

        // Test source validation - prevents crawler from attempting invalid sources
        let invalid_item = NewCrawlQueueItem {
            source: "".to_string(), // Empty source name
            series_id: "GDP".to_string(),
            ..valid_item.clone()
        };

        assert!(
            invalid_item.validate().is_err(),
            "Empty source should fail validation"
        );

        // Test priority validation - ensures priority values are within valid range
        let invalid_priority = NewCrawlQueueItem {
            priority: 0, // Below minimum priority
            ..valid_item.clone()
        };

        assert!(
            invalid_priority.validate().is_err(),
            "Invalid priority should fail validation"
        );
    }
}

#[cfg(test)]
mod tests;
