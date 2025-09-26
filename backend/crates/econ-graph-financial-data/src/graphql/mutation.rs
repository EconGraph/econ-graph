use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::database::Database;
use crate::models::input::{CreateDataPointInput, CreateEconomicSeriesInput};
use crate::models::{DataPoint, EconomicSeries};

pub struct Mutation;

#[Object]
impl Mutation {
    /// Create a new economic series
    async fn create_series(
        &self,
        ctx: &Context<'_>,
        input: CreateEconomicSeriesInput,
    ) -> Result<EconomicSeries> {
        let database = ctx.data::<Database>()?;

        // Convert input to full model
        let series = EconomicSeries {
            id: Uuid::new_v4(),
            source_id: input.source_id,
            external_id: input.external_id,
            title: input.title,
            description: input.description,
            units: input.units,
            frequency: input.frequency,
            seasonal_adjustment: input.seasonal_adjustment,
            start_date: input.start_date,
            end_date: input.end_date,
            is_active: input.is_active,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        database
            .create_series(series.clone())
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(series)
    }

    /// Create data points for a series
    async fn create_data_points(
        &self,
        ctx: &Context<'_>,
        inputs: Vec<CreateDataPointInput>,
    ) -> Result<Vec<DataPoint>> {
        let database = ctx.data::<Database>()?;

        // Convert inputs to full models
        let points: Vec<DataPoint> = inputs
            .into_iter()
            .map(|input| DataPoint {
                id: Uuid::new_v4(),
                series_id: input.series_id,
                date: input.date,
                value: input.value,
                revision_date: input.revision_date,
                is_original_release: input.is_original_release,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .collect();

        database
            .create_data_points(points.clone())
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(points)
    }
}
