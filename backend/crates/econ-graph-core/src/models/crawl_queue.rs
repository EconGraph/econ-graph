use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::schema::crawl_queue;

/// Crawl queue item for managing data collection jobs
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
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
}

/// Crawl queue item update model
#[derive(Debug, Clone, AsChangeset, Validate, Deserialize)]
#[diesel(table_name = crawl_queue)]
pub struct UpdateCrawlQueueItem {
    pub status: Option<String>,
    pub retry_count: Option<i32>,
    #[validate(length(max = 2000))]
    pub error_message: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub scheduled_for: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub locked_at: Option<DateTime<Utc>>,
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

impl CrawlQueueItem {
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

    /// Check if the item is ready for processing
    pub fn is_ready_for_processing(&self) -> bool {
        matches!(QueueStatus::from(self.status.clone()), QueueStatus::Pending)
            && !self.is_locked()
            && self
                .scheduled_for
                .is_none_or(|scheduled| scheduled <= Utc::now())
    }

    /// Calculate processing duration if locked
    pub fn processing_duration(&self) -> Option<i64> {
        self.locked_at
            .map(|locked_at| (Utc::now() - locked_at).num_seconds())
    }

    /// Create a new crawl queue item
    pub async fn create(
        pool: &crate::database::DatabasePool,
        new_item: &NewCrawlQueueItem,
    ) -> crate::error::AppResult<Self> {
        use crate::schema::crawl_queue::dsl;

        let mut conn = pool.get().await?;

        let item = diesel::insert_into(dsl::crawl_queue)
            .values(new_item)
            .get_result::<Self>(&mut conn)
            .await?;

        Ok(item)
    }

    /// Get next available item for processing using SKIP LOCKED
    pub async fn get_next_for_processing(
        pool: &crate::database::DatabasePool,
        worker_id: &str,
    ) -> crate::error::AppResult<Option<Self>> {
        use crate::schema::crawl_queue::dsl;

        let mut conn = pool.get().await?;

        // Use SKIP LOCKED to get the next available item for processing
        let item = dsl::crawl_queue
            .filter(dsl::status.eq("pending"))
            .filter(dsl::locked_by.is_null())
            .filter(
                dsl::scheduled_for
                    .is_null()
                    .or(dsl::scheduled_for.le(Utc::now())),
            )
            .order(dsl::priority.desc())
            .order(dsl::created_at.asc())
            .for_update()
            .skip_locked()
            .first::<Self>(&mut conn)
            .await
            .optional()?;

        if let Some(item) = item {
            // Lock the item for this worker
            let item_id = item.id;
            let update = UpdateCrawlQueueItem {
                status: Some("processing".to_string()),
                locked_by: Some(worker_id.to_string()),
                locked_at: Some(Utc::now()),
                updated_at: Utc::now(),
                ..Default::default()
            };

            let item = diesel::update(dsl::crawl_queue.filter(dsl::id.eq(item_id)))
                .set(&update)
                .get_result::<Self>(&mut conn)
                .await?;

            return Ok(Some(item));
        }

        Ok(None)
    }

    /// Update crawl queue item
    pub async fn update(
        pool: &crate::database::DatabasePool,
        id: uuid::Uuid,
        update_data: &UpdateCrawlQueueItem,
    ) -> crate::error::AppResult<Self> {
        use crate::schema::crawl_queue::dsl;

        let mut conn = pool.get().await?;

        let item = diesel::update(dsl::crawl_queue.filter(dsl::id.eq(id)))
            .set(update_data)
            .get_result::<Self>(&mut conn)
            .await?;

        Ok(item)
    }

    /// Mark item as completed
    pub async fn mark_completed(
        pool: &crate::database::DatabasePool,
        id: uuid::Uuid,
    ) -> crate::error::AppResult<Self> {
        let update = UpdateCrawlQueueItem {
            status: Some("completed".to_string()),
            locked_by: None,
            locked_at: None,
            updated_at: Utc::now(),
            ..Default::default()
        };

        Self::update(pool, id, &update).await
    }

    /// Mark item as failed
    pub async fn mark_failed(
        pool: &crate::database::DatabasePool,
        id: uuid::Uuid,
        error_message: String,
    ) -> crate::error::AppResult<Self> {
        let update = UpdateCrawlQueueItem {
            status: Some("failed".to_string()),
            error_message: Some(error_message),
            locked_by: None,
            locked_at: None,
            updated_at: Utc::now(),
            ..Default::default()
        };

        Self::update(pool, id, &update).await
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

        // Test locking mechanism - prevents concurrent processing of same item
        item.locked_by = Some("worker-1".to_string());
        item.locked_at = Some(Utc::now());
        assert!(item.is_locked(), "Locked item should report as locked");
        assert!(
            !item.is_ready_for_processing(),
            "Locked item should not be ready for processing"
        );
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
            priority: 5,
            max_retries: 3,
            scheduled_for: None,
        };

        assert!(
            invalid_item.validate().is_err(),
            "Empty source should fail validation"
        );

        // Test priority validation - ensures priority values are within valid range
        let invalid_priority = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "GDP".to_string(),
            priority: 0, // Below minimum priority
            max_retries: 3,
            scheduled_for: None,
        };

        assert!(
            invalid_priority.validate().is_err(),
            "Invalid priority should fail validation"
        );
    }
}

#[cfg(test)]
mod tests;
