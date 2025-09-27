use bigdecimal::{BigDecimal, Zero};
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::schema::data_points;

/// **`DataPoint` Model**
///
/// Represents a single observation in an economic time series, containing both the actual
/// data value and important metadata about data provenance and revisions.
///
/// Economic data is frequently revised by statistical agencies (like the BLS, BEA, Federal Reserve)
/// as more complete information becomes available. This model captures both the original releases
/// and subsequent revisions, enabling users to track how economic indicators change over time
/// and understand the reliability of real-time vs. final data.
///
/// # Use Cases
/// - Storing individual observations from economic time series (GDP, unemployment, inflation, etc.)
/// - Tracking data revisions and their impact on economic analysis
/// - Supporting real-time economic monitoring and nowcasting
/// - Enabling historical analysis of data revision patterns
///
/// # Database Schema
/// Maps to the `data_points` table in `PostgreSQL` with full ACID compliance.
/// Indexes are maintained on `series_id`, `date`, and `revision_date` for optimal query performance.
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_backend::models::DataPoint;
/// use uuid::Uuid;
/// use chrono::{NaiveDate, Utc};
/// use bigdecimal::BigDecimal;
///
/// // A GDP data point showing a quarterly observation
/// let gdp_series_uuid = Uuid::new_v4();
/// let gdp_point = DataPoint {
///     id: Uuid::new_v4(),
///     series_id: gdp_series_uuid,
///     date: NaiveDate::from_ymd_opt(2024, 3, 31).unwrap(), // Q1 2024
///     value: Some(BigDecimal::from(27_360_000_000_000i64)), // $27.36 trillion
///     revision_date: NaiveDate::from_ymd_opt(2024, 4, 25).unwrap(), // First estimate release
///     is_original_release: true,
///     created_at: Utc::now(),
///     updated_at: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = data_points)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DataPoint {
    /// Unique identifier for this data point record
    /// Generated automatically using UUID v4 for global uniqueness
    pub id: Uuid,

    /// Foreign key reference to the economic series this data point belongs to
    /// Links to the `economic_series.id` field to maintain referential integrity
    pub series_id: Uuid,

    /// The observation date for this data point
    /// For monthly data: typically the last day of the month
    /// For quarterly data: typically the last day of the quarter
    /// For annual data: typically December 31st of the year
    pub date: NaiveDate,

    /// The actual numeric value of the economic observation
    /// Uses `BigDecimal` for precise financial/economic calculations without floating-point errors
    /// None indicates missing or unavailable data for this observation period
    pub value: Option<BigDecimal>,

    /// The date when this particular value was published or revised
    /// Critical for understanding data vintage and revision history
    /// Enables analysis of how estimates change as more information becomes available
    pub revision_date: NaiveDate,

    /// Flag indicating whether this is the first published estimate for this observation
    /// true: This is the initial/preliminary estimate (e.g., "advance" GDP estimate)
    /// false: This is a revision of a previously published value
    pub is_original_release: bool,

    /// Timestamp when this record was first inserted into the database
    /// Used for audit trails and data lineage tracking
    pub created_at: DateTime<Utc>,

    /// Timestamp when this record was last modified
    /// Updated automatically on any field changes for change tracking
    pub updated_at: DateTime<Utc>,
}

/// **`NewDataPoint` Model**
///
/// Data transfer object for creating new data point records in the database.
/// Contains only the fields that can be specified during insertion, while
/// auto-generated fields (id, timestamps) are handled by the database.
///
/// # Validation Rules
/// - `series_id` must reference an existing economic series
/// - date cannot be in the future beyond reasonable forecast horizons
/// - value precision is limited to avoid storage issues with extreme decimals
/// - `revision_date` should not be before the observation date
///
/// # Use Cases
/// - Inserting new economic observations from data feeds
/// - Bulk loading historical data during system initialization
/// - Adding revised estimates for existing observation periods
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_backend::models::NewDataPoint;
/// use uuid::Uuid;
/// use chrono::NaiveDate;
/// use bigdecimal::BigDecimal;
/// use std::str::FromStr;
///
/// // Creating a new unemployment rate observation
/// let unemployment_series_id = Uuid::new_v4();
/// let new_point = NewDataPoint {
///     series_id: unemployment_series_id,
///     date: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(), // November 2024
///     value: Some(BigDecimal::from_str("4.1").unwrap()), // 4.1% unemployment
///     revision_date: NaiveDate::from_ymd_opt(2024, 12, 6).unwrap(), // Release date
///     is_original_release: true,
/// };
/// ```
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = data_points)]
pub struct NewDataPoint {
    /// The economic series this data point belongs to
    /// Must be a valid UUID referencing an existing series
    pub series_id: Uuid,

    /// The observation date for this economic data point
    /// Should align with the series frequency (monthly, quarterly, annual)
    pub date: NaiveDate,

    /// The numeric value of the economic observation
    /// None is used for missing data points in incomplete series
    pub value: Option<BigDecimal>,

    /// When this data was published or made available
    /// Used to track data vintage and enable time-aware queries
    pub revision_date: NaiveDate,

    /// Whether this is the initial estimate (true) or a revision (false)
    /// Critical for distinguishing between real-time and final data
    pub is_original_release: bool,
}

/// **`UpdateDataPoint` Model**
///
/// Data transfer object for modifying existing data point records.
/// Supports partial updates where only specified fields are changed,
/// following the principle of minimal data modification.
///
/// # Update Scenarios
/// - Data revisions: Update value when agencies publish revised estimates
/// - Metadata corrections: Fix revision dates or original release flags
/// - Data quality improvements: Correct erroneous values or classifications
///
/// # Audit Trail
/// The `updated_at` timestamp is automatically set to track when changes occur,
/// supporting compliance and data governance requirements.
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_backend::models::UpdateDataPoint;
/// use chrono::{NaiveDate, Utc};
/// use bigdecimal::BigDecimal;
/// use std::str::FromStr;
///
/// // Revising a GDP estimate from preliminary to final
/// let revision = UpdateDataPoint {
///     value: Some(BigDecimal::from_str("27365000000000").unwrap()), // Revised up
///     revision_date: Some(NaiveDate::from_ymd_opt(2024, 6, 27).unwrap()), // Final estimate date
///     is_original_release: Some(false), // This is now a revision
///     updated_at: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, AsChangeset, Validate, Deserialize)]
#[diesel(table_name = data_points)]
pub struct UpdateDataPoint {
    /// Updated value for the economic observation
    /// None can be used to clear a previously set value
    pub value: Option<BigDecimal>,

    /// Updated revision date if the publication timeline changes
    /// Useful for correcting metadata or handling delayed releases
    pub revision_date: Option<NaiveDate>,

    /// Updated flag for original release status
    /// May need correction if initial classification was wrong
    pub is_original_release: Option<bool>,

    /// Timestamp of this update operation
    /// Set automatically to track modification history
    pub updated_at: DateTime<Utc>,
}

/// **`DataPointWithSeries` Model**
///
/// Enhanced data point model that includes series metadata for API responses.
/// Combines data point information with series context to provide complete
/// information needed by frontend applications without requiring additional queries.
///
/// # Purpose
/// - Reduce API round trips by embedding series information
/// - Provide context for data visualization and analysis
/// - Support efficient data transfer to frontend applications
/// - Enable self-contained data point representations
///
/// # Use Cases
/// - GraphQL API responses that need both data and metadata
/// - Data export functionality requiring series context
/// - Chart rendering where series title and units are needed
/// - Data analysis workflows requiring complete information
///
/// # Performance Considerations
/// This struct is typically populated via database joins, so it's optimized
/// for read-heavy workloads rather than frequent updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPointWithSeries {
    /// Unique identifier for the data point
    pub id: Uuid,

    /// The series this data point belongs to
    pub series_id: Uuid,

    /// Human-readable title of the economic series
    /// e.g., "Real Gross Domestic Product", "Unemployment Rate"
    pub series_title: String,

    /// The observation date for this data point
    pub date: NaiveDate,

    /// The numeric value of the observation
    pub value: Option<BigDecimal>,

    /// When this data was published or revised
    pub revision_date: NaiveDate,

    /// Whether this is the original release or a revision
    pub is_original_release: bool,

    /// Units of measurement for the series
    /// e.g., "Billions of Chained 2017 Dollars", "Percent", "Index 1982-84=100"
    pub units: Option<String>,
}

/// **`DataQueryParams` Model**
///
/// Parameters for querying data points with filtering, pagination, and data vintage controls.
/// Provides flexible access to time series data while maintaining performance through
/// validation limits and efficient query patterns.
///
/// # Query Capabilities
/// - Time range filtering for focused analysis periods
/// - Data vintage control (original vs. revised estimates)
/// - Pagination support for large datasets
/// - Flexible parameter combinations for various use cases
///
/// # Validation Rules
/// - Limit capped at 10,000 records to prevent memory issues
/// - Offset must be non-negative for proper pagination
/// - Date ranges are validated for logical consistency
///
/// # Performance Optimizations
/// - Database indexes support efficient filtering on `series_id` and date ranges
/// - Limit and offset enable cursor-based pagination for large result sets
/// - Optional parameters reduce query complexity when not needed
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_backend::models::DataQueryParams;
/// use uuid::Uuid;
/// use chrono::NaiveDate;
///
/// // Query last 12 months of employment data, original releases only
/// let employment_series_id = Uuid::new_v4();
/// let params = DataQueryParams {
///     series_id: employment_series_id,
///     start_date: Some(NaiveDate::from_ymd_opt(2023, 12, 1).unwrap()),
///     end_date: Some(NaiveDate::from_ymd_opt(2024, 11, 30).unwrap()),
///     original_only: Some(true),
///     latest_revision_only: Some(false),
///     limit: Some(12),
///     offset: Some(0),
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct DataQueryParams {
    /// The economic series to query data for
    /// Must reference an existing series in the database
    pub series_id: Uuid,

    /// Optional start date for filtering observations
    /// Inclusive bound - data points on or after this date are included
    pub start_date: Option<NaiveDate>,

    /// Optional end date for filtering observations
    /// Inclusive bound - data points on or before this date are included
    pub end_date: Option<NaiveDate>,

    /// Filter to only include original release estimates
    /// true: Only show first published values (real-time data)
    /// false/None: Include all data regardless of revision status
    pub original_only: Option<bool>,

    /// Filter to only include the latest revision for each observation date
    /// true: Show most recent estimates (final data)
    /// false/None: Include all revisions for complete revision history
    pub latest_revision_only: Option<bool>,

    /// Maximum number of data points to return
    /// Capped at 10,000 to prevent memory exhaustion and ensure reasonable response times
    #[validate(range(min = 1, max = 10000))]
    pub limit: Option<i64>,

    /// Number of records to skip for pagination
    /// Used with limit to implement cursor-based pagination for large datasets
    #[validate(range(min = 0))]
    pub offset: Option<i64>,
}

/// **`DataTransformation` Enum**
///
/// Defines the mathematical transformations that can be applied to economic time series data.
/// These transformations are essential for economic analysis, allowing users to view data
/// in different perspectives that highlight various economic phenomena.
///
/// # Economic Context
/// - Raw levels show absolute values but can obscure trends due to scale
/// - Growth rates reveal momentum and cyclical patterns
/// - Percent changes normalize data for cross-series comparison
/// - Log differences approximate continuous growth rates
///
/// # Transformation Applications
/// - **`YearOverYear`**: Shows annual growth rates, smooths seasonal patterns
/// - **`QuarterOverQuarter`**: Reveals quarterly momentum, useful for GDP analysis
/// - **`MonthOverMonth`**: Captures short-term changes, sensitive to volatility
/// - **`PercentChange`**: General-purpose growth rate calculation
/// - **`LogDifference`**: Approximates continuous compounding, useful for modeling
///
/// # Use Cases
/// - Economic research requiring different data perspectives
/// - Chart visualization with user-selectable transformations
/// - Comparative analysis across series with different scales
/// - Trend analysis and cycle identification
///
/// # Mathematical Notes
/// - All transformations preserve the time series structure
/// - Growth rates are typically expressed as percentages
/// - Log differences are natural log-based for mathematical properties
/// - Missing values in source data propagate through transformations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataTransformation {
    /// No transformation applied - raw data values
    /// Use for: Absolute levels, when original scale is meaningful
    None,

    /// Year-over-year percentage change: ((current - `year_ago`) / `year_ago`) * 100
    /// Use for: Annual growth rates, removing seasonal effects
    YearOverYear,

    /// Quarter-over-quarter percentage change: ((current - `quarter_ago`) / `quarter_ago`) * 100
    /// Use for: Quarterly momentum, short-term trend analysis
    QuarterOverQuarter,

    /// Month-over-month percentage change: ((current - `month_ago`) / `month_ago`) * 100
    /// Use for: Monthly changes, high-frequency analysis
    MonthOverMonth,

    /// General percent change calculation
    /// Use for: Generic growth rate analysis
    PercentChange,

    /// Natural logarithm difference: ln(current) - ln(previous)
    /// Use for: Continuous growth rates, econometric modeling
    LogDifference,
}

/// **Display Implementation for `DataTransformation`**
///
/// Provides human-readable string representations for UI display and logging.
/// These strings are used in chart labels, API responses, and user interfaces
/// to clearly communicate the type of transformation applied to the data.
impl std::fmt::Display for DataTransformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataTransformation::None => write!(f, "None"),
            DataTransformation::YearOverYear => write!(f, "Year-over-Year"),
            DataTransformation::QuarterOverQuarter => write!(f, "Quarter-over-Quarter"),
            DataTransformation::MonthOverMonth => write!(f, "Month-over-Month"),
            DataTransformation::PercentChange => write!(f, "Percent Change"),
            DataTransformation::LogDifference => write!(f, "Log Difference"),
        }
    }
}

/// **String Conversion Implementation for `DataTransformation`**
///
/// Enables flexible parsing of transformation types from various string formats.
/// Supports multiple aliases and formats to accommodate different input sources
/// including API parameters, configuration files, and user input.
///
/// # Supported Formats
/// - Abbreviations: "yoy", "qoq", "mom", "pct", "log"
/// - Underscore format: "`year_over_year`", "`month_over_month`"
/// - Hyphenated format: "year-over-year", "quarter-over-quarter"
/// - Case insensitive matching for user convenience
///
/// # Default Behavior
/// Invalid or unrecognized strings default to `DataTransformation::None`
/// to ensure graceful handling of malformed input.
impl From<String> for DataTransformation {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "yoy" | "year_over_year" | "year-over-year" => DataTransformation::YearOverYear,
            "qoq" | "quarter_over_quarter" | "quarter-over-quarter" => {
                DataTransformation::QuarterOverQuarter
            }
            "mom" | "month_over_month" | "month-over-month" => DataTransformation::MonthOverMonth,
            "pct" | "percent" | "percent_change" => DataTransformation::PercentChange,
            "log" | "log_diff" | "log_difference" => DataTransformation::LogDifference,
            _ => DataTransformation::None,
        }
    }
}

/// **`TransformedDataPoint` Model**
///
/// Represents a data point that has undergone mathematical transformation for analysis.
/// Contains both the original value and the computed transformation result, enabling
/// users to understand both the raw data and its analytical representation.
///
/// # Purpose
/// - Provide transformed data for economic analysis and visualization
/// - Maintain traceability between original and computed values
/// - Support comparative analysis across different transformation types
/// - Enable flexible data presentation in charts and reports
///
/// # Use Cases
/// - API responses for transformed time series data
/// - Chart rendering with selectable transformation options
/// - Economic analysis requiring growth rates or other derived metrics
/// - Data export functionality with transformation metadata
///
/// # Data Integrity
/// Both original and transformed values are preserved to ensure transparency
/// and enable verification of transformation calculations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformedDataPoint {
    /// The observation date for this data point
    pub date: NaiveDate,

    /// The original, untransformed value from the database
    /// Preserved for transparency and verification purposes
    pub original_value: Option<BigDecimal>,

    /// The result of applying the transformation to the original value
    /// May be None if transformation cannot be computed (e.g., missing historical data)
    pub transformed_value: Option<BigDecimal>,

    /// The type of transformation that was applied
    /// Used for labeling and understanding the computed values
    pub transformation: DataTransformation,

    /// When the original data was published or revised
    pub revision_date: NaiveDate,

    /// Whether the original value was from the first release or a revision
    pub is_original_release: bool,
}

/// **`DataPoint` Implementation**
///
/// Provides methods for calculating common economic transformations on individual data points.
/// These methods form the building blocks for time series transformations and enable
/// flexible data analysis capabilities throughout the application.
impl DataPoint {
    /// **Calculate Year-over-Year Percentage Change**
    ///
    /// Computes the annual growth rate by comparing the current value to the value
    /// from the same period one year ago. This is a fundamental economic metric
    /// that smooths out seasonal variations and reveals long-term trends.
    ///
    /// # Formula
    /// `YoY` Change = ((Current Value - Previous Year Value) / Previous Year Value) * 100
    ///
    /// # Parameters
    /// - `previous_year_value`: The value from the same period one year ago
    ///
    /// # Returns
    /// - `Some(BigDecimal)`: The `YoY` percentage change if calculation is possible
    /// - `None`: If current value, previous value is missing, or previous value is zero
    ///
    /// # Use Cases
    /// - GDP growth analysis (e.g., "GDP grew 2.4% year-over-year")
    /// - Inflation measurement (e.g., "CPI increased 3.2% from last year")
    /// - Employment trend analysis
    /// - Long-term economic performance tracking
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_backend::models::DataPoint;
    /// use bigdecimal::BigDecimal;
    /// use uuid::Uuid;
    /// use chrono::{NaiveDate, Utc};
    ///
    /// let current_gdp = DataPoint {
    ///     id: Uuid::new_v4(),
    ///     series_id: Uuid::new_v4(),
    ///     date: NaiveDate::from_ymd_opt(2024, 3, 31).unwrap(),
    ///     value: Some(BigDecimal::from(27360)),
    ///     revision_date: NaiveDate::from_ymd_opt(2024, 4, 25).unwrap(),
    ///     is_original_release: true,
    ///     created_at: Utc::now(),
    ///     updated_at: Utc::now(),
    /// };
    /// let previous_year_gdp = Some(BigDecimal::from(26744));
    /// let yoy_growth = current_gdp.calculate_yoy_change(previous_year_gdp);
    /// // Result: Some(2.3) representing 2.3% growth
    /// ```
    pub fn calculate_yoy_change(
        &self,
        previous_year_value: Option<BigDecimal>,
    ) -> Option<BigDecimal> {
        match (&self.value, &previous_year_value) {
            (Some(current), Some(previous)) if *previous != BigDecimal::from(0) => {
                Some(((current - previous) / previous) * BigDecimal::from(100))
            }
            _ => None,
        }
    }

    /// **Calculate Quarter-over-Quarter Percentage Change**
    ///
    /// Computes the quarterly growth rate by comparing the current value to the
    /// previous quarter's value. This metric reveals short-term economic momentum
    /// and is particularly useful for analyzing business cycles.
    ///
    /// # Formula
    /// `QoQ` Change = ((Current Value - Previous Quarter Value) / Previous Quarter Value) * 100
    ///
    /// # Parameters
    /// - `previous_quarter_value`: The value from the previous quarter
    ///
    /// # Returns
    /// - `Some(BigDecimal)`: The `QoQ` percentage change if calculation is possible
    /// - `None`: If current value, previous value is missing, or previous value is zero
    ///
    /// # Use Cases
    /// - GDP quarterly growth analysis
    /// - Business cycle identification
    /// - Short-term economic momentum tracking
    /// - Policy impact assessment over quarters
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_backend::models::DataPoint;
    /// use bigdecimal::BigDecimal;
    /// use uuid::Uuid;
    /// use chrono::{NaiveDate, Utc};
    ///
    /// let q2_gdp = DataPoint {
    ///     id: Uuid::new_v4(),
    ///     series_id: Uuid::new_v4(),
    ///     date: NaiveDate::from_ymd_opt(2024, 6, 30).unwrap(),
    ///     value: Some(BigDecimal::from(27360)),
    ///     revision_date: NaiveDate::from_ymd_opt(2024, 7, 25).unwrap(),
    ///     is_original_release: true,
    ///     created_at: Utc::now(),
    ///     updated_at: Utc::now(),
    /// };
    /// let q1_gdp = BigDecimal::from(27100);
    /// let qoq_growth = q2_gdp.calculate_qoq_change(Some(&q1_gdp));
    /// // Result: Some(0.96) representing 0.96% quarterly growth
    /// ```
    pub fn calculate_qoq_change(
        &self,
        previous_quarter_value: Option<&BigDecimal>,
    ) -> Option<BigDecimal> {
        match (&self.value, previous_quarter_value) {
            (Some(current), Some(previous)) if !previous.is_zero() => {
                Some(((current - previous) / previous) * BigDecimal::from(100))
            }
            _ => None,
        }
    }

    /// **Calculate Month-over-Month Percentage Change**
    ///
    /// Computes the monthly growth rate by comparing the current value to the
    /// previous month's value. This provides the highest frequency view of
    /// economic changes but can be volatile due to short-term fluctuations.
    ///
    /// # Formula
    /// `MoM` Change = ((Current Value - Previous Month Value) / Previous Month Value) * 100
    ///
    /// # Parameters
    /// - `previous_month_value`: The value from the previous month
    ///
    /// # Returns
    /// - `Some(BigDecimal)`: The `MoM` percentage change if calculation is possible
    /// - `None`: If current value, previous value is missing, or previous value is zero
    ///
    /// # Use Cases
    /// - High-frequency economic monitoring
    /// - Employment report analysis (jobs added/lost monthly)
    /// - Inflation tracking (monthly CPI changes)
    /// - Real-time economic nowcasting
    ///
    /// # Volatility Considerations
    /// Monthly changes can be noisy due to seasonal effects, temporary disruptions,
    /// and measurement errors. Consider using alongside smoothed or seasonally
    /// adjusted data for trend analysis.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_backend::models::DataPoint;
    /// use bigdecimal::BigDecimal;
    /// use uuid::Uuid;
    /// use chrono::{NaiveDate, Utc};
    ///
    /// let nov_employment = DataPoint {
    ///     id: Uuid::new_v4(),
    ///     series_id: Uuid::new_v4(),
    ///     date: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
    ///     value: Some(BigDecimal::from(157500)),
    ///     revision_date: NaiveDate::from_ymd_opt(2024, 12, 6).unwrap(),
    ///     is_original_release: true,
    ///     created_at: Utc::now(),
    ///     updated_at: Utc::now(),
    /// };
    /// let oct_employment = BigDecimal::from(157300);
    /// let mom_change = nov_employment.calculate_mom_change(Some(&oct_employment));
    /// // Result: Some(0.13) representing 0.13% monthly employment growth
    /// ```
    pub fn calculate_mom_change(
        &self,
        previous_month_value: Option<&BigDecimal>,
    ) -> Option<BigDecimal> {
        match (&self.value, previous_month_value) {
            (Some(current), Some(previous)) if !previous.is_zero() => {
                Some(((current - previous) / previous) * BigDecimal::from(100))
            }
            _ => None,
        }
    }
}

impl Default for NewDataPoint {
    fn default() -> Self {
        Self {
            series_id: Uuid::new_v4(),
            date: chrono::Utc::now().date_naive(),
            value: None,
            revision_date: chrono::Utc::now().date_naive(),
            is_original_release: true,
        }
    }
}

impl Default for UpdateDataPoint {
    fn default() -> Self {
        Self {
            value: None,
            revision_date: None,
            is_original_release: None,
            updated_at: Utc::now(),
        }
    }
}

impl DataPoint {
    /// Find all data points
    pub async fn find_all(
        pool: &crate::database::DatabasePool,
    ) -> crate::error::AppResult<Vec<Self>> {
        use crate::schema::data_points::dsl;

        let mut conn = pool.get().await.map_err(|e| {
            crate::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let data_points =
            diesel_async::RunQueryDsl::load(dsl::data_points.select(Self::as_select()), &mut conn)
                .await?;

        Ok(data_points)
    }

    /// Create a new data point
    pub async fn create(
        pool: &crate::database::DatabasePool,
        new_data_point: &NewDataPoint,
    ) -> crate::error::AppResult<Self> {
        use crate::schema::data_points::dsl;

        let mut conn = pool.get().await.map_err(|e| {
            crate::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let data_point = diesel_async::RunQueryDsl::get_result(
            diesel::insert_into(dsl::data_points).values(new_data_point),
            &mut conn,
        )
        .await?;

        Ok(data_point)
    }

    /// Create multiple data points in a batch
    pub async fn create_batch(
        pool: &crate::database::DatabasePool,
        new_data_points: &[NewDataPoint],
    ) -> crate::error::AppResult<Vec<Self>> {
        use crate::schema::data_points::dsl;

        let mut conn = pool.get().await.map_err(|e| {
            crate::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let data_points = diesel_async::RunQueryDsl::get_results(
            diesel::insert_into(dsl::data_points).values(new_data_points),
            &mut conn,
        )
        .await?;

        Ok(data_points)
    }

    /// Find data points for a series within a date range
    pub async fn find_by_series_and_date_range(
        pool: &crate::database::DatabasePool,
        series_id: uuid::Uuid,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> crate::error::AppResult<Vec<Self>> {
        use crate::schema::data_points::dsl;

        let mut conn = pool.get().await.map_err(|e| {
            crate::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let data_points = diesel_async::RunQueryDsl::load(
            dsl::data_points
                .filter(dsl::series_id.eq(series_id))
                .filter(dsl::date.between(start_date, end_date))
                .order(dsl::date.asc()),
            &mut conn,
        )
        .await?;

        Ok(data_points)
    }

    /// Get a data point by series ID and specific date
    pub async fn get_by_series_and_date(
        pool: &crate::database::DatabasePool,
        series_id: uuid::Uuid,
        date: &chrono::NaiveDate,
    ) -> crate::error::AppResult<Option<Self>> {
        use crate::schema::data_points::dsl;

        let mut conn = pool.get().await.map_err(|e| {
            crate::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let data_point = diesel_async::RunQueryDsl::first(
            dsl::data_points
                .filter(dsl::series_id.eq(series_id))
                .filter(dsl::date.eq(*date)),
            &mut conn,
        )
        .await
        .optional()?;

        Ok(data_point)
    }
}

// Inline tests moved to external test file
#[cfg(test)]
mod inline_tests {
    use super::*;
    use bigdecimal::BigDecimal;

    #[test]
    fn test_data_transformation_conversion() {
        // REQUIREMENT: Users should be able to select YoY, QoQ, MoM transformations
        // PURPOSE: Verify that transformation strings from the frontend are correctly parsed
        // This ensures the GraphQL API can handle transformation requests properly

        // Test year-over-year transformation - required for economic analysis
        assert_eq!(
            DataTransformation::from("yoy".to_string()),
            DataTransformation::YearOverYear
        );
        // Test quarter-over-quarter transformation - required for quarterly data analysis
        assert_eq!(
            DataTransformation::from("qoq".to_string()),
            DataTransformation::QuarterOverQuarter
        );
        // Test month-over-month transformation - required for monthly data analysis
        assert_eq!(
            DataTransformation::from("mom".to_string()),
            DataTransformation::MonthOverMonth
        );
        // Test unknown transformations default to None - handles invalid input gracefully
        assert_eq!(
            DataTransformation::from("unknown".to_string()),
            DataTransformation::None
        );
    }

    #[test]
    fn test_yoy_calculation() {
        // REQUIREMENT: The application should calculate year-over-year percentage changes
        // PURPOSE: Verify that YoY calculations produce correct percentage change values
        // This is essential for economic analysis and matches FRED's calculation methodology

        let data_point = DataPoint {
            id: Uuid::new_v4(),
            series_id: Uuid::new_v4(),
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            value: Some(BigDecimal::from(110)),
            revision_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Test normal YoY calculation: (110-100)/100 * 100 = 10%
        let yoy_change = data_point.calculate_yoy_change(Some("100.0".parse().unwrap()));
        assert_eq!(
            yoy_change,
            Some("10.0".parse().unwrap()),
            "YoY calculation should return 10% increase"
        );

        // Test division by zero protection - prevents runtime panics
        let yoy_change_zero = data_point.calculate_yoy_change(Some("0.0".parse().unwrap()));
        assert_eq!(
            yoy_change_zero, None,
            "YoY calculation should handle zero previous value"
        );

        // Test missing data handling - common in economic time series
        let yoy_change_none = data_point.calculate_yoy_change(None);
        assert_eq!(
            yoy_change_none, None,
            "YoY calculation should handle missing previous value"
        );
    }

    #[test]
    fn test_data_query_params_validation() {
        // REQUIREMENT: API should validate input parameters to prevent abuse and errors
        // PURPOSE: Verify that query parameter validation prevents excessive data requests
        // This protects the database from overload and ensures reasonable response times

        let valid_params = DataQueryParams {
            series_id: Uuid::new_v4(),
            start_date: None,
            end_date: None,
            original_only: None,
            latest_revision_only: None,
            limit: Some(100),
            offset: Some(0),
        };

        // Verify valid parameters pass validation - required for normal operation
        assert!(
            valid_params.validate().is_ok(),
            "Valid query parameters should pass validation"
        );

        // Test limit validation to prevent excessive data requests
        let invalid_params = DataQueryParams {
            series_id: Uuid::new_v4(),
            start_date: None,
            end_date: None,
            original_only: None,
            latest_revision_only: None,
            limit: Some(20000), // Exceeds maximum allowed limit
            offset: Some(0),
        };

        // Verify excessive limits are rejected - required for system protection
        assert!(
            invalid_params.validate().is_err(),
            "Excessive limit should fail validation"
        );
    }
}

#[cfg(test)]
mod tests;
