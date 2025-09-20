//! Metrics service for periodically updating application-specific metrics
//!
//! This service runs in the background and periodically updates metrics
//! that require database queries to get current counts and status.

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::metrics_enhanced::*;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

/// Metrics service that periodically updates application metrics
pub struct MetricsService {
    pool: Arc<DatabasePool>,
    update_interval: Duration,
}

impl MetricsService {
    /// Create a new metrics service
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self {
            pool,
            update_interval: Duration::from_secs(60), // Update every minute
        }
    }

    /// Create a new metrics service with custom update interval
    pub fn with_interval(pool: Arc<DatabasePool>, interval: Duration) -> Self {
        Self {
            pool,
            update_interval: interval,
        }
    }

    /// Start the metrics service
    pub async fn start(self) -> AppResult<()> {
        info!("Starting metrics service with {}s update interval", self.update_interval.as_secs());

        let mut interval_timer = interval(self.update_interval);

        loop {
            interval_timer.tick().await;

            if let Err(e) = self.update_metrics().await {
                error!("Failed to update metrics: {}", e);
                record_error("metrics_update", "metrics_service");
            }
        }
    }

    /// Update all application-specific metrics
    async fn update_metrics(&self) -> AppResult<()> {
        // Update database counts
        self.update_database_counts().await?;

        // Update data source health
        self.update_data_source_health().await?;

        // Update user session metrics
        self.update_user_metrics().await?;

        // Update memory usage
        self.update_memory_usage().await?;

        // Update database connection pool metrics
        self.update_db_pool_metrics().await?;

        Ok(())
    }

    /// Update database record counts
    async fn update_database_counts(&self) -> AppResult<()> {
        use diesel::prelude::*;
        use crate::schema::*;

        let mut conn = self.pool.get().await?;

        // Count economic series
        let series_count: i64 = economic_series::table
            .count()
            .get_result(&mut conn)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        update_economic_series_count(series_count);

        // Count data points
        let data_points_count: i64 = data_points::table
            .count()
            .get_result(&mut conn)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        update_data_points_count(data_points_count);

        // Count active users (users with recent activity)
        let active_users_count: i64 = users::table
            .filter(users::last_login.gt(chrono::Utc::now() - chrono::Duration::days(30)))
            .count()
            .get_result(&mut conn)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        update_active_users_count(active_users_count);

        // Count chart annotations
        let annotations_count: i64 = chart_annotations::table
            .count()
            .get_result(&mut conn)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        update_chart_annotations_count(annotations_count);

        Ok(())
    }

    /// Update data source health metrics
    async fn update_data_source_health(&self) -> AppResult<()> {
        use diesel::prelude::*;
        use crate::schema::*;

        let mut conn = self.pool.get().await?;

        // Check if we have recent successful crawls for each data source
        let healthy_sources: i64 = data_sources::table
            .left_join(crawl_attempts::table.on(
                data_sources::id.eq(crawl_attempts::data_source_id)
                .and(crawl_attempts::status.eq("completed"))
                .and(crawl_attempts::created_at.gt(chrono::Utc::now() - chrono::Duration::hours(24)))
            ))
            .filter(crawl_attempts::id.is_not_null())
            .count()
            .get_result(&mut conn)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let total_sources: i64 = data_sources::table
            .count()
            .get_result(&mut conn)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Set health to 1 if all sources are healthy, 0 otherwise
        let is_healthy = healthy_sources == total_sources && total_sources > 0;
        update_data_source_health(is_healthy);

        Ok(())
    }

    /// Update user session metrics
    async fn update_user_metrics(&self) -> AppResult<()> {
        use diesel::prelude::*;
        use crate::schema::*;

        let mut conn = self.pool.get().await?;

        // Count active user sessions (users with recent activity)
        let active_sessions: i64 = users::table
            .filter(users::last_login.gt(chrono::Utc::now() - chrono::Duration::hours(1)))
            .count()
            .get_result(&mut conn)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        update_user_sessions_active(active_sessions);

        Ok(())
    }

    /// Update memory usage metrics
    async fn update_memory_usage(&self) -> AppResult<()> {
        // Get current memory usage
        let memory_info = sysinfo::System::new_all();
        let memory_usage = memory_info.used_memory() as i64;
        update_memory_usage(memory_usage);

        Ok(())
    }

    /// Update database connection pool metrics
    async fn update_db_pool_metrics(&self) -> AppResult<()> {
        // Note: The actual connection pool metrics would need to be exposed
        // by the database pool implementation. For now, we'll use placeholder values.
        // In a real implementation, you'd want to expose these from your connection pool.

        // Placeholder values - in practice, you'd get these from your pool
        let active_connections = 5; // self.pool.active_connections()
        let idle_connections = 10;  // self.pool.idle_connections()
        let total_connections = 15; // self.pool.total_connections()

        update_db_pool_metrics(active_connections, idle_connections, total_connections);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestContainer;
    use serial_test::serial;
    use std::time::Duration;

    #[tokio::test]
    #[serial]
    async fn test_metrics_service_creation() {
        // Test that metrics service can be created without errors
        let container = TestContainer::new().await;
        let pool = container.pool();

        let service = MetricsService::new(pool);
        assert_eq!(service.update_interval, Duration::from_secs(60));

        let custom_interval = Duration::from_secs(30);
        let service_custom = MetricsService::with_interval(pool, custom_interval);
        assert_eq!(service_custom.update_interval, custom_interval);
    }

    #[tokio::test]
    #[serial]
    async fn test_update_metrics_with_empty_database() {
        // Test that metrics update works with empty database
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();

        let service = MetricsService::new(pool);

        // Should not panic even with empty database
        let result = service.update_metrics().await;
        assert!(result.is_ok(), "Metrics update should succeed even with empty database");
    }

    #[tokio::test]
    #[serial]
    async fn test_update_metrics_with_sample_data() {
        // Test that metrics update works with sample data
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();

        // Add some sample data to test metrics collection
        use crate::models::{DataSource, EconomicSeries, NewEconomicSeries};
        use crate::models::NewDataSource;

        // Create a data source
        let source = NewDataSource {
            name: "Test Source".to_string(),
            base_url: "https://test.example.com".to_string(),
            api_key: Some("test-key".to_string()),
            rate_limit: Some(100),
            description: Some("Test data source".to_string()),
        };
        let data_source = DataSource::create(pool.clone(), &source).await.unwrap();

        // Create some economic series
        for i in 0..5 {
            let series = NewEconomicSeries {
                source_id: data_source.id,
                series_id: format!("TEST{}", i),
                title: format!("Test Series {}", i),
                description: Some(format!("Test description {}", i)),
                frequency: Some("Monthly".to_string()),
                units: Some("Index".to_string()),
                seasonal_adjustment: Some("Seasonally Adjusted".to_string()),
                last_updated: Some(chrono::Utc::now()),
            };
            EconomicSeries::create(pool.clone(), &series).await.unwrap();
        }

        let service = MetricsService::new(pool);

        // Update metrics and verify it works
        let result = service.update_metrics().await;
        assert!(result.is_ok(), "Metrics update should succeed with sample data");

        // Verify that metrics were actually updated by checking the output
        let metrics_output = crate::metrics_enhanced::generate_metrics().unwrap();
        assert!(metrics_output.contains("economic_series_total"));
        assert!(metrics_output.contains("data_points_total"));
    }

    #[tokio::test]
    #[serial]
    async fn test_metrics_service_error_handling() {
        // Test that metrics service handles errors gracefully
        let container = TestContainer::new().await;
        let pool = container.pool();

        let service = MetricsService::new(pool);

        // Test with invalid database connection (by dropping the container)
        drop(container);

        // This should handle the error gracefully
        let result = service.update_metrics().await;
        // The service should handle database errors without panicking
        // (exact error handling depends on implementation)
    }

    #[tokio::test]
    #[serial]
    async fn test_metrics_service_interval_configuration() {
        // Test different interval configurations
        let container = TestContainer::new().await;
        let pool = container.pool();

        // Test default interval
        let service_default = MetricsService::new(pool.clone());
        assert_eq!(service_default.update_interval, Duration::from_secs(60));

        // Test custom interval
        let custom_interval = Duration::from_secs(30);
        let service_custom = MetricsService::with_interval(pool, custom_interval);
        assert_eq!(service_custom.update_interval, custom_interval);

        // Test very short interval for testing
        let test_interval = Duration::from_millis(100);
        let service_test = MetricsService::with_interval(
            service_custom.pool.clone(),
            test_interval
        );
        assert_eq!(service_test.update_interval, test_interval);
    }

    #[tokio::test]
    #[serial]
    async fn test_metrics_collection_completeness() {
        // Test that all expected metrics are collected
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();

        // Add comprehensive test data
        use crate::models::{DataSource, EconomicSeries, DataPoint, NewDataSource, NewEconomicSeries, NewDataPoint};
        use chrono::{Utc, NaiveDate};

        // Create data source
        let source = NewDataSource {
            name: "Comprehensive Test Source".to_string(),
            base_url: "https://comprehensive.test.com".to_string(),
            api_key: Some("comprehensive-key".to_string()),
            rate_limit: Some(1000),
            description: Some("Comprehensive test data source".to_string()),
        };
        let data_source = DataSource::create(pool.clone(), &source).await.unwrap();

        // Create multiple series
        for i in 0..10 {
            let series = NewEconomicSeries {
                source_id: data_source.id,
                series_id: format!("COMPREHENSIVE{}", i),
                title: format!("Comprehensive Test Series {}", i),
                description: Some(format!("Comprehensive test description {}", i)),
                frequency: Some("Monthly".to_string()),
                units: Some("Index".to_string()),
                seasonal_adjustment: Some("Seasonally Adjusted".to_string()),
                last_updated: Some(Utc::now()),
            };
            let created_series = EconomicSeries::create(pool.clone(), &series).await.unwrap();

            // Add data points for each series
            for j in 0..20 {
                let data_point = NewDataPoint {
                    series_id: created_series.id,
                    date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap() + chrono::Duration::days(j as i64 * 30),
                    value: Some(rust_decimal::Decimal::from(j * 10)),
                    metadata: Some(serde_json::json!({"test": true})),
                };
                DataPoint::create(pool.clone(), &data_point).await.unwrap();
            }
        }

        let service = MetricsService::new(pool);

        // Update metrics
        let result = service.update_metrics().await;
        assert!(result.is_ok(), "Metrics update should succeed with comprehensive data");

        // Verify comprehensive metrics output
        let metrics_output = crate::metrics_enhanced::generate_metrics().unwrap();

        // Check that all expected metric types are present
        let expected_metrics = [
            "economic_series_total",
            "data_points_total",
            "data_sources_total",
            "application_uptime_seconds",
            "memory_usage_bytes",
            "db_connections_active",
            "db_connections_idle",
            "db_connections_total",
        ];

        for metric in expected_metrics {
            assert!(
                metrics_output.contains(metric),
                "Metrics output should contain {} metric",
                metric
            );
        }

        // Verify that the counts are reasonable
        assert!(metrics_output.contains("economic_series_total 10"));
        assert!(metrics_output.contains("data_points_total 200")); // 10 series * 20 points each
        assert!(metrics_output.contains("data_sources_total 1"));
    }
}

/// Start the metrics service in a background task
pub async fn start_metrics_service(pool: Arc<DatabasePool>) -> AppResult<()> {
    let metrics_service = MetricsService::new(pool);

    tokio::spawn(async move {
        if let Err(e) = metrics_service.start().await {
            error!("Metrics service failed: {}", e);
        }
    });

    info!("Metrics service started successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_metrics_service_creation() {
        // This test would require a test database setup
        // For now, just test that the service can be created
        let pool = Arc::new(crate::database::create_pool("postgresql://test:test@localhost/test").await.unwrap());
        let service = MetricsService::new(pool);
        assert_eq!(service.update_interval, Duration::from_secs(60));
    }

    #[tokio::test]
    #[serial]
    async fn test_metrics_service_with_custom_interval() {
        let pool = Arc::new(crate::database::create_pool("postgresql://test:test@localhost/test").await.unwrap());
        let custom_interval = Duration::from_secs(30);
        let service = MetricsService::with_interval(pool, custom_interval);
        assert_eq!(service.update_interval, custom_interval);
    }
}
