//! Crawl attempt model for tracking crawl history and success rates

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::schema::crawl_attempts;
use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel::ExpressionMethods;
use diesel_async::RunQueryDsl;
// Removed futures_util import as we're using .await? instead of .map_err()
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Crawl attempt record for tracking crawl history
#[derive(
    Debug, Clone, Serialize, Deserialize, Queryable, Identifiable, Associations, Selectable,
)]
#[diesel(belongs_to(crate::models::economic_series::EconomicSeries, foreign_key = series_id))]
#[diesel(table_name = crawl_attempts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CrawlAttempt {
    pub id: Uuid,
    pub series_id: Uuid,
    pub attempted_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,

    // Crawl attempt details
    pub crawl_method: String,
    pub crawl_url: Option<String>,
    pub http_status_code: Option<i32>,

    // Data freshness tracking
    pub data_found: bool,
    pub new_data_points: Option<i32>,
    pub latest_data_date: Option<NaiveDate>,
    pub data_freshness_hours: Option<i32>,

    // Error tracking
    pub success: bool,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: Option<i32>,

    // Performance metrics
    pub response_time_ms: Option<i32>,
    pub data_size_bytes: Option<i32>,
    pub rate_limit_remaining: Option<i32>,

    // Metadata
    pub user_agent: Option<String>,
    pub request_headers: Option<serde_json::Value>,
    pub response_headers: Option<serde_json::Value>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New crawl attempt for insertion
#[derive(Debug, Clone, Insertable, Validate)]
#[diesel(table_name = crawl_attempts)]
pub struct NewCrawlAttempt {
    pub series_id: Uuid,
    pub attempted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,

    // Crawl attempt details
    #[validate(length(max = 50))]
    pub crawl_method: String,
    pub crawl_url: Option<String>,
    pub http_status_code: Option<i32>,

    // Data freshness tracking
    pub data_found: Option<bool>,
    pub new_data_points: Option<i32>,
    pub latest_data_date: Option<NaiveDate>,
    pub data_freshness_hours: Option<i32>,

    // Error tracking
    pub success: Option<bool>,
    #[validate(length(max = 50))]
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: Option<i32>,

    // Performance metrics
    pub response_time_ms: Option<i32>,
    pub data_size_bytes: Option<i32>,
    pub rate_limit_remaining: Option<i32>,

    // Metadata
    pub user_agent: Option<String>,
    pub request_headers: Option<serde_json::Value>,
    pub response_headers: Option<serde_json::Value>,
}

impl Default for NewCrawlAttempt {
    fn default() -> Self {
        Self {
            series_id: Uuid::new_v4(),
            attempted_at: Some(Utc::now()),
            completed_at: None,
            crawl_method: "api".to_string(),
            crawl_url: None,
            http_status_code: None,
            data_found: Some(false),
            new_data_points: Some(0),
            latest_data_date: None,
            data_freshness_hours: None,
            success: Some(false),
            error_type: None,
            error_message: None,
            retry_count: Some(0),
            response_time_ms: None,
            data_size_bytes: None,
            rate_limit_remaining: None,
            user_agent: None,
            request_headers: None,
            response_headers: None,
        }
    }
}

/// Crawl attempt status for tracking progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrawlAttemptStatus {
    Started,
    InProgress,
    Completed,
    Failed,
    Retrying,
}

/// Error types for crawl attempts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrawlErrorType {
    Network,
    ApiLimit,
    DataFormat,
    NotFound,
    Authentication,
    RateLimit,
    Timeout,
    ServerError,
    Unknown,
}

impl CrawlAttempt {
    /// Create a new crawl attempt
    pub async fn create(pool: &DatabasePool, new_attempt: &NewCrawlAttempt) -> AppResult<Self> {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let attempt = diesel::insert_into(crawl_attempts::table)
            .values(new_attempt)
            .returning(CrawlAttempt::as_returning())
            .get_result(&mut conn)
            .await?;

        Ok(attempt)
    }

    /// Get crawl attempts for a specific series
    pub async fn get_by_series_id(pool: &DatabasePool, series_id: &Uuid) -> AppResult<Vec<Self>> {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let attempts = crawl_attempts::table
            .filter(crawl_attempts::series_id.eq(*series_id))
            .order(crawl_attempts::attempted_at.desc())
            .load::<CrawlAttempt>(&mut conn)
            .await?;

        Ok(attempts)
    }

    /// Get recent crawl attempts (last N days)
    pub async fn get_recent(pool: &DatabasePool, days: i32) -> AppResult<Vec<Self>> {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);

        let attempts = crawl_attempts::table
            .filter(crawl_attempts::attempted_at.ge(cutoff_date))
            .order(crawl_attempts::attempted_at.desc())
            .load::<CrawlAttempt>(&mut conn)
            .await?;

        Ok(attempts)
    }

    /// Get crawl success rate for a series
    pub async fn get_success_rate(
        pool: &DatabasePool,
        series_id: &Uuid,
        days: i32,
    ) -> AppResult<f64> {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);

        let total_attempts: i64 = crawl_attempts::table
            .filter(crawl_attempts::series_id.eq(*series_id))
            .filter(crawl_attempts::attempted_at.ge(cutoff_date))
            .count()
            .get_result(&mut conn)
            .await?;

        if total_attempts == 0 {
            return Ok(0.0);
        }

        let successful_attempts: i64 = crawl_attempts::table
            .filter(crawl_attempts::series_id.eq(*series_id))
            .filter(crawl_attempts::attempted_at.ge(cutoff_date))
            .filter(crawl_attempts::success.eq(true))
            .count()
            .get_result(&mut conn)
            .await?;

        Ok(successful_attempts as f64 / total_attempts as f64)
    }

    /// Get average data freshness for a series
    pub async fn get_avg_data_freshness(
        pool: &DatabasePool,
        series_id: &Uuid,
        days: i32,
    ) -> AppResult<Option<f64>> {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);

        let avg_freshness: Option<BigDecimal> = crawl_attempts::table
            .filter(crawl_attempts::series_id.eq(*series_id))
            .filter(crawl_attempts::attempted_at.ge(cutoff_date))
            .filter(crawl_attempts::data_found.eq(true))
            .filter(crawl_attempts::data_freshness_hours.is_not_null())
            .select(diesel::dsl::avg(crawl_attempts::data_freshness_hours))
            .first(&mut conn)
            .await?;

        // Convert BigDecimal to f64
        let result = avg_freshness.map(|bd| bd.to_f64().unwrap_or(0.0));
        Ok(result)
    }

    /// Update crawl attempt with completion data
    pub async fn update_completion(
        pool: &DatabasePool,
        attempt_id: &Uuid,
        success: bool,
        data_found: bool,
        new_data_points: i32,
        latest_data_date: Option<NaiveDate>,
        data_freshness_hours: Option<i32>,
        error_type: Option<String>,
        error_message: Option<String>,
        response_time_ms: Option<i32>,
        data_size_bytes: Option<i32>,
    ) -> AppResult<Self> {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let updated_attempt =
            diesel::update(crawl_attempts::table.filter(crawl_attempts::id.eq(*attempt_id)))
                .set((
                    crawl_attempts::completed_at.eq(Some(Utc::now())),
                    crawl_attempts::success.eq(success),
                    crawl_attempts::data_found.eq(data_found),
                    crawl_attempts::new_data_points.eq(new_data_points),
                    crawl_attempts::latest_data_date.eq(latest_data_date),
                    crawl_attempts::data_freshness_hours.eq(data_freshness_hours),
                    crawl_attempts::error_type.eq(error_type),
                    crawl_attempts::error_message.eq(error_message),
                    crawl_attempts::response_time_ms.eq(response_time_ms),
                    crawl_attempts::data_size_bytes.eq(data_size_bytes),
                    crawl_attempts::updated_at.eq(Utc::now()),
                ))
                .returning(CrawlAttempt::as_returning())
                .get_result(&mut conn)
                .await?;

        Ok(updated_attempt)
    }

    /// Get crawl statistics for predictive crawling
    pub async fn get_crawl_statistics(
        pool: &DatabasePool,
        series_id: &Uuid,
    ) -> AppResult<CrawlStatistics> {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Get last 30 days of attempts
        let cutoff_date = Utc::now() - chrono::Duration::days(30);

        let attempts = crawl_attempts::table
            .filter(crawl_attempts::series_id.eq(*series_id))
            .filter(crawl_attempts::attempted_at.ge(cutoff_date))
            .order(crawl_attempts::attempted_at.desc())
            .load::<CrawlAttempt>(&mut conn)
            .await?;

        if attempts.is_empty() {
            return Ok(CrawlStatistics::default());
        }

        let total_attempts = attempts.len();
        let successful_attempts = attempts.iter().filter(|a| a.success).count();
        let data_found_attempts = attempts.iter().filter(|a| a.data_found).count();

        let success_rate = successful_attempts as f64 / total_attempts as f64;
        let data_found_rate = data_found_attempts as f64 / total_attempts as f64;

        // Calculate average data freshness
        let freshness_values: Vec<i32> = attempts
            .iter()
            .filter_map(|a| a.data_freshness_hours)
            .collect();

        let avg_freshness = if !freshness_values.is_empty() {
            Some(freshness_values.iter().sum::<i32>() as f64 / freshness_values.len() as f64)
        } else {
            None
        };

        // Calculate average response time
        let response_times: Vec<i32> = attempts.iter().filter_map(|a| a.response_time_ms).collect();

        let avg_response_time = if !response_times.is_empty() {
            Some(response_times.iter().sum::<i32>() as f64 / response_times.len() as f64)
        } else {
            None
        };

        // Get most recent attempt
        let last_attempt = if !attempts.is_empty() {
            Some(attempts[0].clone())
        } else {
            None
        };

        Ok(CrawlStatistics {
            series_id: *series_id,
            total_attempts,
            successful_attempts,
            data_found_attempts,
            success_rate,
            data_found_rate,
            avg_freshness_hours: avg_freshness,
            avg_response_time_ms: avg_response_time,
            last_attempt,
            recommended_crawl_frequency_hours: Self::calculate_recommended_frequency(
                success_rate,
                avg_freshness,
                data_found_rate,
            ),
        })
    }

    /// Calculate recommended crawl frequency based on historical data
    fn calculate_recommended_frequency(
        success_rate: f64,
        avg_freshness: Option<f64>,
        data_found_rate: f64,
    ) -> i32 {
        // Base frequency on data freshness and success rate
        let base_frequency = if let Some(freshness) = avg_freshness {
            // If data is typically fresh (less than 24 hours old), crawl more frequently
            if freshness < 24.0 {
                6 // Every 6 hours
            } else if freshness < 168.0 {
                // 1 week
                24 // Daily
            } else {
                168 // Weekly
            }
        } else {
            24 // Default to daily if no freshness data
        };

        // Adjust based on success rate
        let adjusted_frequency = if success_rate > 0.9 {
            base_frequency // High success rate, keep current frequency
        } else if success_rate > 0.7 {
            base_frequency * 2 // Medium success rate, reduce frequency
        } else {
            base_frequency * 4 // Low success rate, reduce frequency significantly
        };

        // Adjust based on data found rate
        let final_frequency = if data_found_rate > 0.8 {
            adjusted_frequency // High data found rate, keep frequency
        } else if data_found_rate > 0.5 {
            adjusted_frequency * 2 // Medium data found rate, reduce frequency
        } else {
            adjusted_frequency * 4 // Low data found rate, reduce frequency significantly
        };

        final_frequency.max(1).min(168 * 4) // Between 1 hour and 4 weeks
    }
}

/// Crawl statistics for predictive crawling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlStatistics {
    pub series_id: Uuid,
    pub total_attempts: usize,
    pub successful_attempts: usize,
    pub data_found_attempts: usize,
    pub success_rate: f64,
    pub data_found_rate: f64,
    pub avg_freshness_hours: Option<f64>,
    pub avg_response_time_ms: Option<f64>,
    pub last_attempt: Option<CrawlAttempt>,
    pub recommended_crawl_frequency_hours: i32,
}

impl Default for CrawlStatistics {
    fn default() -> Self {
        Self {
            series_id: Uuid::new_v4(),
            total_attempts: 0,
            successful_attempts: 0,
            data_found_attempts: 0,
            success_rate: 0.0,
            data_found_rate: 0.0,
            avg_freshness_hours: None,
            avg_response_time_ms: None,
            last_attempt: None,
            recommended_crawl_frequency_hours: 24, // Default to daily
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestContainer;

    #[tokio::test]
    async fn test_new_crawl_attempt_default() {
        let attempt = NewCrawlAttempt::default();

        assert_eq!(attempt.crawl_method, "api");
        assert_eq!(attempt.data_found, Some(false));
        assert_eq!(attempt.new_data_points, Some(0));
        assert_eq!(attempt.success, Some(false));
        assert_eq!(attempt.retry_count, Some(0));
        assert!(attempt.attempted_at.is_some());
        assert!(attempt.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_crawl_statistics_default() {
        let stats = CrawlStatistics::default();

        assert_eq!(stats.total_attempts, 0);
        assert_eq!(stats.successful_attempts, 0);
        assert_eq!(stats.data_found_attempts, 0);
        assert_eq!(stats.success_rate, 0.0);
        assert_eq!(stats.data_found_rate, 0.0);
        assert_eq!(stats.recommended_crawl_frequency_hours, 24);
        assert!(stats.avg_freshness_hours.is_none());
        assert!(stats.avg_response_time_ms.is_none());
        assert!(stats.last_attempt.is_none());
    }

    #[tokio::test]
    async fn test_crawl_attempt_create_and_retrieve() {
        let container = TestContainer::new().await;
        let pool = container.pool();

        // Create a test series first
        use crate::models::{DataSource, EconomicSeries, NewDataSource, NewEconomicSeries};
        use chrono::NaiveDate;

        let data_source = DataSource::create(
            &pool,
            NewDataSource {
                name: "Test Crawl Source".to_string(),
                description: Some("Test source for crawl attempts".to_string()),
                base_url: "https://test-crawl.example.com/api".to_string(),
                api_key_required: false,
                rate_limit_per_minute: 100,
                is_visible: true,
                is_enabled: true,
                requires_admin_approval: false,
                crawl_frequency_hours: 24,
                api_documentation_url: Some("https://test-crawl.example.com/docs".to_string()),
            },
        )
        .await
        .expect("Should create data source");

        let series = EconomicSeries::create(
            &pool,
            &NewEconomicSeries {
                title: "Test Crawl Series".to_string(),
                description: Some("Test series for crawl attempts".to_string()),
                external_id: "TEST_CRAWL_001".to_string(),
                source_id: data_source.id,
                frequency: "Monthly".to_string(),
                units: Some("Test Units".to_string()),
                seasonal_adjustment: Some("Not Seasonally Adjusted".to_string()),
                first_discovered_at: Some(chrono::Utc::now()),
                last_crawled_at: None,
                first_missing_date: None,
                crawl_status: None,
                crawl_error_message: None,
                start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                end_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
                is_active: true,
            },
        )
        .await
        .expect("Should create economic series");

        // Create a crawl attempt
        let new_attempt = NewCrawlAttempt {
            series_id: series.id,
            attempted_at: Some(chrono::Utc::now()),
            completed_at: None,
            crawl_method: "api".to_string(),
            crawl_url: Some("https://test-crawl.example.com/api/series/TEST_CRAWL_001".to_string()),
            http_status_code: None,
            data_found: Some(false),
            new_data_points: Some(0),
            latest_data_date: None,
            data_freshness_hours: None,
            success: Some(false),
            error_type: None,
            error_message: None,
            retry_count: Some(0),
            response_time_ms: None,
            data_size_bytes: None,
            rate_limit_remaining: None,
            user_agent: Some("EconGraph-Crawler/1.0".to_string()),
            request_headers: None,
            response_headers: None,
        };

        let attempt = CrawlAttempt::create(&pool, &new_attempt)
            .await
            .expect("Should create crawl attempt");

        assert_eq!(attempt.series_id, series.id);
        assert_eq!(attempt.crawl_method, "api");
        assert!(!attempt.data_found);
        assert!(!attempt.success);
        assert_eq!(attempt.retry_count, Some(0));

        // Test retrieving attempts by series ID
        let attempts = CrawlAttempt::get_by_series_id(&pool, &series.id)
            .await
            .expect("Should get crawl attempts");

        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].id, attempt.id);

        // Test updating completion
        let updated_attempt = CrawlAttempt::update_completion(
            &pool,
            &attempt.id,
            true,                                                // success
            true,                                                // data_found
            5,                                                   // new_data_points
            Some(NaiveDate::from_ymd_opt(2024, 12, 1).unwrap()), // latest_data_date
            Some(24),                                            // data_freshness_hours
            None,                                                // error_type
            None,                                                // error_message
            Some(1500),                                          // response_time_ms
            Some(2048),                                          // data_size_bytes
        )
        .await
        .expect("Should update crawl attempt");

        assert!(updated_attempt.success);
        assert!(updated_attempt.data_found);
        assert_eq!(updated_attempt.new_data_points, Some(5));
        assert_eq!(updated_attempt.response_time_ms, Some(1500));
        assert_eq!(updated_attempt.data_size_bytes, Some(2048));
        assert!(updated_attempt.completed_at.is_some());

        // Test getting success rate
        let success_rate = CrawlAttempt::get_success_rate(&pool, &series.id, 30)
            .await
            .expect("Should get success rate");

        assert_eq!(success_rate, 1.0); // 100% success rate

        // Test getting crawl statistics
        let stats = CrawlAttempt::get_crawl_statistics(&pool, &series.id)
            .await
            .expect("Should get crawl statistics");

        assert_eq!(stats.total_attempts, 1);
        assert_eq!(stats.successful_attempts, 1);
        assert_eq!(stats.data_found_attempts, 1);
        assert_eq!(stats.success_rate, 1.0);
        assert_eq!(stats.data_found_rate, 1.0);
        assert_eq!(stats.avg_freshness_hours, Some(24.0));
        assert_eq!(stats.avg_response_time_ms, Some(1500.0));
        assert!(stats.last_attempt.is_some());
    }
}
