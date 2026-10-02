use chrono::Datelike;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::{Array, Bool, Text};
use diesel::SelectableHelper;
use diesel_async::RunQueryDsl;
use serde_json::Value;

use econ_graph_core::{
    database::DatabasePool,
    error::{AppError, AppResult},
    models::{
        economic_series::SeriesFrequency, DataPoint, DataQueryParams, DataTransformation,
        EconomicSeries, SeriesSearchParams, TransformedDataPoint,
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
/// use econ_graph_services::series_service::list_series;
/// use econ_graph_core::models::SeriesSearchParams;
/// use econ_graph_core::database::DatabasePool;
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
    let mut conn = pool.get().await.map_err(|e| {
        econ_graph_core::error::AppError::DatabaseError(format!(
            "Failed to get database connection: {}",
            e
        ))
    })?;

    let mut query = economic_series::table
        .filter(economic_series::is_active.eq(params.is_active.unwrap_or(true)))
        .filter(economic_series::end_date.is_not_null())
        .into_boxed();

    // Apply filters
    if let Some(source_id) = params.source_id {
        query = query.filter(economic_series::source_id.eq(source_id));
    }

    if let Some(frequency) = params.frequency {
        // Sources store their own frequency text verbatim (FRED: "Weekly, Ending Friday", BLS:
        // "Semi-Annual", ...), so an exact match misses most rows; match the same prefixes
        // `SeriesFrequency` classifies raw text by instead.
        let patterns = SeriesFrequency::from(frequency).sql_like_patterns();
        query = query.filter(
            sql::<Bool>("frequency ILIKE ANY(")
                .bind::<Array<Text>, _>(patterns)
                .sql(")"),
        );
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
/// use econ_graph_services::series_service::get_series_by_id;
/// use econ_graph_core::database::DatabasePool;
/// use econ_graph_core::error::AppError;
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
    let mut conn = pool.get().await.map_err(|e| {
        econ_graph_core::error::AppError::DatabaseError(format!(
            "Failed to get database connection: {}",
            e
        ))
    })?;

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
/// - `context`: Earlier points to load with a page after the first, for transformations
///
/// # Returns
/// - `Ok(SeriesDataPage)`: One page of matching points ordered by date (then revision date), the
///   total number of matching points across all pages, and the page's offset
/// - `Err(AppError)`: Database connection errors, invalid parameters, or query execution failures
///
/// # Filtering Capabilities
/// - **Time Range**: Filter by start and end dates for focused analysis periods
/// - **Data Vintage**: Choose between original releases and revised estimates
/// - **Revision Control**: Access complete revision history or latest values only
/// - **Pagination**: `limit` defaults to and is capped at [`MAX_SERIES_DATA_PAGE`]; page with
///   `offset` until [`SeriesDataPage::has_next_page`] is false
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
/// - Revision filtering runs in SQL, so the count and pages see the same rows
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
/// use econ_graph_services::series_service::get_series_data;
/// use econ_graph_core::models::DataQueryParams;
/// use econ_graph_core::database::DatabasePool;
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
///     as_of: None,
///     limit: Some(12),
///     offset: Some(0),
/// };
/// let page = get_series_data(pool, params, None).await?;
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
    context: Option<PageContext>,
) -> AppResult<SeriesDataPage> {
    let limit = params
        .limit
        .unwrap_or(MAX_SERIES_DATA_PAGE)
        .clamp(0, MAX_SERIES_DATA_PAGE);
    let offset = params.offset.unwrap_or(0).max(0);

    let mut conn = pool.get().await.map_err(|e| {
        AppError::DatabaseError(format!("Failed to get database connection: {}", e))
    })?;

    // One snapshot, so a crawl writing in between can't make the count, the page and the
    // context disagree.
    conn.build_transaction()
        .repeatable_read()
        .read_only()
        .run(async |conn| {
            let total_count = filtered_data_points(&params)
                .count()
                .get_result::<i64>(conn)
                .await?;

            let points = in_page_order(filtered_data_points(&params))
                .limit(limit)
                .offset(offset)
                .load::<DataPoint>(conn)
                .await?;

            let context = match (context, points.as_slice().first()) {
                (Some(context), Some(first)) if offset > 0 => {
                    load_context(conn, &params, first, context).await?
                }
                _ => Vec::new(),
            };

            Ok(SeriesDataPage {
                points,
                context,
                total_count,
                offset,
            })
        })
        .await
}

/// Largest page [`get_series_data`] returns. A longer series is read page by page.
pub const MAX_SERIES_DATA_PAGE: i64 = 10_000;

/// One page of a series' data points, in page order: date, then revision date, then id.
#[derive(Debug, Clone)]
pub struct SeriesDataPage {
    /// The points on this page.
    pub points: Vec<DataPoint>,
    /// Points before this page that a transformation compares the page's points with (see
    /// [`PageContext`]), in page order. Empty on the first page.
    pub context: Vec<DataPoint>,
    /// How many points match the filters across all pages.
    pub total_count: i64,
    /// How many matching points come before this page.
    pub offset: i64,
}

impl SeriesDataPage {
    /// Whether matching points remain after this page.
    pub fn has_next_page(&self) -> bool {
        self.offset + (self.points.len() as i64) < self.total_count
    }
}

/// Which points before a page a transformation of it needs, so the page transforms the same as
/// it would inside the whole series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageContext {
    /// Every earlier point dated at most this many days before the page's first point.
    Days(i64),
    /// The point just before the page.
    PreviousPoint,
    /// The series' earliest usable (non-null, non-zero) point, the base of a percent change.
    FirstPoint,
}

/// Points before `first` in page order, selected by `context`, in page order.
async fn load_context(
    conn: &mut diesel_async::AsyncPgConnection,
    params: &DataQueryParams,
    first: &DataPoint,
    context: PageContext,
) -> Result<Vec<DataPoint>, diesel::result::Error> {
    use data_points::dsl::{date, id, revision_date, value};

    // (date, revision_date, id) < first's, spelled out for Diesel.
    let before_first = date.lt(first.date).or(date.eq(first.date).and(
        revision_date
            .lt(first.revision_date)
            .or(revision_date.eq(first.revision_date).and(id.lt(first.id))),
    ));
    let earlier = filtered_data_points(params).filter(before_first);

    match context {
        PageContext::Days(days) => {
            let from = first
                .date
                .checked_sub_signed(chrono::Duration::days(days))
                .unwrap_or(chrono::NaiveDate::MIN);
            in_page_order(earlier.filter(date.ge(from)))
                .load::<DataPoint>(conn)
                .await
        }
        PageContext::PreviousPoint => {
            earlier
                .order_by((date.desc(), revision_date.desc(), id.desc()))
                .limit(1)
                .load::<DataPoint>(conn)
                .await
        }
        PageContext::FirstPoint => {
            // The percent-change base is the series' earliest *usable* value: a null or zero
            // reading at the very first date (or several) is skipped. `.ne` on a nullable column
            // already excludes NULLs under SQL's three-valued logic, so this one filter covers
            // both.
            in_page_order(earlier.filter(value.ne(bigdecimal::BigDecimal::from(0))))
                .limit(1)
                .load::<DataPoint>(conn)
                .await
        }
    }
}

/// Data points of `params.series_id` matching its date and revision filters, unordered and
/// unpaged. The count, the page and the context all start here so they agree.
fn filtered_data_points(
    params: &DataQueryParams,
) -> data_points::BoxedQuery<'static, diesel::pg::Pg> {
    let mut query = data_points::table
        .filter(data_points::series_id.eq(params.series_id))
        .into_boxed();

    if let Some(start_date) = params.start_date {
        query = query.filter(data_points::date.ge(start_date));
    }
    if let Some(end_date) = params.end_date {
        query = query.filter(data_points::date.le(end_date));
    }

    let original_only = params.original_only.unwrap_or(false);
    let as_of_or_latest = params.as_of.is_some() || params.latest_revision_only.unwrap_or(false);
    if original_only || as_of_or_latest {
        query = query.filter(econ_graph_core::models::exclude_synthetic_legacy_rows());
    }
    if original_only {
        query = query.filter(data_points::is_original_release.eq(true));
    }
    if as_of_or_latest {
        query = query.filter(econ_graph_core::models::revision_filter(
            params.as_of,
            original_only,
        ));
    }

    query
}

/// Page order: date, then revision date, then id, so pages are stable when a date has several
/// revisions.
fn in_page_order(
    query: data_points::BoxedQuery<'static, diesel::pg::Pg>,
) -> data_points::BoxedQuery<'static, diesel::pg::Pg> {
    query.order_by((
        data_points::date.asc(),
        data_points::revision_date.asc(),
        data_points::id.asc(),
    ))
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
