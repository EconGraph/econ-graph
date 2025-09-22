use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::schema::economic_series;

/// Economic time series model
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = economic_series)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct EconomicSeries {
    pub id: Uuid,
    pub source_id: Uuid,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub units: Option<String>,
    pub frequency: String,
    pub seasonal_adjustment: Option<String>,
    pub last_updated: Option<DateTime<Utc>>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub first_discovered_at: Option<DateTime<Utc>>,
    pub last_crawled_at: Option<DateTime<Utc>>,
    pub first_missing_date: Option<NaiveDate>,
    pub crawl_status: Option<String>,
    pub crawl_error_message: Option<String>,
}

/// New economic series for insertion
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = economic_series)]
pub struct NewEconomicSeries {
    pub source_id: Uuid,
    #[validate(length(min = 1, max = 255))]
    pub external_id: String,
    #[validate(length(min = 1, max = 500))]
    pub title: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    #[validate(length(max = 100))]
    pub units: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub frequency: String,
    #[validate(length(max = 100))]
    pub seasonal_adjustment: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub first_discovered_at: Option<DateTime<Utc>>,
    pub last_crawled_at: Option<DateTime<Utc>>,
    pub first_missing_date: Option<NaiveDate>,
    #[validate(length(max = 50))]
    pub crawl_status: Option<String>,
    pub crawl_error_message: Option<String>,
}

/// Economic series update model
#[derive(Debug, Clone, AsChangeset, Validate, Deserialize)]
#[diesel(table_name = economic_series)]
pub struct UpdateEconomicSeries {
    #[validate(length(min = 1, max = 500))]
    pub title: Option<String>,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    #[validate(length(max = 100))]
    pub units: Option<String>,
    #[validate(length(max = 100))]
    pub seasonal_adjustment: Option<String>,
    pub last_updated: Option<DateTime<Utc>>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: Option<bool>,
    pub updated_at: DateTime<Utc>,
}

/// Series frequency enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SeriesFrequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Annual,
    Irregular,
}

impl std::fmt::Display for SeriesFrequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeriesFrequency::Daily => write!(f, "Daily"),
            SeriesFrequency::Weekly => write!(f, "Weekly"),
            SeriesFrequency::Monthly => write!(f, "Monthly"),
            SeriesFrequency::Quarterly => write!(f, "Quarterly"),
            SeriesFrequency::Annual => write!(f, "Annual"),
            SeriesFrequency::Irregular => write!(f, "Irregular"),
        }
    }
}

impl From<String> for SeriesFrequency {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "daily" | "d" => SeriesFrequency::Daily,
            "weekly" | "w" => SeriesFrequency::Weekly,
            "monthly" | "m" => SeriesFrequency::Monthly,
            "quarterly" | "q" => SeriesFrequency::Quarterly,
            "annual" | "a" | "yearly" | "y" => SeriesFrequency::Annual,
            _ => SeriesFrequency::Irregular,
        }
    }
}

/// Series search parameters
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SeriesSearchParams {
    #[validate(length(min = 1, max = 100))]
    pub query: Option<String>,
    pub source_id: Option<Uuid>,
    pub frequency: Option<String>,
    pub is_active: Option<bool>,
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<i64>,
    #[validate(range(min = 0))]
    pub offset: Option<i64>,
}

impl Default for NewEconomicSeries {
    fn default() -> Self {
        Self {
            source_id: Uuid::new_v4(),
            external_id: String::new(),
            title: String::new(),
            description: None,
            units: None,
            frequency: "Monthly".to_string(),
            seasonal_adjustment: None,
            start_date: None,
            end_date: None,
            is_active: true,
            first_discovered_at: Some(chrono::Utc::now()),
            last_crawled_at: None,
            first_missing_date: None,
            crawl_status: None,
            crawl_error_message: None,
        }
    }
}

impl Default for UpdateEconomicSeries {
    fn default() -> Self {
        Self {
            title: None,
            description: None,
            units: None,
            seasonal_adjustment: None,
            last_updated: Some(Utc::now()),
            start_date: None,
            end_date: None,
            is_active: None,
            updated_at: Utc::now(),
        }
    }
}

impl EconomicSeries {
    /// Find all economic series
    pub async fn find_all(
        pool: &crate::database::DatabasePool,
    ) -> crate::error::AppResult<Vec<Self>> {
        use crate::schema::economic_series::dsl;

        let mut conn = pool.get().await.map_err(|e| {
            crate::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let series = dsl::economic_series
            .select(Self::as_select())
            .load::<Self>(&mut conn)
            .await?;

        Ok(series)
    }

    /// Find economic series by external ID and data source
    pub async fn find_by_external_id(
        pool: &crate::database::DatabasePool,
        external_id: &str,
        data_source_id: uuid::Uuid,
    ) -> crate::error::AppResult<Self> {
        use crate::schema::economic_series::dsl;

        let mut conn = pool.get().await.map_err(|e| {
            crate::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let series = dsl::economic_series
            .filter(dsl::external_id.eq(external_id))
            .filter(dsl::source_id.eq(data_source_id))
            .select(Self::as_select())
            .first::<Self>(&mut conn)
            .await?;

        Ok(series)
    }

    /// Create a new economic series
    pub async fn create(
        pool: &crate::database::DatabasePool,
        new_series: &NewEconomicSeries,
    ) -> crate::error::AppResult<Self> {
        use crate::schema::economic_series::dsl;

        // Validate the new series
        new_series.validate()?;

        let mut conn = pool.get().await.map_err(|e| {
            crate::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let series = diesel::insert_into(dsl::economic_series)
            .values(new_series)
            .returning(Self::as_select())
            .get_result::<Self>(&mut conn)
            .await?;

        Ok(series)
    }

    /// Update an existing economic series
    pub async fn update(
        pool: &crate::database::DatabasePool,
        id: uuid::Uuid,
        update_data: &UpdateEconomicSeries,
    ) -> crate::error::AppResult<Self> {
        use crate::schema::economic_series::dsl;

        let mut conn = pool.get().await.map_err(|e| {
            crate::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let series = diesel::update(dsl::economic_series.filter(dsl::id.eq(id)))
            .set(update_data)
            .get_result::<Self>(&mut conn)
            .await?;

        Ok(series)
    }

    /// Get or create an economic series
    pub async fn get_or_create(
        pool: &crate::database::DatabasePool,
        external_id: &str,
        data_source_id: uuid::Uuid,
        new_series: &NewEconomicSeries,
    ) -> crate::error::AppResult<Self> {
        // Try to find existing series first
        match Self::find_by_external_id(pool, external_id, data_source_id).await {
            Ok(existing) => Ok(existing),
            Err(_) => {
                // Series doesn't exist, create it
                Self::create(pool, new_series).await
            }
        }
    }

    /// Update the date range for a series
    pub async fn update_date_range(
        pool: &crate::database::DatabasePool,
        series_id: uuid::Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> crate::error::AppResult<Self> {
        use crate::schema::economic_series::dsl;

        let mut conn = pool.get().await.map_err(|e| {
            crate::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let update_data = UpdateEconomicSeries {
            start_date: Some(start_date),
            end_date: Some(end_date),
            last_updated: Some(Utc::now()),
            updated_at: Utc::now(),
            ..Default::default()
        };

        let series = diesel::update(dsl::economic_series.filter(dsl::id.eq(series_id)))
            .set(&update_data)
            .returning(Self::as_select())
            .get_result::<Self>(&mut conn)
            .await?;

        Ok(series)
    }
}

#[cfg(test)]
mod _inline_tests {
    use super::*;

    #[test]
    fn test_series_frequency_conversion() {
        // REQUIREMENT: Economic series should support multiple frequency types (Daily, Weekly, Monthly, Quarterly, Annual)
        // PURPOSE: Verify that frequency strings from external APIs are correctly parsed into our enum types
        // This ensures compatibility with FRED and BLS data which use different frequency naming conventions

        // Test standard frequency names - required for FRED compatibility
        assert_eq!(
            SeriesFrequency::from("monthly".to_string()),
            SeriesFrequency::Monthly
        );
        assert_eq!(
            SeriesFrequency::from("quarterly".to_string()),
            SeriesFrequency::Quarterly
        );

        // Test abbreviated forms - required for BLS compatibility
        assert_eq!(
            SeriesFrequency::from("M".to_string()),
            SeriesFrequency::Monthly
        );

        // Test unknown frequencies default to Irregular - handles edge cases gracefully
        assert_eq!(
            SeriesFrequency::from("unknown".to_string()),
            SeriesFrequency::Irregular
        );
    }

    #[test]
    fn test_series_frequency_display() {
        // REQUIREMENT: Frequency information should be displayed consistently in the UI
        // PURPOSE: Verify that frequency enums convert to human-readable strings for display
        // This ensures the frontend shows consistent frequency labels to users

        // Verify display format matches UI requirements
        assert_eq!(SeriesFrequency::Monthly.to_string(), "Monthly");
        assert_eq!(SeriesFrequency::Quarterly.to_string(), "Quarterly");
    }

    #[test]
    fn test_new_economic_series_validation() {
        // REQUIREMENT: All economic series data should be validated before database insertion
        // PURPOSE: Verify that input validation prevents invalid data from being stored
        // This ensures data integrity and prevents database constraint violations

        let series = NewEconomicSeries {
            external_id: "TEST001".to_string(),
            title: "Test Series".to_string(),
            frequency: "Monthly".to_string(),
            ..Default::default()
        };

        // Verify valid series passes validation - required for normal operation
        assert!(
            series.validate().is_ok(),
            "Valid series should pass validation"
        );

        // Test validation failure with empty external_id
        let invalid_series = NewEconomicSeries {
            external_id: "".to_string(), // Too short - violates minimum length constraint
            title: "Test Series".to_string(),
            frequency: "Monthly".to_string(),
            ..Default::default()
        };

        // Verify invalid data is rejected - required for data integrity
        assert!(
            invalid_series.validate().is_err(),
            "Empty external_id should fail validation"
        );
    }
}

#[cfg(test)]
mod tests;
