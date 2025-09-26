use crate::models::DecimalScalar;
use async_graphql::InputObject;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Input type for creating economic series
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct CreateEconomicSeriesInput {
    pub source_id: Uuid,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub units: Option<String>,
    pub frequency: String,
    pub seasonal_adjustment: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
}

/// Input type for creating data points
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct CreateDataPointInput {
    pub series_id: Uuid,
    pub date: NaiveDate,
    pub value: Option<DecimalScalar>,
    pub revision_date: NaiveDate,
    pub is_original_release: bool,
}
