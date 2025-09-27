use async_graphql::{Scalar, ScalarType, SimpleObject, Value};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

pub mod input;
pub mod validation;

use crate::models::validation::{
    validate_date_range, validate_decimal_value, validate_external_id, validate_frequency,
    validate_not_future_date, validate_seasonal_adjustment, validate_string_length, Validate,
};
use anyhow::Result;

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

impl Validate for EconomicSeries {
    fn validate(&self) -> Result<()> {
        // Validate external ID format
        validate_external_id(&self.external_id)?;

        // Validate string lengths
        validate_string_length("title", &self.title, 1, 500)?;
        if let Some(ref description) = self.description {
            validate_string_length("description", description, 1, 2000)?;
        }
        if let Some(ref units) = self.units {
            validate_string_length("units", units, 1, 100)?;
        }

        // Validate frequency
        validate_frequency(&self.frequency)?;

        // Validate seasonal adjustment if provided
        if let Some(ref adjustment) = self.seasonal_adjustment {
            validate_seasonal_adjustment(adjustment)?;
        }

        // Validate date range
        validate_date_range(self.start_date, self.end_date)?;

        // Validate dates are not in the future
        if let Some(date) = self.start_date {
            validate_not_future_date(date)?;
        }
        if let Some(date) = self.end_date {
            validate_not_future_date(date)?;
        }

        Ok(())
    }
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

impl Validate for DataPoint {
    fn validate(&self) -> Result<()> {
        // Validate dates are not in the future
        validate_not_future_date(self.date)?;
        validate_not_future_date(self.revision_date)?;

        // Validate that revision date is not before the data date
        if self.revision_date < self.date {
            return Err(anyhow::anyhow!(
                "Revision date {} cannot be before data date {}",
                self.revision_date,
                self.date
            ));
        }

        // Validate decimal value if present
        if let Some(ref value) = self.value {
            validate_decimal_value(&value.0)?;
        }

        Ok(())
    }
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

impl Validate for DataSource {
    fn validate(&self) -> Result<()> {
        // Validate string lengths
        validate_string_length("name", &self.name, 1, 200)?;
        if let Some(ref description) = self.description {
            validate_string_length("description", description, 1, 1000)?;
        }
        if let Some(ref url) = self.url {
            validate_string_length("url", url, 1, 500)?;
            // Basic URL validation
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err(anyhow::anyhow!("URL must start with http:// or https://"));
            }
        }

        Ok(())
    }
}
