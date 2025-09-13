use async_graphql::*;
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::SelectableHelper;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{
    AnnotationComment, ChartAnnotation, ChartCollaborator, DataPoint, DataSource, EconomicSeries,
    SearchSortOrder, SearchStatistics, SearchSuggestion, SeriesSearchResult, SuggestionType, User,
};

/// GraphQL representation of an economic series
#[derive(Clone)]
pub struct EconomicSeriesType {
    pub id: ID,
    pub source_id: ID,
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
}

#[Object]
impl EconomicSeriesType {
    async fn id(&self) -> &ID {
        &self.id
    }

    async fn source_id(&self) -> &ID {
        &self.source_id
    }

    async fn external_id(&self) -> &str {
        &self.external_id
    }

    async fn title(&self) -> &str {
        &self.title
    }

    async fn description(&self) -> &Option<String> {
        &self.description
    }

    async fn units(&self) -> &Option<String> {
        &self.units
    }

    async fn frequency(&self) -> &str {
        &self.frequency
    }

    async fn seasonal_adjustment(&self) -> &Option<String> {
        &self.seasonal_adjustment
    }

    async fn last_updated(&self) -> &Option<DateTime<Utc>> {
        &self.last_updated
    }

    async fn start_date(&self) -> &Option<NaiveDate> {
        &self.start_date
    }

    async fn end_date(&self) -> &Option<NaiveDate> {
        &self.end_date
    }

    async fn is_active(&self) -> bool {
        self.is_active
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    async fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Fetch the data source using direct database query
    async fn source(&self, ctx: &Context<'_>) -> Result<Option<DataSourceType>> {
        let pool = ctx.data::<crate::database::DatabasePool>()?;
        let source_uuid = Uuid::parse_str(&self.source_id)?;

        use crate::schema::data_sources::dsl;
        use diesel::{ExpressionMethods, OptionalExtension, QueryDsl};
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;
        let source = dsl::data_sources
            .filter(dsl::id.eq(source_uuid))
            .select(crate::models::DataSource::as_select())
            .first::<crate::models::DataSource>(&mut conn)
            .await
            .optional()?;

        Ok(source.map(|s| s.into()))
    }

    /// Fetch recent data points using direct database query
    async fn recent_data_points(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 100)] limit: i32,
    ) -> Result<Vec<DataPointType>> {
        let pool = ctx.data::<crate::database::DatabasePool>()?;
        let series_uuid = Uuid::parse_str(&self.id)?;

        use crate::schema::data_points::dsl;
        use diesel::{ExpressionMethods, QueryDsl};
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;
        let data_points = dsl::data_points
            .filter(dsl::series_id.eq(series_uuid))
            .order(dsl::date.desc())
            .limit(limit as i64)
            .load::<crate::models::DataPoint>(&mut conn)
            .await?;

        let limited_points = data_points.into_iter().map(DataPointType::from).collect();

        Ok(limited_points)
    }

    /// Get data point count using direct database query
    async fn data_point_count(&self, ctx: &Context<'_>) -> Result<i32> {
        let pool = ctx.data::<crate::database::DatabasePool>()?;
        let series_uuid = Uuid::parse_str(&self.id)?;

        use crate::schema::data_points::dsl;
        use diesel::{ExpressionMethods, QueryDsl};
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;
        let count = dsl::data_points
            .filter(dsl::series_id.eq(series_uuid))
            .count()
            .get_result::<i64>(&mut conn)
            .await?;

        Ok(count as i32)
    }

    /// Fetch data points with filters using a custom DataLoader
    async fn data_points(
        &self,
        ctx: &Context<'_>,
        filter: Option<DataFilterInput>,
        transformation: Option<DataTransformationType>,
    ) -> Result<Vec<DataPointType>> {
        use crate::database::DatabasePool;

        let pool = ctx.data::<DatabasePool>()?;
        let series_uuid = Uuid::parse_str(&self.id)?;

        let filter = filter.unwrap_or_default();

        // Query data points directly from database
        use crate::schema::data_points::dsl;
        use diesel::{ExpressionMethods, QueryDsl};
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;
        let mut query = dsl::data_points
            .filter(dsl::series_id.eq(series_uuid))
            .into_boxed();

        if let Some(start_date) = filter.start_date {
            query = query.filter(dsl::date.ge(start_date));
        }

        if let Some(end_date) = filter.end_date {
            query = query.filter(dsl::date.le(end_date));
        }

        if filter.original_only.unwrap_or(false) {
            query = query.filter(dsl::is_original_release.eq(true));
        }

        let data_points = query
            .order(dsl::date.asc())
            .load::<crate::models::DataPoint>(&mut conn)
            .await?;

        // Apply transformation if requested
        if let Some(transformation) = transformation {
            // Transform the data points using the GraphQL transformation function
            let transformed =
                crate::graphql::query::apply_data_transformation(data_points, transformation)
                    .await?;

            // Convert transformed data points to GraphQL types
            Ok(transformed.into_iter().map(DataPointType::from).collect())
        } else {
            Ok(data_points.into_iter().map(DataPointType::from).collect())
        }
    }
}

impl From<EconomicSeries> for EconomicSeriesType {
    fn from(series: EconomicSeries) -> Self {
        Self {
            id: ID::from(series.id.to_string()),
            source_id: ID::from(series.source_id.to_string()),
            external_id: series.external_id,
            title: series.title,
            description: series.description,
            units: series.units,
            frequency: series.frequency,
            seasonal_adjustment: series.seasonal_adjustment,
            last_updated: series.last_updated,
            start_date: series.start_date,
            end_date: series.end_date,
            is_active: series.is_active,
            created_at: series.created_at,
            updated_at: series.updated_at,
        }
    }
}

impl From<crate::models::search::SeriesSearchResult> for EconomicSeriesType {
    fn from(result: crate::models::search::SeriesSearchResult) -> Self {
        Self {
            id: ID::from(result.id.to_string()),
            source_id: ID::from(result.source_id.to_string()),
            external_id: result.external_id,
            title: result.title,
            description: result.description,
            units: Some(result.units),
            frequency: result.frequency,
            seasonal_adjustment: None,
            last_updated: Some(result.last_updated.and_utc()),
            start_date: Some(result.start_date),
            end_date: result.end_date,
            is_active: result.is_active,
            created_at: chrono::Utc::now(), // Not available in search result
            updated_at: chrono::Utc::now(), // Not available in search result
        }
    }
}

/// GraphQL representation of a data point
#[derive(SimpleObject, Clone)]
#[graphql(name = "DataPoint")]
pub struct DataPointType {
    pub id: ID,
    pub series_id: ID,
    pub date: NaiveDate,
    pub value: Option<BigDecimal>,
    pub revision_date: NaiveDate,
    pub is_original_release: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DataPoint> for DataPointType {
    fn from(data_point: DataPoint) -> Self {
        Self {
            id: ID::from(data_point.id.to_string()),
            series_id: ID::from(data_point.series_id.to_string()),
            date: data_point.date,
            value: data_point.value,
            revision_date: data_point.revision_date,
            is_original_release: data_point.is_original_release,
            created_at: data_point.created_at,
            updated_at: data_point.updated_at,
        }
    }
}

/// GraphQL representation of a data source
#[derive(Clone)]
pub struct DataSourceType {
    pub id: ID,
    pub name: String,
    pub description: Option<String>,
    pub base_url: String,
    pub api_key_required: bool,
    pub rate_limit_per_minute: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[Object]
impl DataSourceType {
    async fn id(&self) -> &ID {
        &self.id
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn description(&self) -> &Option<String> {
        &self.description
    }

    async fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn api_key_required(&self) -> bool {
        self.api_key_required
    }

    async fn rate_limit_per_minute(&self) -> i32 {
        self.rate_limit_per_minute
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    async fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Fetch all series for this data source using DataLoader
    async fn series(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 50)] first: i32,
        after: Option<String>,
    ) -> Result<SeriesConnection> {
        let pool = ctx.data::<crate::database::DatabasePool>()?;
        let source_uuid = Uuid::parse_str(&self.id)?;

        use crate::schema::economic_series::dsl;
        use diesel::{ExpressionMethods, QueryDsl};
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;
        let all_series = dsl::economic_series
            .filter(dsl::source_id.eq(source_uuid))
            .filter(dsl::is_active.eq(true))
            .select(crate::models::EconomicSeries::as_select())
            .load::<crate::models::EconomicSeries>(&mut conn)
            .await?;

        // Apply pagination (simplified - in production you'd want cursor-based pagination)
        let start_index = after
            .and_then(|cursor| cursor.parse::<usize>().ok())
            .unwrap_or(0);

        let end_index = (start_index + first as usize).min(all_series.len());
        let series_page = all_series[start_index..end_index].to_vec();

        let has_next_page = end_index < all_series.len();
        let has_previous_page = start_index > 0;

        Ok(SeriesConnection {
            nodes: series_page
                .into_iter()
                .map(EconomicSeriesType::from)
                .collect(),
            total_count: all_series.len() as i32,
            page_info: PageInfo {
                has_next_page,
                has_previous_page,
                start_cursor: if start_index > 0 {
                    Some(start_index.to_string())
                } else {
                    None
                },
                end_cursor: if has_next_page {
                    Some(end_index.to_string())
                } else {
                    None
                },
            },
        })
    }

    /// Get count of active series for this data source
    async fn series_count(&self, ctx: &Context<'_>) -> Result<i32> {
        let pool = ctx.data::<crate::database::DatabasePool>()?;
        let source_uuid = Uuid::parse_str(&self.id)?;

        use crate::schema::economic_series::dsl;
        use diesel::{ExpressionMethods, QueryDsl};
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;
        let count = dsl::economic_series
            .filter(dsl::source_id.eq(source_uuid))
            .filter(dsl::is_active.eq(true))
            .count()
            .get_result::<i64>(&mut conn)
            .await?;

        Ok(count as i32)
    }
}

impl From<DataSource> for DataSourceType {
    fn from(source: DataSource) -> Self {
        Self {
            id: ID::from(source.id.to_string()),
            name: source.name,
            description: source.description,
            base_url: source.base_url,
            api_key_required: source.api_key_required,
            rate_limit_per_minute: source.rate_limit_per_minute,
            created_at: source.created_at,
            updated_at: source.updated_at,
        }
    }
}

/// Transformed data point for GraphQL responses
#[derive(SimpleObject, Clone)]
#[graphql(name = "TransformedDataPoint")]
pub struct TransformedDataPointType {
    pub date: NaiveDate,
    pub original_value: Option<BigDecimal>,
    pub transformed_value: Option<BigDecimal>,
    pub transformation: DataTransformationType,
    pub revision_date: NaiveDate,
    pub is_original_release: bool,
}

/// Data transformation enumeration for GraphQL
#[derive(Enum, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[graphql(name = "DataTransformation")]
pub enum DataTransformationType {
    None,
    YearOverYear,
    QuarterOverQuarter,
    MonthOverMonth,
    PercentChange,
    LogDifference,
}

impl From<crate::models::DataTransformation> for DataTransformationType {
    fn from(transformation: crate::models::DataTransformation) -> Self {
        match transformation {
            crate::models::DataTransformation::None => DataTransformationType::None,
            crate::models::DataTransformation::YearOverYear => DataTransformationType::YearOverYear,
            crate::models::DataTransformation::QuarterOverQuarter => {
                DataTransformationType::QuarterOverQuarter
            }
            crate::models::DataTransformation::MonthOverMonth => {
                DataTransformationType::MonthOverMonth
            }
            crate::models::DataTransformation::PercentChange => {
                DataTransformationType::PercentChange
            }
            crate::models::DataTransformation::LogDifference => {
                DataTransformationType::LogDifference
            }
        }
    }
}

impl From<DataTransformationType> for crate::models::DataTransformation {
    fn from(transformation: DataTransformationType) -> Self {
        match transformation {
            DataTransformationType::None => crate::models::DataTransformation::None,
            DataTransformationType::YearOverYear => crate::models::DataTransformation::YearOverYear,
            DataTransformationType::QuarterOverQuarter => {
                crate::models::DataTransformation::QuarterOverQuarter
            }
            DataTransformationType::MonthOverMonth => {
                crate::models::DataTransformation::MonthOverMonth
            }
            DataTransformationType::PercentChange => {
                crate::models::DataTransformation::PercentChange
            }
            DataTransformationType::LogDifference => {
                crate::models::DataTransformation::LogDifference
            }
        }
    }
}

/// Series frequency enumeration for GraphQL
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(name = "SeriesFrequency")]
#[derive(Debug)]
pub enum SeriesFrequencyType {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Annual,
    Irregular,
}

/// Paginated result for series
#[derive(SimpleObject)]
#[graphql(name = "SeriesConnection")]
pub struct SeriesConnection {
    pub nodes: Vec<EconomicSeriesType>,
    pub total_count: i32,
    pub page_info: PageInfo,
}

/// Paginated result for data points
#[derive(SimpleObject)]
#[graphql(name = "DataPointConnection")]
pub struct DataPointConnection {
    pub nodes: Vec<DataPointType>,
    pub total_count: i32,
    pub page_info: PageInfo,
}

/// Page information for pagination
#[derive(SimpleObject)]
#[graphql(name = "PageInfo")]
pub struct PageInfo {
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

/// Search result type
#[derive(SimpleObject)]
#[graphql(name = "SearchResult")]
pub struct SearchResult {
    pub series: Vec<EconomicSeriesType>,
    pub total_count: i32,
    pub query: String,
    pub took_ms: i32,
}

/// Queue statistics for monitoring
#[derive(SimpleObject)]
#[graphql(name = "QueueStatistics")]
pub struct QueueStatisticsType {
    pub total_items: i32,
    pub pending_items: i32,
    pub processing_items: i32,
    pub completed_items: i32,
    pub failed_items: i32,
    pub retrying_items: i32,
    pub oldest_pending: Option<DateTime<Utc>>,
    pub average_processing_time: Option<f64>,
}

/// Crawler status information
#[derive(SimpleObject)]
#[graphql(name = "CrawlerStatus")]
pub struct CrawlerStatusType {
    pub is_running: bool,
    pub active_workers: i32,
    pub last_crawl: Option<DateTime<Utc>>,
    pub next_scheduled_crawl: Option<DateTime<Utc>>,
}

/// Input types for mutations and complex queries
#[derive(InputObject)]
#[graphql(name = "SeriesFilter")]
pub struct SeriesFilterInput {
    pub source_id: Option<ID>,
    pub frequency: Option<SeriesFrequencyType>,
    pub is_active: Option<bool>,
    pub search_query: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "DataFilter")]
pub struct DataFilterInput {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub original_only: Option<bool>,
    pub latest_revision_only: Option<bool>,
}

impl Default for DataFilterInput {
    fn default() -> Self {
        Self {
            start_date: None,
            end_date: None,
            original_only: Some(false),
            latest_revision_only: Some(false),
        }
    }
}

#[derive(InputObject)]
#[graphql(name = "PaginationInput")]
pub struct PaginationInput {
    pub first: Option<i32>,
    pub after: Option<String>,
    pub last: Option<i32>,
    pub before: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "TriggerCrawlInput")]
pub struct TriggerCrawlInput {
    pub sources: Option<Vec<String>>,
    pub series_ids: Option<Vec<String>>,
    pub priority: Option<i32>,
}

/// GraphQL representation of a search result
#[derive(Clone, SimpleObject)]
pub struct SeriesSearchResultType {
    /// Unique identifier for the series
    pub id: ID,
    /// Series title
    pub title: String,
    /// Series description
    pub description: Option<String>,
    /// External identifier from data source
    pub external_id: String,
    /// Data source ID
    pub source_id: ID,
    /// Data frequency (Monthly, Quarterly, etc.)
    pub frequency: String,
    /// Data units
    pub units: String,
    /// Series start date
    pub start_date: NaiveDate,
    /// Series end date (if applicable)
    pub end_date: Option<NaiveDate>,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
    /// Whether the series is active
    pub is_active: bool,
    /// Search relevance ranking score
    pub rank: f32,
    /// Similarity score for fuzzy matching
    pub similarity_score: f32,
}

impl From<SeriesSearchResult> for SeriesSearchResultType {
    fn from(result: SeriesSearchResult) -> Self {
        Self {
            id: ID::from(result.id),
            title: result.title,
            description: result.description,
            external_id: result.external_id,
            source_id: ID::from(result.source_id),
            frequency: result.frequency,
            units: result.units,
            start_date: result.start_date,
            end_date: result.end_date,
            last_updated: DateTime::from_naive_utc_and_offset(result.last_updated, Utc),
            is_active: result.is_active,
            rank: result.rank,
            similarity_score: result.similarity_score,
        }
    }
}

/// GraphQL representation of search suggestions
#[derive(Clone, SimpleObject)]
pub struct SearchSuggestionType {
    /// Suggested query text
    pub suggestion: String,
    /// Number of matching series
    pub match_count: i32,
    /// Type of suggestion
    pub suggestion_type: SuggestionTypeEnum,
    /// Confidence score
    pub confidence: f32,
}

impl From<SearchSuggestion> for SearchSuggestionType {
    fn from(suggestion: SearchSuggestion) -> Self {
        Self {
            suggestion: suggestion.suggestion,
            match_count: suggestion.match_count,
            suggestion_type: suggestion.suggestion_type.into(),
            confidence: suggestion.confidence,
        }
    }
}

/// GraphQL enum for suggestion types
#[derive(Clone, Copy, Enum, Eq, PartialEq)]
pub enum SuggestionTypeEnum {
    /// Spelling correction
    Correction,
    /// Query completion
    Completion,
    /// Synonym expansion
    Synonym,
}

impl From<SuggestionType> for SuggestionTypeEnum {
    fn from(suggestion_type: SuggestionType) -> Self {
        match suggestion_type {
            SuggestionType::Correction => SuggestionTypeEnum::Correction,
            SuggestionType::Completion => SuggestionTypeEnum::Completion,
            SuggestionType::Synonym => SuggestionTypeEnum::Synonym,
        }
    }
}

/// GraphQL enum for search sort order
#[derive(Clone, Copy, Enum, Eq, PartialEq)]
pub enum SearchSortOrderEnum {
    /// Sort by relevance score
    Relevance,
    /// Sort by title alphabetically
    Title,
    /// Sort by last updated date
    LastUpdated,
    /// Sort by start date
    StartDate,
}

impl From<SearchSortOrderEnum> for SearchSortOrder {
    fn from(sort_order: SearchSortOrderEnum) -> Self {
        match sort_order {
            SearchSortOrderEnum::Relevance => SearchSortOrder::Relevance,
            SearchSortOrderEnum::Title => SearchSortOrder::Title,
            SearchSortOrderEnum::LastUpdated => SearchSortOrder::LastUpdated,
            SearchSortOrderEnum::StartDate => SearchSortOrder::StartDate,
        }
    }
}

/// GraphQL input for search parameters
#[derive(Clone, InputObject)]
pub struct SearchParamsInput {
    /// Search query text
    pub query: String,
    /// Minimum similarity threshold for fuzzy matching
    pub similarity_threshold: Option<f32>,
    /// Maximum number of results
    pub limit: Option<i32>,
    /// Offset for pagination
    pub offset: Option<i32>,
    /// Filter by data source ID
    pub source_id: Option<ID>,
    /// Filter by series frequency
    pub frequency: Option<String>,
    /// Include inactive series
    pub include_inactive: Option<bool>,
    /// Sort order for results
    pub sort_by: Option<SearchSortOrderEnum>,
}

/// GraphQL representation of a chart annotation
#[derive(Clone, SimpleObject)]
pub struct ChartAnnotationType {
    /// Annotation ID
    pub id: ID,
    /// User who created the annotation
    pub user_id: ID,
    /// Associated series ID (if applicable)
    pub series_id: Option<String>,
    /// Associated chart ID (if applicable)
    pub chart_id: Option<ID>,
    /// Date the annotation refers to
    pub annotation_date: NaiveDate,
    /// Value the annotation refers to (if applicable)
    pub annotation_value: Option<BigDecimal>,
    /// Annotation title
    pub title: String,
    /// Annotation description/content
    pub description: Option<String>,
    /// Annotation color for display
    pub color: Option<String>,
    /// Type of annotation (note, highlight, warning, etc.)
    pub annotation_type: Option<String>,
    /// Whether the annotation is visible to others
    pub is_visible: Option<bool>,
    /// Whether the annotation is pinned
    pub is_pinned: Option<bool>,
    /// Tags associated with the annotation
    pub tags: Option<Vec<Option<String>>>,
    /// Creation timestamp
    pub created_at: Option<DateTime<Utc>>,
    /// Last update timestamp
    pub updated_at: Option<DateTime<Utc>>,
}

impl From<ChartAnnotation> for ChartAnnotationType {
    fn from(annotation: ChartAnnotation) -> Self {
        Self {
            id: ID::from(annotation.id),
            user_id: ID::from(annotation.user_id),
            series_id: annotation.series_id,
            chart_id: annotation.chart_id.map(ID::from),
            annotation_date: annotation.annotation_date,
            annotation_value: annotation.annotation_value,
            title: annotation.title,
            description: annotation.description,
            color: annotation.color,
            annotation_type: annotation.annotation_type,
            is_visible: annotation.is_visible,
            is_pinned: annotation.is_pinned,
            tags: annotation.tags,
            created_at: annotation.created_at,
            updated_at: annotation.updated_at,
        }
    }
}

/// GraphQL representation of an annotation comment
#[derive(Clone, SimpleObject)]
pub struct AnnotationCommentType {
    /// Comment ID
    pub id: ID,
    /// Associated annotation ID
    pub annotation_id: ID,
    /// User who created the comment
    pub user_id: ID,
    /// Comment content
    pub content: String,
    /// Whether the comment thread is resolved
    pub is_resolved: Option<bool>,
    /// Creation timestamp
    pub created_at: Option<DateTime<Utc>>,
    /// Last update timestamp
    pub updated_at: Option<DateTime<Utc>>,
}

impl From<AnnotationComment> for AnnotationCommentType {
    fn from(comment: AnnotationComment) -> Self {
        Self {
            id: ID::from(comment.id),
            annotation_id: ID::from(comment.annotation_id),
            user_id: ID::from(comment.user_id),
            content: comment.content,
            is_resolved: comment.is_resolved,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        }
    }
}

/// GraphQL representation of a chart collaborator
#[derive(Clone, SimpleObject)]
pub struct ChartCollaboratorType {
    /// Collaborator entry ID
    pub id: ID,
    /// Chart ID being shared
    pub chart_id: ID,
    /// User ID of the collaborator
    pub user_id: ID,
    /// User who invited this collaborator
    pub invited_by: Option<ID>,
    /// Role/permission level
    pub role: Option<String>,
    /// Detailed permissions (JSON)
    pub permissions: Option<String>, // Serialize JSON as string for GraphQL
    /// Creation timestamp
    pub created_at: Option<DateTime<Utc>>,
    /// Last access timestamp
    pub last_accessed_at: Option<DateTime<Utc>>,
}

impl From<ChartCollaborator> for ChartCollaboratorType {
    fn from(collaborator: ChartCollaborator) -> Self {
        Self {
            id: ID::from(collaborator.id),
            chart_id: ID::from(collaborator.chart_id),
            user_id: ID::from(collaborator.user_id),
            invited_by: collaborator.invited_by.map(ID::from),
            role: collaborator.role,
            permissions: collaborator
                .permissions
                .map(|p| serde_json::to_string(&p).unwrap_or_default()),
            created_at: collaborator.created_at,
            last_accessed_at: collaborator.last_accessed_at,
        }
    }
}

/// GraphQL representation of a user
#[derive(Clone, SimpleObject)]
pub struct UserType {
    /// User ID
    pub id: ID,
    /// Email address
    pub email: String,
    /// Display name
    pub name: String,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Authentication provider
    pub provider: String,
    /// User role
    pub role: String,
    /// Organization
    pub organization: Option<String>,
    /// UI theme preference
    pub theme: String,
    /// Default chart type preference
    pub default_chart_type: String,
    /// Whether notifications are enabled
    pub notifications_enabled: bool,
    /// Whether collaboration features are enabled
    pub collaboration_enabled: bool,
    /// Whether the account is active
    pub is_active: bool,
    /// Whether email is verified
    pub email_verified: bool,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Last login timestamp
    pub last_login_at: Option<DateTime<Utc>>,
}

impl From<User> for UserType {
    fn from(user: User) -> Self {
        Self {
            id: ID::from(user.id),
            email: user.email,
            name: user.name,
            avatar_url: user.avatar_url,
            provider: user.provider,
            role: user.role,
            organization: user.organization,
            theme: user.theme,
            default_chart_type: user.default_chart_type,
            notifications_enabled: user.notifications_enabled,
            collaboration_enabled: user.collaboration_enabled,
            is_active: user.is_active,
            email_verified: user.email_verified,
            created_at: user.created_at,
            updated_at: user.updated_at,
            last_login_at: user.last_login_at,
        }
    }
}

/// Input for creating a new annotation
#[derive(InputObject)]
pub struct CreateAnnotationInput {
    /// User ID creating the annotation
    pub user_id: ID,
    /// Series ID the annotation is for
    pub series_id: ID,
    /// Date the annotation refers to
    pub annotation_date: NaiveDate,
    /// Value the annotation refers to (optional)
    pub annotation_value: Option<BigDecimal>,
    /// Annotation title
    pub title: String,
    /// Annotation content/description
    pub content: String,
    /// Type of annotation (note, highlight, warning, etc.)
    pub annotation_type: String,
    /// Color for display (optional)
    pub color: Option<String>,
    /// Whether the annotation is public (visible to others)
    pub is_public: Option<bool>,
}

/// Input for adding a comment to an annotation
#[derive(InputObject)]
pub struct AddCommentInput {
    /// User ID adding the comment
    pub user_id: ID,
    /// Annotation ID to comment on
    pub annotation_id: ID,
    /// Comment content
    pub content: String,
}

/// Input for sharing a chart with another user
#[derive(InputObject)]
pub struct ShareChartInput {
    /// Owner user ID (who is sharing)
    pub owner_user_id: ID,
    /// Target user ID (who to share with)
    pub target_user_id: ID,
    /// Chart ID to share
    pub chart_id: ID,
    /// Permission level (view, comment, edit, admin)
    pub permission_level: String,
}

/// Input for deleting an annotation
#[derive(InputObject)]
pub struct DeleteAnnotationInput {
    /// User ID requesting deletion
    pub user_id: ID,
    /// Annotation ID to delete
    pub annotation_id: ID,
}

// Admin GraphQL Types

/// Input for creating a new user (admin only)
#[derive(InputObject)]
pub struct CreateUserInput {
    /// Email address
    pub email: String,
    /// Display name
    pub name: String,
    /// Password (for email-based users)
    pub password: Option<String>,
    /// User role
    pub role: String,
    /// Organization (optional)
    pub organization: Option<String>,
    /// Whether account is active
    pub is_active: Option<bool>,
    /// Whether to send welcome email
    pub send_welcome_email: Option<bool>,
}

/// Input for updating a user (admin only)
#[derive(InputObject)]
pub struct UpdateUserInput {
    /// Email address
    pub email: Option<String>,
    /// Display name (optional)
    pub name: Option<String>,
    /// Avatar URL (optional)
    pub avatar_url: Option<String>,
    /// User role (optional)
    pub role: Option<String>,
    /// Organization (optional)
    pub organization: Option<String>,
    /// UI theme preference
    pub theme: Option<String>,
    /// Default chart type preference
    pub default_chart_type: Option<String>,
    /// Whether notifications are enabled
    pub notifications_enabled: Option<bool>,
    /// Whether collaboration features are enabled
    pub collaboration_enabled: Option<bool>,
    /// Whether account is active (optional)
    pub is_active: Option<bool>,
    /// Whether email is verified (optional)
    pub email_verified: Option<bool>,
}

/// Input for filtering users (admin only)
#[derive(InputObject)]
pub struct UserFilterInput {
    /// Filter by role
    pub role: Option<String>,
    /// Filter by organization
    pub organization: Option<String>,
    /// Filter by active status
    pub is_active: Option<bool>,
    /// Filter by email verified status
    pub email_verified: Option<bool>,
    /// Search term for name or email
    pub search_query: Option<String>,
}

/// Input for filtering audit logs (admin only)
#[derive(InputObject)]
pub struct AuditLogFilterInput {
    /// Filter by user ID
    pub user_id: Option<ID>,
    /// Filter by action
    pub action: Option<String>,
    /// Filter by resource type
    pub resource_type: Option<String>,
    /// Created after date
    pub created_after: Option<DateTime<Utc>>,
    /// Created before date
    pub created_before: Option<DateTime<Utc>>,
}

/// GraphQL connection for users
#[derive(SimpleObject)]
pub struct UserConnection {
    /// List of users
    pub nodes: Vec<UserType>,
    /// Total count of users
    pub total_count: i32,
    /// Pagination info
    pub page_info: PageInfo,
}

/// GraphQL representation of a user session
#[derive(Clone, SimpleObject)]
pub struct UserSessionType {
    /// Session ID
    pub id: ID,
    /// User ID
    pub user_id: ID,
    /// Session creation time
    pub created_at: DateTime<Utc>,
    /// Last activity time
    pub last_activity: DateTime<Utc>,
    /// Session expiration time
    pub expires_at: DateTime<Utc>,
    /// User agent string
    pub user_agent: Option<String>,
    /// IP address
    pub ip_address: Option<String>,
    /// Whether session is active
    pub is_active: bool,
}

/// GraphQL representation of system health
#[derive(Clone, SimpleObject)]
pub struct SystemHealthType {
    /// Overall system status
    pub status: String,
    /// System metrics
    pub metrics: SystemMetricsType,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// GraphQL representation of system metrics
#[derive(Clone, SimpleObject)]
pub struct SystemMetricsType {
    /// Total number of users
    pub total_users: i32,
    /// Number of active users
    pub active_users: i32,
    /// Total number of sessions
    pub total_sessions: i32,
    /// Number of active sessions
    pub active_sessions: i32,
    /// Database size in MB
    pub database_size_mb: f64,
    /// Number of queue items
    pub queue_items: i32,
    /// API requests per minute
    pub api_requests_per_minute: f64,
    /// Average response time in milliseconds
    pub average_response_time_ms: f64,
}

/// GraphQL representation of a security event
#[derive(Clone, SimpleObject)]
pub struct SecurityEventType {
    /// Event ID
    pub id: ID,
    /// Event type
    pub event_type: String,
    /// User ID (if applicable)
    pub user_id: Option<ID>,
    /// User email (if applicable)
    pub user_email: Option<String>,
    /// IP address
    pub ip_address: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Event description
    pub description: String,
    /// Event severity
    pub severity: String,
    /// Event timestamp
    pub created_at: DateTime<Utc>,
}

/// GraphQL representation of an audit log entry
#[derive(Clone, SimpleObject)]
pub struct AuditLogType {
    /// Log entry ID
    pub id: ID,
    /// User ID who performed action
    pub user_id: ID,
    /// User name who performed action
    pub user_name: String,
    /// Action performed
    pub action: String,
    /// Type of resource affected
    pub resource_type: String,
    /// ID of resource affected
    pub resource_id: Option<String>,
    /// IP address
    pub ip_address: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Additional details
    pub details: Option<String>,
    /// Action timestamp
    pub created_at: DateTime<Utc>,
}

/// GraphQL connection for audit logs
#[derive(SimpleObject)]
pub struct AuditLogConnection {
    /// List of audit log entries
    pub nodes: Vec<AuditLogType>,
    /// Total count of entries
    pub total_count: i32,
    /// Pagination info
    pub page_info: PageInfo,
}
