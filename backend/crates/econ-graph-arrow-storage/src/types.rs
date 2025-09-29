//! Shared types for financial data storage

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Economic series model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicSeries {
    pub id: Uuid,
    pub source_id: String,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub units: Option<String>,
    pub frequency: String,
    pub seasonal_adjustment: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub is_active: bool,
}

/// Data point model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub id: Uuid,
    pub series_id: Uuid,
    pub date: String,
    pub value: Option<f64>,
    pub revision_date: String,
    pub is_original_release: bool,
}
