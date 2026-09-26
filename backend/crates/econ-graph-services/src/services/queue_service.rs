//! Queue service: legacy convenience wrappers over the crawl queue model.
//!
//! Every state transition here delegates to the queue API on
//! [`CrawlQueueItem`], so there is one code path for queue semantics.
//!
//! - Worker paths finish the item they claimed: [`complete_claimed_item`],
//!   [`fail_claimed_item`] and [`retry_claimed_item_later`] take the claimed row and are
//!   lease-guarded (see `CrawlQueueItem::{complete, fail, retry_later}`), so a worker whose lease
//!   expired cannot overwrite a newer claim of the same item.
//! - The id-based transitions (`mark_item_completed`, `mark_item_failed`,
//!   `update_queue_item_status`, `update_queue_item_for_retry`) are admin overrides with no worker
//!   identity: they use the *unguarded* `force_*` variants and ignore which claim holds the item.

use chrono::{DateTime, Duration, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

use econ_graph_core::{
    database::DatabasePool,
    error::{AppError, AppResult},
    models::{
        CrawlQueueItem, LeaseOutcome, QueueStatistics, QueueStatus, QueueTransition,
        UpdateCrawlQueueItem,
    },
    schema::crawl_queue,
};

/// Worker id used when items are claimed through [`get_next_queue_items`].
pub const QUEUE_SERVICE_WORKER_ID: &str = "queue-service";

/// Claim up to `limit` due items (highest priority first) for processing.
///
/// Each returned item has been atomically moved to `processing` and locked by
/// [`QUEUE_SERVICE_WORKER_ID`] (see [`CrawlQueueItem::claim_next`]); the caller owns it and must
/// finish it by passing the returned item to [`complete_claimed_item`], [`fail_claimed_item`] or
/// [`retry_claimed_item_later`].
pub async fn get_next_queue_items(
    pool: &DatabasePool,
    limit: i64,
) -> AppResult<Vec<CrawlQueueItem>> {
    let mut items = Vec::new();
    while (items.len() as i64) < limit {
        match CrawlQueueItem::claim_next(pool, QUEUE_SERVICE_WORKER_ID, None).await? {
            Some(item) => items.push(item),
            None => break,
        }
    }
    Ok(items)
}

/// Lock a specific queue item for processing by a worker and return the claim, which the worker
/// finishes with [`complete_claimed_item`], [`fail_claimed_item`] or [`retry_claimed_item_later`].
///
/// Fails with `Conflict` if the item doesn't exist or isn't claimable (pending/retrying).
pub async fn lock_queue_item(
    pool: &DatabasePool,
    item_id: Uuid,
    worker_id: &str,
) -> AppResult<CrawlQueueItem> {
    match CrawlQueueItem::claim_by_id(pool, item_id, worker_id).await? {
        Some(claim) => Ok(claim),
        None => Err(AppError::Conflict(format!(
            "crawl_queue item {item_id} is not claimable"
        ))),
    }
}

/// Update queue item status with optional error message (admin override: ignores which worker
/// holds the lock). Every accepted status releases the worker lock.
///
/// `Processing` is rejected with `Validation`: a `processing` row must be owned by a worker that
/// will finish it (the DB requires `locked_by`/`locked_at` together, and a lock nobody holds
/// would just sit there until `release_stuck` burns one of the item's attempts). Use
/// [`lock_queue_item`] (or `CrawlQueueItem::claim_by_id`) to take ownership instead.
pub async fn update_queue_item_status(
    pool: &DatabasePool,
    item_id: Uuid,
    status: QueueStatus,
    error_message: Option<String>,
) -> AppResult<()> {
    match status {
        QueueStatus::Completed => {
            CrawlQueueItem::force_complete(pool, item_id).await?;
            Ok(())
        }
        QueueStatus::Failed => {
            CrawlQueueItem::force_fail(pool, item_id, error_message.as_deref().unwrap_or("failed"))
                .await?;
            Ok(())
        }
        QueueStatus::Processing => Err(AppError::Validation(format!(
            "cannot set crawl_queue item {item_id} to processing without a worker lock; \
             use lock_queue_item"
        ))),
        QueueStatus::Pending | QueueStatus::Retrying | QueueStatus::Cancelled => {
            let update = UpdateCrawlQueueItem {
                status: Some(status.to_string()),
                error_message: error_message.map(Some),
                // finished_at is NULL while an item is active; cancelled is terminal.
                finished_at: Some(match status {
                    QueueStatus::Cancelled => Some(Utc::now()),
                    _ => None,
                }),
                ..Default::default()
            }
            .clearing_lock();
            CrawlQueueItem::update(pool, item_id, &update).await?;
            Ok(())
        }
    }
}

/// Backoff used by [`update_queue_item_for_retry`]: 2^attempt minutes, capped at 60.
pub fn retry_backoff(attempt: i32) -> std::time::Duration {
    let minutes = 2_u64.saturating_pow(attempt.max(0) as u32).min(60);
    std::time::Duration::from_secs(minutes * 60)
}

/// Record a failed attempt and reschedule with exponential backoff (2^n minutes, max 60),
/// or mark the item failed once `max_retries` is reached. Releases the lock.
/// Admin override: applies to any active item regardless of lock owner
/// (see `CrawlQueueItem::force_retry_later`).
pub async fn update_queue_item_for_retry(
    pool: &DatabasePool,
    item_id: Uuid,
    error_message: Option<String>,
) -> AppResult<()> {
    use crawl_queue::dsl;

    let current_retry_count: i32 = {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;
        dsl::crawl_queue
            .filter(dsl::id.eq(item_id))
            .select(dsl::retry_count)
            .first(&mut conn)
            .await?
    };

    CrawlQueueItem::force_retry_later(
        pool,
        item_id,
        error_message.as_deref().unwrap_or("retry"),
        retry_backoff(current_retry_count + 1),
        true,
    )
    .await?;
    Ok(())
}

/// Mark a claimed item completed, if `claim` (from [`get_next_queue_items`],
/// [`get_and_lock_next_item`] or [`lock_queue_item`]) is still the item's current claim.
pub async fn complete_claimed_item(
    pool: &DatabasePool,
    claim: &CrawlQueueItem,
) -> AppResult<LeaseOutcome> {
    CrawlQueueItem::complete(pool, claim).await
}

/// Mark a claimed item permanently failed, if `claim` is still the item's current claim.
pub async fn fail_claimed_item(
    pool: &DatabasePool,
    claim: &CrawlQueueItem,
    error_message: &str,
) -> AppResult<LeaseOutcome> {
    CrawlQueueItem::fail(pool, claim, error_message).await
}

/// Record a failed attempt on a claimed item and reschedule it with the same exponential backoff
/// as [`update_queue_item_for_retry`], or fail it once `max_retries` is reached. Returns
/// [`QueueTransition::LostLease`] (changing nothing) if `claim` is no longer the current claim.
pub async fn retry_claimed_item_later(
    pool: &DatabasePool,
    claim: &CrawlQueueItem,
    error_message: &str,
) -> AppResult<QueueTransition> {
    CrawlQueueItem::retry_later(
        pool,
        claim,
        error_message,
        retry_backoff(claim.retry_count + 1),
        true,
    )
    .await
}

/// Unlock a queue item (release worker lock) and put it back to `pending`.
/// Finished (completed/failed/cancelled) or missing items are left untouched.
pub async fn unlock_queue_item(pool: &DatabasePool, item_id: Uuid) -> AppResult<()> {
    CrawlQueueItem::release(pool, item_id).await?;
    Ok(())
}

/// Get comprehensive queue statistics for monitoring
pub async fn get_queue_statistics(pool: &DatabasePool) -> AppResult<QueueStatistics> {
    use crawl_queue::dsl;
    use diesel::dsl::{count, min};

    let mut conn = pool.get().await.map_err(|e| {
        econ_graph_core::error::AppError::DatabaseError(format!(
            "Failed to get database connection: {}",
            e
        ))
    })?;

    // Get total count
    let total_items: i64 = dsl::crawl_queue
        .select(count(dsl::id))
        .first(&mut conn)
        .await?;

    // Get counts by status
    let pending_items: i64 = dsl::crawl_queue
        .filter(dsl::status.eq("pending"))
        .select(count(dsl::id))
        .first(&mut conn)
        .await?;

    let processing_items: i64 = dsl::crawl_queue
        .filter(dsl::status.eq("processing"))
        .select(count(dsl::id))
        .first(&mut conn)
        .await?;

    let completed_items: i64 = dsl::crawl_queue
        .filter(dsl::status.eq("completed"))
        .select(count(dsl::id))
        .first(&mut conn)
        .await?;

    let failed_items: i64 = dsl::crawl_queue
        .filter(dsl::status.eq("failed"))
        .select(count(dsl::id))
        .first(&mut conn)
        .await?;

    let retrying_items: i64 = dsl::crawl_queue
        .filter(dsl::status.eq("retrying"))
        .select(count(dsl::id))
        .first(&mut conn)
        .await?;

    // Get oldest pending item
    let oldest_pending: Option<DateTime<Utc>> = dsl::crawl_queue
        .filter(dsl::status.eq("pending"))
        .select(min(dsl::created_at))
        .first(&mut conn)
        .await?;

    // Calculate average processing time for completed items
    let avg_processing_time: Option<f64> = get_average_processing_time(&mut conn).await?;

    Ok(QueueStatistics {
        total_items,
        pending_items,
        processing_items,
        completed_items,
        failed_items,
        retrying_items,
        oldest_pending,
        average_processing_time: avg_processing_time,
    })
}

/// Calculate average processing time (seconds) for completed items:
/// `avg(finished_at - started_at)` over completed rows that have both timestamps.
async fn get_average_processing_time(conn: &mut AsyncPgConnection) -> AppResult<Option<f64>> {
    use diesel::sql_types::{Double, Nullable};

    #[derive(QueryableByName)]
    struct Avg {
        #[diesel(sql_type = Nullable<Double>)]
        avg_seconds: Option<f64>,
    }

    let row = diesel::sql_query(
        "SELECT (AVG(EXTRACT(EPOCH FROM (finished_at - started_at))))::float8 AS avg_seconds \
         FROM crawl_queue \
         WHERE status = 'completed' \
           AND started_at IS NOT NULL \
           AND finished_at IS NOT NULL",
    )
    .get_result::<Avg>(conn)
    .await?;

    Ok(row.avg_seconds)
}

/// Clean up completed and failed queue items that finished more than 30 days ago.
/// (The crawler worker purges on its own schedule; see `WorkerConfig::queue_retention`.)
pub async fn cleanup_old_queue_items(pool: &DatabasePool) -> AppResult<i64> {
    cleanup_old_queue_items_with_retention(pool, 30).await // Default 30 days retention
}

/// Clean up completed and failed queue items that finished more than `retention_days` ago
/// (negative values are treated as 0). Batched; see `CrawlQueueItem::purge_finished`.
pub async fn cleanup_old_queue_items_with_retention(
    pool: &DatabasePool,
    retention_days: i64,
) -> AppResult<i64> {
    let secs = u64::try_from(retention_days.max(0))
        .unwrap_or(0)
        .saturating_mul(86_400);
    CrawlQueueItem::purge_finished(pool, std::time::Duration::from_secs(secs)).await
}

/// Claim the next due item for a worker (see [`CrawlQueueItem::claim_next`]).
pub async fn get_and_lock_next_item(
    pool: &DatabasePool,
    worker_id: &str,
) -> AppResult<Option<CrawlQueueItem>> {
    CrawlQueueItem::claim_next(pool, worker_id, None).await
}

/// Mark item as completed regardless of which worker holds it (admin override; see
/// [`CrawlQueueItem::force_complete`]). Errors with `NotFound` for a missing id.
pub async fn mark_item_completed(pool: &DatabasePool, item_id: Uuid) -> AppResult<()> {
    CrawlQueueItem::force_complete(pool, item_id).await?;
    Ok(())
}

/// Mark item as permanently failed regardless of which worker holds it (admin override; see
/// [`CrawlQueueItem::force_fail`]). Errors with `NotFound` for a missing id.
pub async fn mark_item_failed(
    pool: &DatabasePool,
    item_id: Uuid,
    error_message: String,
) -> AppResult<()> {
    CrawlQueueItem::force_fail(pool, item_id, &error_message).await?;
    Ok(())
}

/// Get items that have been locked for too long (stuck items)
/// These might be from crashed workers and need to be unlocked
pub async fn get_stuck_items(
    pool: &DatabasePool,
    timeout_minutes: i64,
) -> AppResult<Vec<CrawlQueueItem>> {
    use crawl_queue::dsl;

    let mut conn = pool.get().await.map_err(|e| {
        econ_graph_core::error::AppError::DatabaseError(format!(
            "Failed to get database connection: {}",
            e
        ))
    })?;
    let timeout = Utc::now() - Duration::minutes(timeout_minutes);

    let stuck_items = dsl::crawl_queue
        .filter(dsl::status.eq("processing"))
        .filter(dsl::locked_at.is_not_null())
        .filter(dsl::locked_at.lt(timeout))
        .load::<CrawlQueueItem>(&mut conn)
        .await?;

    Ok(stuck_items)
}

/// Unlock stuck items (recover from crashed workers): processing items locked for longer than
/// `timeout_minutes` go back to `pending` with the lock cleared, counting one attempt (items
/// that reach `max_retries` become `failed`); see [`CrawlQueueItem::release_stuck`].
pub async fn unlock_stuck_items(pool: &DatabasePool, timeout_minutes: i64) -> AppResult<i64> {
    let older_than = std::time::Duration::from_secs(timeout_minutes.max(0) as u64 * 60);
    CrawlQueueItem::release_stuck(pool, older_than).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::models::{JobKind, NewCrawlQueueItem};
    use econ_graph_core::test_utils::TestContainer;
    use serial_test::serial;

    async fn load_item(pool: &DatabasePool, id: Uuid) -> CrawlQueueItem {
        let mut conn = pool.get().await.unwrap();
        crawl_queue::table
            .find(id)
            .first::<CrawlQueueItem>(&mut conn)
            .await
            .unwrap()
    }

    #[tokio::test]
    #[serial]
    async fn test_claimed_item_transitions_are_lease_guarded() {
        // REQUIREMENT: a worker finishing the item it claimed must not overwrite a newer claim
        // after its lease expired and the item was claimed again (even under the same worker id).
        let container = TestContainer::new().await;
        let pool = container.pool();
        container.clean_database().await.unwrap();

        let created = CrawlQueueItem::create(
            &pool,
            &NewCrawlQueueItem {
                source: "FRED".to_string(),
                series_id: "TEST_CLAIMED".to_string(),
                priority: 5,
                max_retries: 3,
                scheduled_for: None,
                kind: JobKind::FetchSeries.to_string(),
            },
        )
        .await
        .unwrap();

        let stale = get_next_queue_items(&pool, 1).await.unwrap().remove(0);
        assert_eq!(stale.id, created.id);
        assert_eq!(unlock_stuck_items(&pool, 0).await.unwrap(), 1);
        let fresh = lock_queue_item(&pool, created.id, QUEUE_SERVICE_WORKER_ID)
            .await
            .unwrap();

        assert_eq!(
            complete_claimed_item(&pool, &stale).await.unwrap(),
            LeaseOutcome::LostLease
        );
        assert_eq!(
            fail_claimed_item(&pool, &stale, "boom").await.unwrap(),
            LeaseOutcome::LostLease
        );
        assert_eq!(
            retry_claimed_item_later(&pool, &stale, "503")
                .await
                .unwrap(),
            QueueTransition::LostLease
        );
        assert_eq!(load_item(&pool, created.id).await.status, "processing");

        // The current claim reschedules with backoff, then (reclaimed) completes.
        let t = retry_claimed_item_later(&pool, &fresh, "503")
            .await
            .unwrap();
        assert!(matches!(t, QueueTransition::Rescheduled { .. }));
        let row = load_item(&pool, created.id).await;
        assert_eq!(row.status, "retrying");
        assert!(row.scheduled_for.unwrap() > Utc::now());
        diesel::update(crawl_queue::table.find(created.id))
            .set(crawl_queue::scheduled_for.eq(Some(Utc::now() - Duration::seconds(1))))
            .execute(&mut pool.get().await.unwrap())
            .await
            .unwrap();
        let again = get_and_lock_next_item(&pool, "worker-2")
            .await
            .unwrap()
            .expect("due retry is claimable");
        assert_eq!(
            complete_claimed_item(&pool, &again).await.unwrap(),
            LeaseOutcome::Applied
        );
        assert_eq!(load_item(&pool, created.id).await.status, "completed");
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_statistics_empty() {
        // REQUIREMENT: Queue system should provide monitoring statistics
        // PURPOSE: Verify that statistics are correctly calculated for empty queue
        // This ensures monitoring dashboards can track queue health

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        let stats = get_queue_statistics(&pool).await.unwrap();

        assert_eq!(stats.total_items, 0);
        assert_eq!(stats.pending_items, 0);
        assert_eq!(stats.processing_items, 0);
        assert_eq!(stats.completed_items, 0);
        assert_eq!(stats.failed_items, 0);
        assert_eq!(stats.retrying_items, 0);
        assert!(stats.oldest_pending.is_none());
        assert!(stats.average_processing_time.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_skip_locked_functionality() {
        // REQUIREMENT: Queue must use SKIP LOCKED for concurrent processing
        // PURPOSE: Verify that queue items can be retrieved without blocking
        // This ensures multiple workers can process the queue simultaneously

        use econ_graph_core::test_utils::TestContainer;

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create a test queue item
        let new_item = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "GDP".to_string(),
            priority: 5,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_item = CrawlQueueItem::create(&pool, &new_item).await.unwrap();

        // Get next items using SKIP LOCKED
        let items = get_next_queue_items(&pool, 10).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, created_item.id);
        assert_eq!(items[0].source, "FRED");
        assert_eq!(items[0].series_id, "GDP");
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_item_locking_and_unlocking() {
        // REQUIREMENT: Workers should be able to lock items for processing
        // PURPOSE: Verify that queue items can be locked and unlocked properly
        // This prevents multiple workers from processing the same item

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create a test queue item
        let new_item = NewCrawlQueueItem {
            source: "BLS".to_string(),
            series_id: "UNEMPLOYMENT".to_string(),
            priority: 8,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_item = CrawlQueueItem::create(&pool, &new_item).await.unwrap();
        let worker_id = "test-worker-1";

        // Lock the item
        lock_queue_item(&pool, created_item.id, worker_id)
            .await
            .unwrap();

        // Verify item is locked (should not appear in next items)
        let items = get_next_queue_items(&pool, 10).await.unwrap();
        assert_eq!(
            items.len(),
            0,
            "Locked item should not appear in available items"
        );

        // Unlock the item
        unlock_queue_item(&pool, created_item.id).await.unwrap();

        // The lock must really be cleared (previously `locked_by: None` was skipped by the
        // changeset, so the lock stayed and the item was never re-picked).
        let row = load_item(&pool, created_item.id).await;
        assert_eq!(row.status, "pending");
        assert!(
            row.locked_by.is_none(),
            "locked_by must be NULL after unlock"
        );
        assert!(
            row.locked_at.is_none(),
            "locked_at must be NULL after unlock"
        );

        // ... and the item is available to the next worker.
        let next = get_and_lock_next_item(&pool, "test-worker-2")
            .await
            .unwrap();
        assert_eq!(next.map(|i| i.id), Some(created_item.id));
    }

    #[tokio::test]
    #[serial]
    async fn test_unlock_queue_item_nulls_lock_columns() {
        // REQUIREMENT: unlock_queue_item must null locked_by/locked_at in the database
        let container = TestContainer::new().await;
        let pool = container.pool();
        container.clean_database().await.unwrap();

        let created = CrawlQueueItem::create(
            &pool,
            &NewCrawlQueueItem {
                source: "FRED".to_string(),
                series_id: "UNLOCK_ME".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let claimed = get_and_lock_next_item(&pool, "w-unlock")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, created.id);
        assert!(claimed.locked_by.is_some() && claimed.locked_at.is_some());

        unlock_queue_item(&pool, created.id).await.unwrap();

        let row = load_item(&pool, created.id).await;
        assert_eq!(row.locked_by, None);
        assert_eq!(row.locked_at, None);
        assert_eq!(row.status, "pending");

        // Unlocking a finished item must not resurrect it.
        mark_item_completed(&pool, created.id).await.unwrap();
        unlock_queue_item(&pool, created.id).await.unwrap();
        assert_eq!(load_item(&pool, created.id).await.status, "completed");
    }

    #[tokio::test]
    #[serial]
    async fn test_retry_backoff_and_retrying_items_are_reclaimed() {
        // update_queue_item_for_retry -> retry_later with 2^n minute backoff capped at 60,
        // and a due 'retrying' item is picked up again (bug 3).
        assert_eq!(retry_backoff(1).as_secs(), 2 * 60);
        assert_eq!(retry_backoff(3).as_secs(), 8 * 60);
        assert_eq!(retry_backoff(6).as_secs(), 60 * 60);
        assert_eq!(retry_backoff(30).as_secs(), 60 * 60);

        let container = TestContainer::new().await;
        let pool = container.pool();
        container.clean_database().await.unwrap();

        let created = CrawlQueueItem::create(
            &pool,
            &NewCrawlQueueItem {
                source: "FRED".to_string(),
                series_id: "BACKOFF".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        get_and_lock_next_item(&pool, "w").await.unwrap().unwrap();

        let before = Utc::now();
        update_queue_item_for_retry(&pool, created.id, Some("boom".to_string()))
            .await
            .unwrap();
        let row = load_item(&pool, created.id).await;
        assert_eq!(row.status, "retrying");
        assert_eq!(row.retry_count, 1);
        assert!(row.locked_by.is_none() && row.locked_at.is_none());
        let scheduled = row.scheduled_for.unwrap();
        assert!(scheduled >= before + Duration::seconds(119));
        assert!(scheduled <= Utc::now() + Duration::seconds(121));

        // Not due yet.
        assert!(get_and_lock_next_item(&pool, "w").await.unwrap().is_none());

        // Make it due: it must be claimable again.
        let mut conn = pool.get().await.unwrap();
        diesel::update(crawl_queue::table.find(created.id))
            .set(crawl_queue::scheduled_for.eq(Some(Utc::now() - Duration::seconds(1))))
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        let again = get_and_lock_next_item(&pool, "w2").await.unwrap();
        assert_eq!(again.map(|i| i.id), Some(created.id));
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_status_updates() {
        // REQUIREMENT: Queue items should track processing status
        // PURPOSE: Verify that status updates work correctly for monitoring
        // This ensures queue progress can be tracked and reported

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create a test queue item
        let new_item = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "INFLATION".to_string(),
            priority: 3,
            max_retries: 2,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_item = CrawlQueueItem::create(&pool, &new_item).await.unwrap();

        // Update to completed status
        update_queue_item_status(&pool, created_item.id, QueueStatus::Completed, None)
            .await
            .unwrap();

        // Verify statistics reflect the status change
        let stats = get_queue_statistics(&pool).await.unwrap();
        assert_eq!(stats.total_items, 1);
        assert_eq!(stats.completed_items, 1);
        assert_eq!(stats.pending_items, 0);

        // Update to failed status with error message
        update_queue_item_status(
            &pool,
            created_item.id,
            QueueStatus::Failed,
            Some("API timeout error".to_string()),
        )
        .await
        .unwrap();

        // Verify statistics reflect the new status
        let stats = get_queue_statistics(&pool).await.unwrap();
        assert_eq!(stats.failed_items, 1);
        assert_eq!(stats.completed_items, 0);

        // `processing` without a worker lock is rejected and changes nothing.
        let err = update_queue_item_status(&pool, created_item.id, QueueStatus::Processing, None)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "{err:?}");
        let row = load_item(&pool, created_item.id).await;
        assert_eq!(row.status, "failed");
        assert!(row.locked_by.is_none() && row.locked_at.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_retry_logic() {
        // REQUIREMENT: Failed items should be retried with exponential backoff
        // PURPOSE: Verify that retry logic works correctly for transient failures
        // This ensures resilient data collection from external APIs

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create a test queue item with low max retries
        let new_item = NewCrawlQueueItem {
            source: "BLS".to_string(),
            series_id: "CPI".to_string(),
            priority: 5,
            max_retries: 2,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_item = CrawlQueueItem::create(&pool, &new_item).await.unwrap();

        // First retry
        update_queue_item_for_retry(&pool, created_item.id, Some("Network timeout".to_string()))
            .await
            .unwrap();

        let stats = get_queue_statistics(&pool).await.unwrap();
        assert_eq!(stats.retrying_items, 1);

        // Second retry (should reach max retries and fail)
        update_queue_item_for_retry(&pool, created_item.id, Some("Still timing out".to_string()))
            .await
            .unwrap();

        let stats = get_queue_statistics(&pool).await.unwrap();
        assert_eq!(stats.failed_items, 1);
        assert_eq!(stats.retrying_items, 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_get_and_lock_next_item() {
        // REQUIREMENT: Workers should get and lock items in one atomic operation
        // PURPOSE: Verify that get_and_lock_next_item works correctly
        // This ensures efficient worker processing without race conditions

        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();

        // Create multiple test queue items with different priorities
        let high_priority = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "GDP".to_string(),
            priority: 9, // High priority
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let low_priority = NewCrawlQueueItem {
            source: "BLS".to_string(),
            series_id: "UNEMPLOYMENT".to_string(),
            priority: 2, // Low priority
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        CrawlQueueItem::create(&pool, &low_priority).await.unwrap();
        let high_item = CrawlQueueItem::create(&pool, &high_priority).await.unwrap();

        // Get and lock next item - should return high priority item
        let worker_id = "test-worker-priority";
        let locked_item = get_and_lock_next_item(&pool, worker_id).await.unwrap();

        assert!(locked_item.is_some());
        let item = locked_item.unwrap();

        // The returned item should be either our high priority item, or another high priority item
        // The key is that it should be locked and have the correct status
        assert_eq!(item.status, "processing");
        assert_eq!(item.locked_by, Some(worker_id.to_string()));

        // If it's our high priority item, verify the priority
        if item.id == high_item.id {
            assert_eq!(item.priority, 9);
        }

        // Verify no more items available (one is locked, other is lower priority but should still be available)
        let next_item = get_and_lock_next_item(&pool, "worker-2").await.unwrap();
        assert!(next_item.is_some()); // Should get the low priority item
    }

    #[tokio::test]
    #[serial]
    async fn test_cleanup_old_queue_items() {
        // REQUIREMENT: Old completed items should be cleaned up to prevent database bloat
        // PURPOSE: Verify that cleanup functionality works correctly
        // This ensures long-running systems don't accumulate unlimited queue history

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create and complete a test item
        let new_item = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "TEST_CLEANUP".to_string(),
            priority: 5,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_item = CrawlQueueItem::create(&pool, &new_item).await.unwrap();

        // Mark as completed
        mark_item_completed(&pool, created_item.id).await.unwrap();

        // Verify item exists
        let stats = get_queue_statistics(&pool).await.unwrap();
        assert_eq!(stats.total_items, 1);
        assert_eq!(stats.completed_items, 1);

        // Clean up with 0 day retention (should remove the item)
        let deleted_count = cleanup_old_queue_items_with_retention(&pool, 0)
            .await
            .unwrap();
        assert_eq!(deleted_count, 1);

        // Verify item is gone
        let stats = get_queue_statistics(&pool).await.unwrap();
        assert_eq!(stats.total_items, 0);
        assert_eq!(stats.completed_items, 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_mark_item_failed() {
        // REQUIREMENT: Items should be marked as failed with error messages
        // PURPOSE: Verify that mark_item_failed works correctly
        // This ensures failed items are properly tracked for debugging

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create a test queue item
        let new_item = NewCrawlQueueItem {
            source: "BLS".to_string(),
            series_id: "TEST_FAILED".to_string(),
            priority: 5,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_item = CrawlQueueItem::create(&pool, &new_item).await.unwrap();
        let error_message = "API rate limit exceeded".to_string();

        // Mark as failed
        mark_item_failed(&pool, created_item.id, error_message.clone())
            .await
            .unwrap();

        // Verify statistics reflect the failure
        let stats = get_queue_statistics(&pool).await.unwrap();
        assert_eq!(stats.total_items, 1);
        assert_eq!(stats.failed_items, 1);
        assert_eq!(stats.pending_items, 0);

        // Verify the item is marked as failed (would need to query the item directly to check error message)
        println!("Item marked as failed with error: {}", error_message);
    }

    #[tokio::test]
    #[serial]
    async fn test_get_stuck_items() {
        // REQUIREMENT: System should identify items that have been locked too long
        // PURPOSE: Verify that get_stuck_items correctly identifies stuck items
        // This ensures recovery from crashed workers

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create a test queue item
        let new_item = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "TEST_STUCK".to_string(),
            priority: 5,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_item = CrawlQueueItem::create(&pool, &new_item).await.unwrap();
        let worker_id = "test-worker-stuck";

        // Lock the item
        lock_queue_item(&pool, created_item.id, worker_id)
            .await
            .unwrap();

        // Get stuck items with very short timeout (should find our item)
        let stuck_items = get_stuck_items(&pool, 0).await.unwrap();
        assert_eq!(stuck_items.len(), 1);
        assert_eq!(stuck_items[0].id, created_item.id);
        assert_eq!(stuck_items[0].locked_by, Some(worker_id.to_string()));

        // Get stuck items with very long timeout (should find no items)
        let stuck_items_long = get_stuck_items(&pool, 10080).await.unwrap(); // 1 week
        assert_eq!(stuck_items_long.len(), 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_unlock_stuck_items() {
        // REQUIREMENT: System should unlock items that have been stuck too long
        // PURPOSE: Verify that unlock_stuck_items correctly recovers from crashed workers
        // This ensures automatic recovery from worker failures

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create multiple test queue items
        let new_item1 = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "TEST_STUCK_1".to_string(),
            priority: 5,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let new_item2 = NewCrawlQueueItem {
            source: "BLS".to_string(),
            series_id: "TEST_STUCK_2".to_string(),
            priority: 3,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_item1 = CrawlQueueItem::create(&pool, &new_item1).await.unwrap();
        let created_item2 = CrawlQueueItem::create(&pool, &new_item2).await.unwrap();

        // Lock both items
        lock_queue_item(&pool, created_item1.id, "worker-1")
            .await
            .unwrap();
        lock_queue_item(&pool, created_item2.id, "worker-2")
            .await
            .unwrap();

        // Verify both items are locked
        let stats = get_queue_statistics(&pool).await.unwrap();
        assert_eq!(stats.processing_items, 2);

        // Unlock stuck items with short timeout
        let unlocked_count = unlock_stuck_items(&pool, 0).await.unwrap();
        assert_eq!(unlocked_count, 2);

        // Verify both items are now unlocked (back to pending)
        let stats = get_queue_statistics(&pool).await.unwrap();
        assert_eq!(stats.processing_items, 0);
        assert_eq!(stats.pending_items, 2);
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_statistics_with_processing_time() {
        // REQUIREMENT: Queue statistics should include average processing time
        // PURPOSE: Verify that processing time calculations work correctly
        // This ensures monitoring can track performance metrics

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create and process a test item to generate processing time data
        let new_item = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "TEST_PROCESSING_TIME".to_string(),
            priority: 5,
            max_retries: 3,
            scheduled_for: None,
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_item = CrawlQueueItem::create(&pool, &new_item).await.unwrap();

        // Lock the item (simulates processing start)
        lock_queue_item(&pool, created_item.id, "test-worker")
            .await
            .unwrap();

        // Wait a small amount to simulate processing time
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Mark as completed
        mark_item_completed(&pool, created_item.id).await.unwrap();

        // Get statistics and verify processing time is calculated
        let stats = get_queue_statistics(&pool).await.unwrap();
        assert_eq!(stats.total_items, 1);
        assert_eq!(stats.completed_items, 1);

        // Completing clears the lock but records started_at / finished_at, which the average uses.
        let row = load_item(&pool, created_item.id).await;
        assert!(row.locked_at.is_none());
        let started = row.started_at.expect("claim sets started_at");
        let finished = row.finished_at.expect("complete sets finished_at");
        assert!(finished >= started);
        let avg = stats
            .average_processing_time
            .expect("average processing time should be computed for completed items");
        assert!(avg >= 0.01, "expected at least the 10ms sleep, got {avg}");
        assert!(avg < 60.0, "unexpectedly long processing time: {avg}");
        println!(
            "Average processing time: {:?}",
            stats.average_processing_time
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_scheduled_items() {
        // REQUIREMENT: Queue should respect scheduled_for timestamps
        // PURPOSE: Verify that scheduled items are not processed until their time
        // This enables delayed processing and rate limiting

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create an item scheduled for the future
        let future_time = Utc::now() + chrono::Duration::hours(1);
        let scheduled_item = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "SCHEDULED_ITEM".to_string(),
            priority: 5,
            max_retries: 3,
            scheduled_for: Some(future_time),
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_item = CrawlQueueItem::create(&pool, &scheduled_item)
            .await
            .unwrap();

        // Get next items - should not include the scheduled item
        let items = get_next_queue_items(&pool, 10).await.unwrap();
        assert_eq!(items.len(), 0, "Scheduled item should not be available yet");

        // Create an item scheduled for the past (should be available)
        let past_time = Utc::now() - chrono::Duration::hours(1);
        let past_item = NewCrawlQueueItem {
            source: "BLS".to_string(),
            series_id: "PAST_ITEM".to_string(),
            priority: 5,
            max_retries: 3,
            scheduled_for: Some(past_time),
            kind: JobKind::FetchSeries.to_string(),
        };

        let created_past_item = CrawlQueueItem::create(&pool, &past_item).await.unwrap();

        // Get next items - should include the past item
        let items = get_next_queue_items(&pool, 10).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, created_past_item.id);
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_error_handling() {
        // REQUIREMENT: Queue operations should handle errors gracefully
        // PURPOSE: Verify that error conditions are handled properly
        // This ensures system resilience

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Test operations on non-existent item
        let fake_id = Uuid::new_v4();

        // These should not panic, but may return errors or succeed silently
        let lock_result = lock_queue_item(&pool, fake_id, "test-worker").await;
        // Lock operation might succeed even for non-existent items (depends on implementation)
        println!("Lock non-existent item result: {:?}", lock_result);

        let unlock_result = unlock_queue_item(&pool, fake_id).await;
        // Unlock operation might succeed even for non-existent items
        println!("Unlock non-existent item result: {:?}", unlock_result);

        let mark_completed_result = mark_item_completed(&pool, fake_id).await;
        println!(
            "Mark non-existent item completed result: {:?}",
            mark_completed_result
        );

        // Test with invalid parameters
        let stats_result = get_queue_statistics(&pool).await;
        assert!(stats_result.is_ok(), "Statistics should always work");

        let cleanup_result = cleanup_old_queue_items_with_retention(&pool, -1).await;
        // Cleanup with negative retention should handle gracefully
        println!(
            "Cleanup with negative retention result: {:?}",
            cleanup_result
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_concurrent_access() {
        // REQUIREMENT: Queue should handle concurrent access safely
        // PURPOSE: Verify that SKIP LOCKED prevents race conditions
        // This ensures multiple workers can operate safely

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Clean database to ensure test isolation
        container.clean_database().await.unwrap();

        // Create multiple items
        let items = vec![
            NewCrawlQueueItem {
                source: "FRED".to_string(),
                series_id: "CONCURRENT_1".to_string(),
                priority: 5,
                max_retries: 3,
                scheduled_for: None,
                kind: JobKind::FetchSeries.to_string(),
            },
            NewCrawlQueueItem {
                source: "BLS".to_string(),
                series_id: "CONCURRENT_2".to_string(),
                priority: 5,
                max_retries: 3,
                scheduled_for: None,
                kind: JobKind::FetchSeries.to_string(),
            },
            NewCrawlQueueItem {
                source: "CENSUS".to_string(),
                series_id: "CONCURRENT_3".to_string(),
                priority: 5,
                max_retries: 3,
                scheduled_for: None,
                kind: JobKind::FetchSeries.to_string(),
            },
        ];

        for item in &items {
            CrawlQueueItem::create(&pool, item).await.unwrap();
        }

        // Simulate concurrent access by getting items multiple times
        let mut handles = vec![];
        for i in 0..3 {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                // get_and_lock_next_item claims atomically; a separate get + lock would race.
                get_and_lock_next_item(&pool_clone, &format!("worker-{}", i))
                    .await
                    .unwrap()
                    .into_iter()
                    .collect::<Vec<_>>()
            });
            handles.push(handle);
        }

        // Wait for all concurrent operations
        let mut results = vec![];
        for handle in handles {
            let result = handle.await.unwrap();
            results.push(result);
        }

        // Verify that different items were locked by different workers
        #[allow(clippy::get_first)]
        let locked_items: std::collections::HashSet<Uuid> = results
            .iter()
            .filter_map(|r| r.get(0).map(|item| item.id))
            .collect();

        // Each of the 3 workers claimed a different one of the 3 items.
        assert_eq!(results.iter().map(|r| r.len()).sum::<usize>(), 3);
        assert_eq!(locked_items.len(), 3);
        println!(
            "Concurrent access test completed. Locked items: {}",
            locked_items.len()
        );
    }
}
