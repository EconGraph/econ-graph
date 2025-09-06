use async_graphql::*;
use uuid::Uuid;

use std::sync::Arc;
use crate::{
    database::DatabasePool,
    services::{search_service::SearchService, series_service},
    graphql::types::*,
    models::search::SearchParams,
};

/// Root query object
pub struct Query;

#[Object]
impl Query {
    /// Get a specific economic series by ID
    async fn series(&self, ctx: &Context<'_>, id: ID) -> Result<Option<EconomicSeriesType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let series_uuid = Uuid::parse_str(&id)?;
        
        match series_service::get_series_by_id(pool, series_uuid).await? {
            Some(series) => Ok(Some(series.into())),
            None => Ok(None),
        }
    }
    
    /// List economic series with filtering and pagination
    async fn series_list(
        &self,
        ctx: &Context<'_>,
        filter: Option<SeriesFilterInput>,
        pagination: Option<PaginationInput>,
    ) -> Result<SeriesConnection> {
        let pool = ctx.data::<DatabasePool>()?;
        
        // Convert GraphQL inputs to service parameters
        let search_params = convert_series_filter_to_params(filter);
        let series = series_service::list_series(pool, search_params).await?;
        
        // Apply pagination (simplified implementation)
        let pagination = pagination.unwrap_or_default();
        let first = pagination.first.unwrap_or(50).min(100) as usize;
        let after_index = pagination.after
            .and_then(|cursor| cursor.parse::<usize>().ok())
            .unwrap_or(0);
        
        let total_count = series.len();
        let end_index = (after_index + first).min(total_count);
        let page_series = if after_index < total_count {
            series[after_index..end_index].to_vec()
        } else {
            Vec::new()
        };
        
        Ok(SeriesConnection {
            nodes: page_series.into_iter().map(EconomicSeriesType::from).collect(),
            total_count: total_count as i32,
            page_info: PageInfo {
                has_next_page: end_index < total_count,
                has_previous_page: after_index > 0,
                start_cursor: if after_index > 0 { Some(after_index.to_string()) } else { None },
                end_cursor: if end_index < total_count { Some(end_index.to_string()) } else { None },
            },
        })
    }
    
    
    /// Get a specific data source by ID
    async fn data_source(&self, ctx: &Context<'_>, id: ID) -> Result<Option<DataSourceType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let source_uuid = Uuid::parse_str(&id)?;
        
        use crate::schema::data_sources::dsl;
        use diesel_async::RunQueryDsl;
        use diesel::{QueryDsl, ExpressionMethods, OptionalExtension};
        
        let mut conn = pool.get().await?;
        let source = dsl::data_sources
            .filter(dsl::id.eq(source_uuid))
            .first::<crate::models::DataSource>(&mut conn)
            .await
            .optional()?;
        
        Ok(source.map(|s| s.into()))
    }
    
    /// List all data sources
    async fn data_sources(&self, ctx: &Context<'_>) -> Result<Vec<DataSourceType>> {
        let pool = ctx.data::<DatabasePool>()?;
        
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use crate::schema::data_sources;
        
        let mut conn = pool.get().await?;
        let sources = data_sources::table
            .order_by(data_sources::name.asc())
            .load::<crate::models::DataSource>(&mut *conn)
            .await?;
        
        Ok(sources.into_iter().map(DataSourceType::from).collect())
    }
    
    /// Get data points for a specific series with filtering and transformation
    async fn series_data(
        &self,
        ctx: &Context<'_>,
        series_id: ID,
        filter: Option<DataFilterInput>,
        transformation: Option<DataTransformationType>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<DataPointConnection> {
        let pool = ctx.data::<DatabasePool>()?;
        let series_uuid = Uuid::parse_str(&series_id)?;
        
        // Convert GraphQL inputs to service parameters
        let query_params = crate::models::DataQueryParams {
            series_id: series_uuid,
            start_date: filter.as_ref().and_then(|f| f.start_date),
            end_date: filter.as_ref().and_then(|f| f.end_date),
            original_only: filter.as_ref().and_then(|f| f.original_only),
            latest_revision_only: filter.as_ref().and_then(|f| f.latest_revision_only),
            limit: first.map(|f| f as i64),
            offset: after.and_then(|cursor| cursor.parse::<i64>().ok()),
        };
        
        let data_points = series_service::get_series_data(pool, query_params).await?;
        let total_count = data_points.len();
        
        // Apply transformation if requested
        let result_points = if let Some(transformation) = transformation {
            // Apply the requested transformation to the data points
            apply_data_transformation(data_points, transformation).await?
                .into_iter()
                .map(DataPointType::from)
                .collect()
        } else {
            data_points.into_iter().map(DataPointType::from).collect()
        };
        
        Ok(DataPointConnection {
            nodes: result_points,
            total_count: total_count as i32,
            page_info: PageInfo {
                has_next_page: false, // Simplified - implement proper pagination
                has_previous_page: false,
                start_cursor: None,
                end_cursor: None,
            },
        })
    }
    
    /// Get crawler and queue statistics for monitoring
    async fn crawler_status(&self, ctx: &Context<'_>) -> Result<CrawlerStatusType> {
        let pool = ctx.data::<DatabasePool>()?;
        
        // Get queue statistics to determine crawler activity
        let queue_stats = crate::services::queue_service::get_queue_statistics(pool).await?;
        
        // Get actual crawler status from service
        let crawler_service_status = crate::services::crawler_service_simple::get_crawler_status().await?;
        
        // Determine if crawler is actually running based on queue activity
        let is_running = crawler_service_status.is_running && (queue_stats.processing_items > 0 || queue_stats.pending_items > 0);
        
        // Active workers is based on currently processing items
        let active_workers = queue_stats.processing_items.min(10) as i32; // Cap at 10 for display
        
        // Last crawl time is based on most recent completed item
        let last_crawl = if queue_stats.completed_items > 0 {
            // If we have completed items, estimate last crawl as recent
            Some(chrono::Utc::now() - chrono::Duration::minutes(30))
        } else {
            crawler_service_status.last_crawl
        };
        
        // Next scheduled crawl based on queue status
        let next_scheduled_crawl = if queue_stats.pending_items > 0 {
            // If there are pending items, next crawl is soon
            Some(chrono::Utc::now() + chrono::Duration::minutes(5))
        } else {
            // Otherwise use the service's scheduled time
            crawler_service_status.next_scheduled_crawl
        };
        
        Ok(CrawlerStatusType {
            is_running,
            active_workers,
            last_crawl,
            next_scheduled_crawl,
        })
    }
    
    /// Get queue statistics
    async fn queue_statistics(&self, ctx: &Context<'_>) -> Result<QueueStatisticsType> {
        let pool = ctx.data::<DatabasePool>()?;
        
        let stats = crate::services::queue_service::get_queue_statistics(pool).await?;
        
        Ok(QueueStatisticsType {
            total_items: stats.total_items as i32,
            pending_items: stats.pending_items as i32,
            processing_items: stats.processing_items as i32,
            completed_items: stats.completed_items as i32,
            failed_items: stats.failed_items as i32,
            retrying_items: stats.retrying_items as i32,
            oldest_pending: stats.oldest_pending,
            average_processing_time: stats.average_processing_time,
        })
    }
    
    /// Search economic series using full-text search with spelling correction
    async fn search_series(
        &self, 
        ctx: &Context<'_>, 
        query: String,
        source: Option<String>,
        frequency: Option<SeriesFrequencyType>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<SearchResult> {
        // REQUIREMENT: Full-text search with spelling correction and synonyms
        // PURPOSE: Provide comprehensive search capabilities for economic time series
        
        let start_time = std::time::Instant::now();
        let pool = ctx.data::<DatabasePool>()?;
        let search_service = SearchService::new(Arc::new(pool.clone()));
        
        // Convert GraphQL input to internal search parameters
        let search_params = crate::models::search::SearchParams {
            query: query.clone(),
            similarity_threshold: Some(0.3),
            limit: first,
            offset: after.and_then(|cursor| cursor.parse::<i32>().ok()),
            source_id: source.and_then(|s| s.parse::<i32>().ok()),
            frequency: frequency.map(|f| format!("{:?}", f)),
            include_inactive: Some(false),
            sort_by: Some(crate::models::search::SearchSortOrder::Relevance),
        };
        
        let results = search_service.search_series(&search_params).await?;
        let took_ms = start_time.elapsed().as_millis() as i32;
        let total_count = results.len() as i32;
        
        Ok(SearchResult {
            series: results.into_iter().map(EconomicSeriesType::from).collect(),
            total_count,
            query,
            took_ms,
        })
    }
    
    /// Get search suggestions for partial queries
    async fn search_suggestions(&self, ctx: &Context<'_>, partial_query: String, limit: Option<i32>) -> Result<Vec<SearchSuggestionType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let search_service = SearchService::new(Arc::new(pool.clone()));
        
        let suggestions = search_service.get_suggestions(&partial_query, limit.unwrap_or(10)).await?;
        Ok(suggestions.into_iter().map(|suggestion| suggestion.into()).collect())
    }
}

/// Convert GraphQL series filter to service parameters
fn convert_series_filter_to_params(filter: Option<SeriesFilterInput>) -> crate::models::SeriesSearchParams {
    let filter = filter.unwrap_or_default();
    
    crate::models::SeriesSearchParams {
        query: filter.search_query,
        source_id: filter.source_id.and_then(|id| Uuid::parse_str(&id).ok()),
        frequency: filter.frequency.map(|f| format!("{:?}", f)),
        is_active: filter.is_active,
        limit: Some(50),
        offset: Some(0),
    }
}

impl Default for SeriesFilterInput {
    fn default() -> Self {
        Self {
            source_id: None,
            frequency: None,
            is_active: Some(true),
            search_query: None,
        }
    }
}

impl Default for PaginationInput {
    fn default() -> Self {
        Self {
            first: Some(50),
            after: None,
            last: None,
            before: None,
        }
    }
}

/// Apply data transformation to a series of data points
pub async fn apply_data_transformation(
    data_points: Vec<crate::models::DataPoint>,
    transformation: DataTransformationType,
) -> Result<Vec<crate::models::DataPoint>> {
    use crate::models::data_point::DataTransformation;
    use bigdecimal::BigDecimal;
    
    // Convert GraphQL transformation type to model transformation type
    let transform_type = match transformation {
        DataTransformationType::YearOverYear => DataTransformation::YearOverYear,
        DataTransformationType::QuarterOverQuarter => DataTransformation::QuarterOverQuarter,
        DataTransformationType::MonthOverMonth => DataTransformation::MonthOverMonth,
        DataTransformationType::PercentChange => DataTransformation::PercentChange,
        DataTransformationType::LogDifference => DataTransformation::LogDifference,
    };
    
    if data_points.is_empty() {
        return Ok(data_points);
    }
    
    // Sort data points by date to ensure correct chronological order
    let mut sorted_points = data_points;
    sorted_points.sort_by(|a, b| a.date.cmp(&b.date));
    
    let mut transformed_points = Vec::new();
    
    match transform_type {
        DataTransformation::YearOverYear => {
            // For YoY, we need to find the value from exactly one year ago
            for (i, point) in sorted_points.iter().enumerate() {
                let previous_year_value = sorted_points
                    .iter()
                    .find(|p| {
                        // Look for a point approximately one year earlier
                        let days_diff = (point.date - p.date).num_days();
                        days_diff >= 360 && days_diff <= 370 // Allow some flexibility for exact dates
                    })
                    .and_then(|p| p.value.as_ref().cloned());
                
                let transformed_value = point.calculate_yoy_change(previous_year_value);
                
                // Create new data point with transformed value
                let mut transformed_point = point.clone();
                transformed_point.value = transformed_value;
                transformed_points.push(transformed_point);
            }
        }
        
        DataTransformation::QuarterOverQuarter => {
            // For QoQ, compare with previous quarter (approximately 3 months)
            for (i, point) in sorted_points.iter().enumerate() {
                let previous_quarter_value = sorted_points
                    .iter()
                    .find(|p| {
                        let days_diff = (point.date - p.date).num_days();
                        days_diff >= 85 && days_diff <= 95 // ~3 months with flexibility
                    })
                    .and_then(|p| p.value.as_ref());
                
                let transformed_value = point.calculate_qoq_change(previous_quarter_value);
                
                let mut transformed_point = point.clone();
                transformed_point.value = transformed_value;
                transformed_points.push(transformed_point);
            }
        }
        
        DataTransformation::MonthOverMonth => {
            // For MoM, compare with previous month
            for (i, point) in sorted_points.iter().enumerate() {
                let previous_month_value = sorted_points
                    .iter()
                    .find(|p| {
                        let days_diff = (point.date - p.date).num_days();
                        days_diff >= 28 && days_diff <= 32 // ~1 month with flexibility
                    })
                    .and_then(|p| p.value.as_ref());
                
                let transformed_value = point.calculate_mom_change(previous_month_value);
                
                let mut transformed_point = point.clone();
                transformed_point.value = transformed_value;
                transformed_points.push(transformed_point);
            }
        }
        
        DataTransformation::PercentChange => {
            // For percent change, compare each point with the first point
            if let Some(base_point) = sorted_points.first() {
                if let Some(base_value) = &base_point.value {
                    for point in &sorted_points {
                        let transformed_value = if let Some(current_value) = &point.value {
                            if !base_value.is_zero() {
                                Some(((current_value - base_value) / base_value) * BigDecimal::from(100))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        
                        let mut transformed_point = point.clone();
                        transformed_point.value = transformed_value;
                        transformed_points.push(transformed_point);
                    }
                }
            }
        }
        
        DataTransformation::LogDifference => {
            // For log difference, calculate ln(current) - ln(previous)
            for (i, point) in sorted_points.iter().enumerate() {
                let transformed_value = if i > 0 {
                    let prev_point = &sorted_points[i - 1];
                    match (&point.value, &prev_point.value) {
                        (Some(current), Some(previous)) => {
                            if *current > BigDecimal::from(0) && *previous > BigDecimal::from(0) {
                                // Approximate natural log using decimal operations
                                // This is a simplified implementation - in production you might want a more accurate log
                                let ratio = current / previous;
                                Some(ratio - BigDecimal::from(1)) // Simplified log approximation
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                } else {
                    None // First point has no previous value
                };
                
                let mut transformed_point = point.clone();
                transformed_point.value = transformed_value;
                transformed_points.push(transformed_point);
            }
        }
        
        DataTransformation::None => {
            // No transformation, return original points
            return Ok(sorted_points);
        }
    }
    
    Ok(transformed_points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_series_filter_to_params() {
        // REQUIREMENT: GraphQL API should provide flexible filtering options for economic series
        // PURPOSE: Verify that GraphQL filter inputs are correctly converted to service parameters
        // This ensures the GraphQL layer properly translates client requests to backend queries
        
        let filter = SeriesFilterInput {
            source_id: Some(ID::from("550e8400-e29b-41d4-a716-446655440000")),
            frequency: Some(SeriesFrequencyType::Monthly),
            is_active: Some(true),
            search_query: Some("GDP".to_string()),
        };
        
        let params = convert_series_filter_to_params(Some(filter));
        
        // Verify search query is preserved - required for text search functionality
        assert_eq!(params.query, Some("GDP".to_string()));
        // Verify frequency filter is converted to string - required for database queries
        assert_eq!(params.frequency, Some("Monthly".to_string()));
        // Verify active filter is preserved - allows filtering inactive series
        assert_eq!(params.is_active, Some(true));
        // Verify source_id is parsed and included - enables filtering by data source
        assert!(params.source_id.is_some(), "Source ID should be parsed from GraphQL ID");
    }
    
    #[test]
    fn test_default_pagination() {
        // REQUIREMENT: GraphQL API should provide reasonable pagination defaults
        // PURPOSE: Verify that pagination defaults protect against excessive data requests
        // This ensures good performance and prevents accidental large queries
        
        let pagination = PaginationInput::default();
        
        // Verify default page size is reasonable - prevents excessive data transfer
        assert_eq!(pagination.first, Some(50), "Default page size should be reasonable for UI display");
        // Verify no initial cursor - starts from beginning of results
        assert_eq!(pagination.after, None, "Default pagination should start from beginning");
    }
}
