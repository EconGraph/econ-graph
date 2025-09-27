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
}
