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
