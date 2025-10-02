//! Financial data models
//!
//! TODO: Add GraphQL support once dependencies are resolved

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

pub mod input;

// Custom scalar for Decimal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecimalScalar(pub Decimal);

impl DecimalScalar {
    pub fn new(decimal: Decimal) -> Self {
        Self(decimal)
    }

    pub fn from_str(s: &str) -> Result<Self, rust_decimal::Error> {
        let decimal = Decimal::from_str(s)?;
        Ok(Self(decimal))
    }

    pub fn inner(&self) -> &Decimal {
        &self.0
    }
}

impl From<Decimal> for DecimalScalar {
    fn from(decimal: Decimal) -> Self {
        Self(decimal)
    }
}

impl From<DecimalScalar> for Decimal {
    fn from(scalar: DecimalScalar) -> Self {
        scalar.0
    }
}

// Economic series model
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
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Data point model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub id: Uuid,
    pub series_id: Uuid,
    pub date: NaiveDate,
    pub value: Option<DecimalScalar>,
    pub revision_date: NaiveDate,
    pub is_original_release: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Series metadata model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesMetadata {
    pub id: Uuid,
    pub source_id: String,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub units: Option<String>,
    pub frequency: String,
    pub seasonal_adjustment: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
