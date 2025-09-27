use async_graphql::{Context, Object, Result};
use chrono::NaiveDate;
use std::time::Instant;
use uuid::Uuid;

use crate::database::Database;
use crate::models::{DataPoint, EconomicSeries};
use crate::monitoring::{HealthChecker, MetricsCollector};

pub struct Query;

#[Object]
impl Query {
    /// Get a specific economic series by ID
    async fn series(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<EconomicSeries>> {
        let start_time = Instant::now();
        let database = ctx.data::<Database>()?;
        let metrics = ctx.data::<MetricsCollector>()?;

        let result = database
            .get_series(id)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()));

        let duration = start_time.elapsed();
        let success = result.is_ok();

        metrics
            .record_graphql_operation("query_series", duration, success)
            .await;

        result
    }

    /// Get data points for a series within a date range
    async fn data_points(
        &self,
        ctx: &Context<'_>,
        series_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<DataPoint>> {
        let start_time = Instant::now();
        let database = ctx.data::<Database>()?;
        let metrics = ctx.data::<MetricsCollector>()?;

        let result = database
            .get_data_points(series_id, start_date, end_date)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()));

        let duration = start_time.elapsed();
        let success = result.is_ok();

        metrics
            .record_graphql_operation("query_data_points", duration, success)
            .await;

        result
    }

    /// List all economic series
    async fn list_series(&self, ctx: &Context<'_>) -> Result<Vec<EconomicSeries>> {
        let start_time = Instant::now();
        let database = ctx.data::<Database>()?;
        let metrics = ctx.data::<MetricsCollector>()?;

        let result = database
            .list_series()
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()));

        let duration = start_time.elapsed();
        let success = result.is_ok();

        metrics
            .record_graphql_operation("query_list_series", duration, success)
            .await;

        result
    }

    /// Health check endpoint
    async fn health(&self, ctx: &Context<'_>) -> Result<String> {
        let health_checker = ctx.data::<HealthChecker>()?;
        let overall_health = health_checker.get_overall_health().await;

        match overall_health {
            crate::monitoring::HealthStatus::Healthy => {
                Ok("Financial Data Service is healthy".to_string())
            }
            crate::monitoring::HealthStatus::Degraded => {
                Ok("Financial Data Service is degraded".to_string())
            }
            crate::monitoring::HealthStatus::Unhealthy => {
                Ok("Financial Data Service is unhealthy".to_string())
            }
        }
    }

    /// Get catalog statistics
    async fn catalog_stats(&self, ctx: &Context<'_>) -> Result<CatalogStatsResponse> {
        let database = ctx.data::<Database>()?;
        let metrics = ctx.data::<MetricsCollector>()?;

        let start_time = Instant::now();
        let stats = database
            .get_catalog_stats()
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let duration = start_time.elapsed();

        metrics
            .record_graphql_operation("catalog_stats", duration, true)
            .await;

        Ok(CatalogStatsResponse {
            total_series: stats.total_series,
            total_data_points: stats.total_data_points,
            earliest_date: stats.earliest_date,
            latest_date: stats.latest_date,
            last_updated: stats.last_updated,
        })
    }

    /// Find series by date range
    async fn find_series_by_date_range(
        &self,
        ctx: &Context<'_>,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<SeriesInfo>> {
        let database = ctx.data::<Database>()?;
        let metrics = ctx.data::<MetricsCollector>()?;

        let start_time = Instant::now();
        let series_list = database
            .find_series_by_date_range(start_date, end_date)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let duration = start_time.elapsed();

        metrics
            .record_graphql_operation("find_series_by_date_range", duration, true)
            .await;

        Ok(series_list
            .into_iter()
            .map(|series| SeriesInfo {
                id: series.id,
                external_id: series.external_id,
                title: series.title,
                frequency: series.frequency,
                start_date: series.start_date,
                end_date: series.end_date,
                data_points: 0,    // TODO: Get actual count
                completeness: 1.0, // TODO: Get actual completeness
                last_updated: series.updated_at,
            })
            .collect())
    }

    /// Find series by external ID
    async fn find_series_by_external_id(
        &self,
        ctx: &Context<'_>,
        external_id: String,
    ) -> Result<Option<SeriesInfo>> {
        let database = ctx.data::<Database>()?;
        let metrics = ctx.data::<MetricsCollector>()?;

        let start_time = Instant::now();
        let series = database
            .find_series_by_external_id(&external_id)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let duration = start_time.elapsed();

        metrics
            .record_graphql_operation("find_series_by_external_id", duration, true)
            .await;

        Ok(series.map(|series| SeriesInfo {
            id: series.id,
            external_id: series.external_id,
            title: series.title,
            frequency: series.frequency,
            start_date: series.start_date,
            end_date: series.end_date,
            data_points: 0,    // TODO: Get actual count
            completeness: 1.0, // TODO: Get actual completeness
            last_updated: series.updated_at,
        }))
    }

    /// Scan data directory for new files
    async fn scan_data_directory(&self, ctx: &Context<'_>) -> Result<ScanResultResponse> {
        let database = ctx.data::<Database>()?;
        let metrics = ctx.data::<MetricsCollector>()?;

        let start_time = Instant::now();
        let result = database
            .scan_data_directory()
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let duration = start_time.elapsed();

        metrics
            .record_graphql_operation("scan_data_directory", duration, true)
            .await;

        Ok(ScanResultResponse {
            files_discovered: result.files_discovered,
            series_discovered: result.series_discovered,
            partitions_discovered: result.partitions_discovered,
            errors: result.errors,
        })
    }

    /// Validate catalog consistency
    async fn validate_catalog(&self, ctx: &Context<'_>) -> Result<ValidationResultResponse> {
        let database = ctx.data::<Database>()?;
        let metrics = ctx.data::<MetricsCollector>()?;

        let start_time = Instant::now();
        let result = database
            .validate_catalog()
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let duration = start_time.elapsed();

        metrics
            .record_graphql_operation("validate_catalog", duration, true)
            .await;

        Ok(ValidationResultResponse {
            is_valid: result.is_valid,
            missing_files: result
                .missing_files
                .into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            orphaned_files: result
                .orphaned_files
                .into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            inconsistent_metadata: result.inconsistent_metadata,
            errors: result.errors,
        })
    }

    /// Get data freshness information
    async fn data_freshness(&self, ctx: &Context<'_>) -> Result<DataFreshnessResponse> {
        let database = ctx.data::<Database>()?;
        let metrics = ctx.data::<MetricsCollector>()?;

        let start_time = Instant::now();
        let stats = database
            .get_catalog_stats()
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let duration = start_time.elapsed();

        metrics
            .record_graphql_operation("data_freshness", duration, true)
            .await;

        // Calculate freshness metrics
        let time_since_update = chrono::Utc::now().signed_duration_since(stats.last_updated);
        let is_stale = time_since_update.num_seconds() > 86400; // 24 hours in seconds

        Ok(DataFreshnessResponse {
            last_updated: stats.last_updated,
            time_since_update_seconds: time_since_update.num_seconds(),
            is_stale,
            total_series: stats.total_series,
        })
    }
}

/// Response type for catalog statistics
#[derive(async_graphql::SimpleObject)]
pub struct CatalogStatsResponse {
    pub total_series: usize,
    pub total_data_points: u64,
    pub earliest_date: Option<NaiveDate>,
    pub latest_date: Option<NaiveDate>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Response type for series information
#[derive(async_graphql::SimpleObject)]
pub struct SeriesInfo {
    pub id: Uuid,
    pub external_id: String,
    pub title: String,
    pub frequency: String,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub data_points: u64,
    pub completeness: f64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Response type for scan results
#[derive(async_graphql::SimpleObject)]
pub struct ScanResultResponse {
    pub files_discovered: usize,
    pub series_discovered: usize,
    pub partitions_discovered: usize,
    pub errors: Vec<String>,
}

/// Response type for validation results
#[derive(async_graphql::SimpleObject)]
pub struct ValidationResultResponse {
    pub is_valid: bool,
    pub missing_files: Vec<String>,
    pub orphaned_files: Vec<String>,
    pub inconsistent_metadata: Vec<Uuid>,
    pub errors: Vec<String>,
}

/// Response type for data freshness
#[derive(async_graphql::SimpleObject)]
pub struct DataFreshnessResponse {
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub time_since_update_seconds: i64,
    pub is_stale: bool,
    pub total_series: usize,
}
