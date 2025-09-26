use async_graphql::{Context, Object, Result};
use chrono::NaiveDate;
use uuid::Uuid;

use crate::database::Database;
use crate::models::{DataPoint, EconomicSeries};

pub struct Query;

#[Object]
impl Query {
    /// Get a specific economic series by ID
    async fn series(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<EconomicSeries>> {
        let database = ctx.data::<Database>()?;
        database
            .get_series(id)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    /// Get data points for a series within a date range
    async fn data_points(
        &self,
        ctx: &Context<'_>,
        series_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<DataPoint>> {
        let database = ctx.data::<Database>()?;
        database
            .get_data_points(series_id, start_date, end_date)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    /// Health check endpoint
    async fn health(&self) -> Result<String> {
        Ok("Financial Data Service is healthy".to_string())
    }
}
