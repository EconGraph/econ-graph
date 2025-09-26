use async_graphql::{Scalar, ScalarType, SimpleObject, Value};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

pub mod input;

// Custom scalar for Decimal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecimalScalar(pub Decimal);

#[Scalar]
impl ScalarType for DecimalScalar {
    fn parse(value: Value) -> async_graphql::InputValueResult<Self> {
        match value {
            Value::String(s) => {
                let decimal = Decimal::from_str(&s)?;
                Ok(DecimalScalar(decimal))
            }
            Value::Number(n) => {
                let decimal = Decimal::from_str(&n.to_string())?;
                Ok(DecimalScalar(decimal))
            }
            _ => Err(async_graphql::InputValueError::custom(
                "Invalid decimal value",
            )),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.to_string())
    }
}

/// Clean data model for economic series - NO crawler fields
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct EconomicSeries {
    pub id: Uuid,
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Clean data points - only validated data
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
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

/// Source information for economic series
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct DataSource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
