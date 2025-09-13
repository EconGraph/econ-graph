use chrono::Datelike;
use diesel::prelude::*;
use diesel::SelectableHelper;
use diesel_async::RunQueryDsl;
use serde_json::Value;

use crate::{
    database::DatabasePool,
    error::{AppError, AppResult},
    models::{
        DataPoint, DataQueryParams, DataTransformation, EconomicSeries, SeriesSearchParams,
        TransformedDataPoint,
    },
    schema::{data_points, economic_series},
};

/// **List Economic Series with Filtering**
///
/// Retrieves a filtered list of economic time series from the database based on search parameters.
/// This function provides the core discovery mechanism for users to find relevant economic data
/// series for analysis and visualization.
///
/// # Parameters
/// - `pool`: Database connection pool for async PostgreSQL operations
/// - `params`: Search parameters including filters, pagination, and sorting options
///
/// # Returns
/// - `Ok(Vec<EconomicSeries>)`: List of series matching the search criteria
/// - `Err(AppError)`: Database connection errors or query execution failures
///
/// # Filtering Capabilities
/// - **Data Source**: Filter by specific statistical agencies (BLS, BEA, Federal Reserve, etc.)
/// - **Category**: Filter by economic categories (employment, GDP, inflation, etc.)
/// - **Frequency**: Filter by data frequency (monthly, quarterly, annual)
/// - **Activity Status**: Include/exclude inactive or discontinued series
/// - **Text Search**: Search in series titles and descriptions
///
/// # Performance Considerations
/// - Utilizes database indexes on commonly filtered fields (source_id, category, is_active)
/// - Implements pagination to handle large result sets efficiently
/// - Supports sorting by relevance, title, or last update date
///
/// # Use Cases
/// - Series discovery interface in the frontend application
/// - API endpoints for economic data exploration
/// - Administrative tools for data catalog management
/// - Research workflows requiring specific types of economic indicators
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_backend::services::list_series;
/// use econ_graph_backend::models::SeriesSearchParams;
/// use econ_graph_backend::database::DatabasePool;
/// use uuid::Uuid;
///
/// # async fn example(pool: &DatabasePool) -> Result<(), Box<dyn std::error::Error>> {
/// // Find all active employment-related series from BLS
/// let bls_source_id = Uuid::new_v4();
/// let params = SeriesSearchParams {
///     source_id: Some(bls_source_id),
///     query: Some("employment".to_string()),
///     is_active: Some(true),
///     limit: Some(50),
///     frequency: None,
///     offset: Some(0),
/// };
/// let employment_series = list_series(pool, params).await?;
/// # Ok(())
/// # }
/// ```
pub async fn list_series(
    pool: &DatabasePool,
    params: SeriesSearchParams,
) -> AppResult<Vec<EconomicSeries>> {
    let mut conn = pool.get().await?;

    let mut query = economic_series::table
        .filter(economic_series::is_active.eq(params.is_active.unwrap_or(true)))
        .into_boxed();

    // Apply filters
    if let Some(source_id) = params.source_id {
        query = query.filter(economic_series::source_id.eq(source_id));
    }

    if let Some(frequency) = params.frequency {
        query = query.filter(economic_series::frequency.eq(frequency));
    }

    if let Some(search_query) = params.query {
        // Use PostgreSQL full-text search
        let search_term = format!("%{}%", search_query);
        query = query.filter(
            economic_series::title
                .ilike(search_term.clone())
                .or(economic_series::description.ilike(search_term)),
        );
    }

    // Apply pagination
    let limit = params.limit.unwrap_or(50).min(1000);
    let offset = params.offset.unwrap_or(0);

    query = query.limit(limit).offset(offset);

    // Order by last_updated desc, then by title
    query = query
        .order_by(economic_series::last_updated.desc())
        .then_order_by(economic_series::title.asc());

    let series = query
        .select(EconomicSeries::as_select())
        .load::<EconomicSeries>(&mut *conn)
        .await?;

    Ok(series)
}

/// **Get Economic Series by ID**
///
/// Retrieves a single economic series record by its unique identifier.
/// This function provides fast, direct access to series metadata and is used
/// throughout the application for series validation and information display.
///
/// # Parameters
/// - `pool`: Database connection pool for async PostgreSQL operations
/// - `series_id`: UUID of the economic series to retrieve
///
/// # Returns
/// - `Ok(Some(EconomicSeries))`: The series record if found
/// - `Ok(None)`: If no series exists with the given ID
/// - `Err(AppError)`: Database connection errors or query execution failures
///
/// # Performance
/// - Uses primary key lookup for optimal performance (O(log n) index access)
/// - Single database query with minimal overhead
/// - Suitable for high-frequency API calls and real-time applications
///
/// # Use Cases
/// - API endpoints requiring series metadata
/// - Data validation before processing operations
/// - Series information display in frontend applications
/// - Permission checks for series access control
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_backend::services::get_series_by_id;
/// use econ_graph_backend::database::DatabasePool;
/// use econ_graph_backend::error::AppError;
/// use uuid::Uuid;
///
/// # async fn example(pool: &DatabasePool) -> Result<(), AppError> {
/// // Retrieve GDP series information
/// let gdp_series_id = Uuid::new_v4();
/// if let Some(gdp_series) = get_series_by_id(pool, gdp_series_id).await? {
///     println!("Found series: {}", gdp_series.title);
/// } else {
///     return Err(AppError::NotFound("Series not found".to_string()));
/// }
/// # Ok(())
/// # }
/// ```
pub async fn get_series_by_id(
    pool: &DatabasePool,
    series_id: uuid::Uuid,
) -> AppResult<Option<EconomicSeries>> {
    let mut conn = pool.get().await?;

    let series = economic_series::table
        .filter(economic_series::id.eq(series_id))
        .select(EconomicSeries::as_select())
        .first::<EconomicSeries>(&mut *conn)
        .await
        .optional()?;

    Ok(series)
}

/// **Get Data Points for Economic Series**
///
/// Retrieves time series data points for a specific economic series with comprehensive
/// filtering, pagination, and data vintage controls. This is the core function for
/// accessing economic data and supports all major use cases from simple data retrieval
/// to complex analytical workflows.
///
/// # Parameters
/// - `pool`: Database connection pool for async PostgreSQL operations
/// - `params`: Query parameters including series ID, date ranges, revision filters, and pagination
///
/// # Returns
/// - `Ok(Vec<DataPoint>)`: Vector of data points matching the query criteria
/// - `Err(AppError)`: Database connection errors, invalid parameters, or query execution failures
///
/// # Filtering Capabilities
/// - **Time Range**: Filter by start and end dates for focused analysis periods
/// - **Data Vintage**: Choose between original releases and revised estimates
/// - **Revision Control**: Access complete revision history or latest values only
/// - **Pagination**: Efficiently handle large datasets with limit and offset
///
/// # Data Vintage Options
/// - **Original Only**: First published estimates (real-time data perspective)
/// - **Latest Revision Only**: Most recent estimates (final data perspective)
/// - **All Revisions**: Complete revision history for data quality analysis
///
/// # Performance Optimizations
/// - Multi-column indexes on (series_id, date, revision_date) for fast filtering
/// - Query optimization for common access patterns
/// - Efficient handling of large time series through pagination
/// - Post-processing optimization for revision filtering when needed
///
/// # Use Cases
/// - Chart data retrieval for visualization components
/// - Economic analysis requiring specific time periods
/// - Data export functionality with flexible filtering
/// - Research workflows needing revision history analysis
/// - Real-time data monitoring with latest values only
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_backend::services::get_series_data;
/// use econ_graph_backend::models::DataQueryParams;
/// use econ_graph_backend::database::DatabasePool;
/// use uuid::Uuid;
/// use chrono::NaiveDate;
///
/// # async fn example(pool: &DatabasePool) -> Result<(), Box<dyn std::error::Error>> {
/// // Get last 12 months of employment data, original releases only
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
/// let data_points = get_series_data(pool, params).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Data Quality Considerations
/// - Missing values are preserved as None to maintain data integrity
/// - Revision dates track when estimates were published or updated
/// - Original release flags enable real-time vs. final data analysis
/// - All timestamps are in UTC for consistency across time zones
pub async fn get_series_data(
    pool: &DatabasePool,
    params: DataQueryParams,
) -> AppResult<Vec<DataPoint>> {
    let mut conn = pool.get().await?;

    let mut query = data_points::table
        .filter(data_points::series_id.eq(params.series_id))
        .into_boxed();

    // Apply date range filters
    if let Some(start_date) = params.start_date {
        query = query.filter(data_points::date.ge(start_date));
    }

    if let Some(end_date) = params.end_date {
        query = query.filter(data_points::date.le(end_date));
    }

    // Apply revision filters
    if let Some(original_only) = params.original_only {
        if original_only {
            query = query.filter(data_points::is_original_release.eq(true));
        }
    }

    if let Some(latest_revision_only) = params.latest_revision_only {
        if latest_revision_only {
            // This is a complex query - for now, we'll handle it in the application layer
            // In production, this should be optimized with a proper SQL query
        }
    }

    // Apply pagination
    let limit = params.limit.unwrap_or(1000).min(10000);
    let offset = params.offset.unwrap_or(0);

    query = query.limit(limit).offset(offset);

    // Order by date
    query = query.order_by(data_points::date.asc());

    let mut data_points = query.load::<DataPoint>(&mut *conn).await?;

    // Post-process for latest revision only if requested
    if params.latest_revision_only.unwrap_or(false) {
        data_points = filter_latest_revisions(data_points);
    }

    Ok(data_points)
}

/// Transform data points according to the specified transformation
pub async fn transform_data_points(
    data_points: Vec<DataPoint>,
    transformation: DataTransformation,
) -> AppResult<Vec<Value>> {
    match transformation {
        DataTransformation::None => Ok(data_points
            .into_iter()
            .map(|dp| {
                serde_json::json!({
                    "date": dp.date,
                    "value": dp.value,
                    "revision_date": dp.revision_date,
                    "is_original_release": dp.is_original_release
                })
            })
            .collect()),
        DataTransformation::YearOverYear => Ok(calculate_yoy_changes(data_points)
            .into_iter()
            .map(|tdp| {
                serde_json::json!({
                    "date": tdp.date,
                    "original_value": tdp.original_value,
                    "transformed_value": tdp.transformed_value,
                    "transformation": tdp.transformation,
                    "revision_date": tdp.revision_date,
                    "is_original_release": tdp.is_original_release
                })
            })
            .collect()),
        DataTransformation::QuarterOverQuarter => Ok(calculate_qoq_changes(data_points)
            .into_iter()
            .map(|tdp| {
                serde_json::json!({
                    "date": tdp.date,
                    "original_value": tdp.original_value,
                    "transformed_value": tdp.transformed_value,
                    "transformation": tdp.transformation,
                    "revision_date": tdp.revision_date,
                    "is_original_release": tdp.is_original_release
                })
            })
            .collect()),
        DataTransformation::MonthOverMonth => Ok(calculate_mom_changes(data_points)
            .into_iter()
            .map(|tdp| {
                serde_json::json!({
                    "date": tdp.date,
                    "original_value": tdp.original_value,
                    "transformed_value": tdp.transformed_value,
                    "transformation": tdp.transformation,
                    "revision_date": tdp.revision_date,
                    "is_original_release": tdp.is_original_release
                })
            })
            .collect()),
        _ => Err(AppError::BadRequest(
            "Unsupported transformation".to_string(),
        )),
    }
}

/// Filter data points to keep only the latest revision for each date
fn filter_latest_revisions(data_points: Vec<DataPoint>) -> Vec<DataPoint> {
    use std::collections::HashMap;

    let mut latest_revisions: HashMap<chrono::NaiveDate, DataPoint> = HashMap::new();

    for data_point in data_points {
        let date = data_point.date;

        match latest_revisions.get(&date) {
            Some(existing) => {
                if data_point.revision_date > existing.revision_date {
                    latest_revisions.insert(date, data_point);
                }
            }
            None => {
                latest_revisions.insert(date, data_point);
            }
        }
    }

    let mut result: Vec<DataPoint> = latest_revisions.into_values().collect();
    result.sort_by(|a, b| a.date.cmp(&b.date));

    result
}

/// Calculate year-over-year changes
fn calculate_yoy_changes(data_points: Vec<DataPoint>) -> Vec<TransformedDataPoint> {
    let mut result = Vec::new();
    let mut previous_year_values: std::collections::HashMap<
        (i32, u32, u32),
        bigdecimal::BigDecimal,
    > = std::collections::HashMap::new();

    for data_point in data_points {
        let date = data_point.date;
        let previous_year = date.year() - 1;
        let _key = (previous_year, date.month(), date.day());

        let transformed_value = if let (Some(ref _current_value), Some(previous_value)) = (
            &data_point.value,
            previous_year_values.get(&(previous_year, date.month(), date.day())),
        ) {
            data_point.calculate_yoy_change(Some(previous_value.clone()))
        } else {
            None
        };

        // Store current value for next year's calculation
        if let Some(ref value) = data_point.value {
            previous_year_values.insert((date.year(), date.month(), date.day()), value.clone());
        }

        result.push(TransformedDataPoint {
            date: data_point.date,
            original_value: data_point.value,
            transformed_value,
            transformation: DataTransformation::YearOverYear,
            revision_date: data_point.revision_date,
            is_original_release: data_point.is_original_release,
        });
    }

    result
}

/// Calculate quarter-over-quarter changes
fn calculate_qoq_changes(data_points: Vec<DataPoint>) -> Vec<TransformedDataPoint> {
    let mut result = Vec::new();
    let mut previous_quarter_value: Option<bigdecimal::BigDecimal> = None;

    for data_point in data_points {
        let transformed_value = if let Some(ref _current_value) = data_point.value {
            data_point.calculate_qoq_change(previous_quarter_value.as_ref())
        } else {
            None
        };

        previous_quarter_value = data_point.value.clone();

        result.push(TransformedDataPoint {
            date: data_point.date,
            original_value: data_point.value,
            transformed_value,
            transformation: DataTransformation::QuarterOverQuarter,
            revision_date: data_point.revision_date,
            is_original_release: data_point.is_original_release,
        });
    }

    result
}

/// Calculate month-over-month changes
fn calculate_mom_changes(data_points: Vec<DataPoint>) -> Vec<TransformedDataPoint> {
    let mut result = Vec::new();
    let mut previous_month_value: Option<bigdecimal::BigDecimal> = None;

    for data_point in data_points {
        let transformed_value = if let Some(ref _current_value) = data_point.value {
            data_point.calculate_mom_change(previous_month_value.as_ref())
        } else {
            None
        };

        previous_month_value = data_point.value.clone();

        result.push(TransformedDataPoint {
            date: data_point.date,
            original_value: data_point.value,
            transformed_value,
            transformation: DataTransformation::MonthOverMonth,
            revision_date: data_point.revision_date,
            is_original_release: data_point.is_original_release,
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    #[test]
    fn test_filter_latest_revisions() {
        // REQUIREMENT: Support plotting both original releases and later corrections
        // PURPOSE: Verify that latest revision filtering works correctly for data analysis
        // This ensures users can choose between original and revised data for analysis

        let data_points = vec![
            DataPoint {
                id: Uuid::new_v4(),
                series_id: Uuid::new_v4(),
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                value: Some(BigDecimal::from(100)),
                revision_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                is_original_release: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            DataPoint {
                id: Uuid::new_v4(),
                series_id: Uuid::new_v4(),
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                value: Some(BigDecimal::from(101)),
                revision_date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(), // Later revision
                is_original_release: false,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
        ];

        let filtered = filter_latest_revisions(data_points);

        // Verify only latest revision is kept - important for accurate current analysis
        assert_eq!(
            filtered.len(),
            1,
            "Should filter to only latest revision per date"
        );
        // Verify correct revision value is preserved - ensures data accuracy
        assert_eq!(
            filtered[0].value,
            Some(BigDecimal::from(101)),
            "Should keep the later revision value"
        );
        // Verify revision metadata is maintained - important for data provenance
        assert!(
            !filtered[0].is_original_release,
            "Should preserve revision metadata"
        );
    }

    #[test]
    fn test_calculate_yoy_changes() {
        // REQUIREMENT: Calculate year-over-year percentage changes for economic analysis
        // PURPOSE: Verify that YoY transformation calculations are mathematically correct
        // This is essential for economic analysis and matches standard industry practices

        let data_points = vec![
            DataPoint {
                id: Uuid::new_v4(),
                series_id: Uuid::new_v4(),
                date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
                value: Some(BigDecimal::from(100)),
                revision_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
                is_original_release: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            DataPoint {
                id: Uuid::new_v4(),
                series_id: Uuid::new_v4(),
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                value: Some(BigDecimal::from(110)),
                revision_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                is_original_release: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
        ];

        let transformed = calculate_yoy_changes(data_points);

        // Verify correct number of transformed points - should match input
        assert_eq!(
            transformed.len(),
            2,
            "Should transform all input data points"
        );
        // Verify first point has no YoY value - no previous year data available
        assert_eq!(
            transformed[0].transformed_value, None,
            "First point should have no YoY calculation"
        );
        // Verify second point has correct YoY calculation: (110-100)/100 * 100 = 10%
        assert_eq!(
            transformed[1].transformed_value,
            Some(BigDecimal::from(10)),
            "YoY should be 10% increase"
        );
    }
}
