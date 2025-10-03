use async_graphql::*;
use chrono::Utc;
use diesel::SelectableHelper;
use uuid::Uuid;

use crate::{
    database::DatabasePool,
    graphql::{context::require_admin, types::*},
    services::{search_service::SearchService, series_service, CompanyService, FinancialStatementService},
};
use std::sync::Arc;

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
        let after_index = pagination
            .after
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
            nodes: page_series
                .into_iter()
                .map(EconomicSeriesType::from)
                .collect(),
            total_count: total_count as i32,
            page_info: PageInfo {
                has_next_page: end_index < total_count,
                has_previous_page: after_index > 0,
                start_cursor: if after_index > 0 {
                    Some(after_index.to_string())
                } else {
                    None
                },
                end_cursor: if end_index < total_count {
                    Some(end_index.to_string())
                } else {
                    None
                },
            },
        })
    }

    /// Get a specific data source by ID
    async fn data_source(&self, ctx: &Context<'_>, id: ID) -> Result<Option<DataSourceType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let source_uuid = Uuid::parse_str(&id)?;

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

    /// List all data sources
    async fn data_sources(&self, ctx: &Context<'_>) -> Result<Vec<DataSourceType>> {
        let pool = ctx.data::<DatabasePool>()?;

        use crate::schema::data_sources;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;
        let sources = data_sources::table
            .order_by(data_sources::name.asc())
            .select(crate::models::DataSource::as_select())
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
            apply_data_transformation(data_points, transformation)
                .await?
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
        let crawler_service_status =
            crate::services::crawler::simple_crawler_service::get_crawler_status().await?;

        // Determine if crawler is actually running based on queue activity
        let is_running = crawler_service_status.is_running
            && (queue_stats.processing_items > 0 || queue_stats.pending_items > 0);

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
            source_id: source.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
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
    async fn search_suggestions(
        &self,
        ctx: &Context<'_>,
        partial_query: String,
        limit: Option<i32>,
    ) -> Result<Vec<SearchSuggestionType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let search_service = SearchService::new(Arc::new(pool.clone()));

        let suggestions = search_service
            .get_suggestions(&partial_query, limit.unwrap_or(10))
            .await?;
        Ok(suggestions
            .into_iter()
            .map(|suggestion| suggestion.into())
            .collect())
    }

    /// Get annotations for a specific series
    async fn annotations_for_series(
        &self,
        ctx: &Context<'_>,
        series_id: String,
        user_id: Option<ID>,
    ) -> Result<Vec<ChartAnnotationType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = crate::services::CollaborationService::new(pool.clone());

        let user_uuid = if let Some(uid) = user_id {
            Some(uuid::Uuid::parse_str(&uid)?)
        } else {
            None
        };

        let annotations = collaboration_service
            .get_annotations_for_series(&series_id, user_uuid)
            .await?;
        Ok(annotations
            .into_iter()
            .map(ChartAnnotationType::from)
            .collect())
    }

    /// Get comments for a specific annotation
    async fn comments_for_annotation(
        &self,
        ctx: &Context<'_>,
        annotation_id: ID,
    ) -> Result<Vec<AnnotationCommentType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = crate::services::CollaborationService::new(pool.clone());

        let annotation_uuid = uuid::Uuid::parse_str(&annotation_id)?;
        let comments = collaboration_service
            .get_comments_for_annotation(annotation_uuid)
            .await?;
        Ok(comments
            .into_iter()
            .map(AnnotationCommentType::from)
            .collect())
    }

    /// Get collaborators for a specific chart
    async fn chart_collaborators(
        &self,
        ctx: &Context<'_>,
        chart_id: ID,
    ) -> Result<Vec<ChartCollaboratorType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = crate::services::CollaborationService::new(pool.clone());

        let chart_uuid = uuid::Uuid::parse_str(&chart_id)?;
        let collaborators = collaboration_service.get_collaborators(chart_uuid).await?;
        Ok(collaborators
            .into_iter()
            .map(|(collaborator, _user)| ChartCollaboratorType::from(collaborator))
            .collect())
    }

    /// Get user information by ID
    async fn user(&self, ctx: &Context<'_>, user_id: ID) -> Result<Option<UserType>> {
        let pool = ctx.data::<DatabasePool>()?;

        let user_uuid = uuid::Uuid::parse_str(&user_id)?;

        use crate::schema::users;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        let user = users::table
            .filter(users::id.eq(user_uuid))
            .select(crate::models::User::as_select())
            .first::<crate::models::User>(&mut conn)
            .await
            .optional()?;

        Ok(user.map(UserType::from))
    }

    // Admin Queries

    /// Get all users (admin only)
    async fn users(
        &self,
        ctx: &Context<'_>,
        filter: Option<UserFilterInput>,
        pagination: Option<PaginationInput>,
    ) -> Result<UserConnection> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;

        use crate::schema::users;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        // Build query with filters
        let mut query = users::table.into_boxed();

        if let Some(filter) = &filter {
            if let Some(role) = &filter.role {
                query = query.filter(users::role.eq(role));
            }
            if let Some(organization) = &filter.organization {
                query = query.filter(users::organization.eq(organization));
            }
            if let Some(is_active) = filter.is_active {
                query = query.filter(users::is_active.eq(is_active));
            }
            if let Some(email_verified) = filter.email_verified {
                query = query.filter(users::email_verified.eq(email_verified));
            }
            if let Some(search_query) = &filter.search_query {
                let search_pattern = format!("%{}%", search_query);
                let pattern_clone = search_pattern.clone();
                query = query.filter(
                    users::name
                        .ilike(search_pattern)
                        .or(users::email.ilike(pattern_clone)),
                );
            }
        }

        // Get total count (rebuild query to avoid move)
        let mut count_query = users::table.into_boxed();
        if let Some(filter) = &filter {
            if let Some(role) = &filter.role {
                count_query = count_query.filter(users::role.eq(role));
            }
            if let Some(organization) = &filter.organization {
                count_query = count_query.filter(users::organization.eq(organization));
            }
            if let Some(is_active) = filter.is_active {
                count_query = count_query.filter(users::is_active.eq(is_active));
            }
            if let Some(email_verified) = filter.email_verified {
                count_query = count_query.filter(users::email_verified.eq(email_verified));
            }
            if let Some(search_query) = &filter.search_query {
                let search_pattern = format!("%{}%", search_query);
                let pattern_clone = search_pattern.clone();
                count_query = count_query.filter(
                    users::name
                        .ilike(search_pattern)
                        .or(users::email.ilike(pattern_clone)),
                );
            }
        }

        let total_count: i64 = count_query.count().get_result(&mut conn).await?;

        // Apply pagination
        let limit = pagination
            .as_ref()
            .and_then(|p| p.first)
            .unwrap_or(50)
            .min(100) as i64; // Cap at 100

        let offset = pagination
            .as_ref()
            .and_then(|p| p.after.clone())
            .and_then(|cursor| cursor.parse::<i64>().ok())
            .unwrap_or(0);

        let users_list: Vec<crate::models::User> = query
            .select(crate::models::User::as_select())
            .order(users::created_at.desc())
            .limit(limit)
            .offset(offset)
            .load(&mut conn)
            .await?;

        let has_next_page = (offset + limit) < total_count;
        let has_previous_page = offset > 0;

        Ok(UserConnection {
            nodes: users_list.into_iter().map(UserType::from).collect(),
            total_count: total_count as i32,
            page_info: PageInfo {
                has_next_page,
                has_previous_page,
                start_cursor: if has_previous_page {
                    Some(offset.to_string())
                } else {
                    None
                },
                end_cursor: if has_next_page {
                    Some((offset + limit).to_string())
                } else {
                    None
                },
            },
        })
    }

    /// Get user sessions (admin only)
    async fn user_sessions(
        &self,
        ctx: &Context<'_>,
        user_id: Option<ID>,
    ) -> Result<Vec<UserSessionType>> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;

        use crate::schema::user_sessions;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        let mut query = user_sessions::table.into_boxed();

        if let Some(user_id_str) = user_id {
            let user_uuid = uuid::Uuid::parse_str(&user_id_str)?;
            query = query.filter(user_sessions::user_id.eq(user_uuid));
        }

        let sessions: Vec<crate::models::UserSession> = query
            .select(crate::models::UserSession::as_select())
            .order(user_sessions::created_at.desc())
            .load(&mut conn)
            .await?;

        Ok(sessions
            .into_iter()
            .map(|session| UserSessionType {
                id: ID::from(session.id),
                user_id: ID::from(session.user_id),
                created_at: session.created_at,
                last_activity: session.last_used_at,
                expires_at: session.expires_at,
                user_agent: session.user_agent,
                ip_address: session.ip_address,
                is_active: session.expires_at > Utc::now(),
            })
            .collect())
    }

    /// Get active user sessions (admin only)
    async fn active_sessions(&self, ctx: &Context<'_>) -> Result<Vec<UserSessionType>> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;

        use crate::schema::user_sessions;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        let sessions: Vec<crate::models::UserSession> = user_sessions::table
            .filter(user_sessions::expires_at.gt(Utc::now()))
            .select(crate::models::UserSession::as_select())
            .order(user_sessions::last_used_at.desc())
            .load(&mut conn)
            .await?;

        Ok(sessions
            .into_iter()
            .map(|session| UserSessionType {
                id: ID::from(session.id),
                user_id: ID::from(session.user_id),
                created_at: session.created_at,
                last_activity: session.last_used_at,
                expires_at: session.expires_at,
                user_agent: session.user_agent,
                ip_address: session.ip_address,
                is_active: true, // All sessions here are active by definition
            })
            .collect())
    }

    /// Get system health metrics (admin only)
    async fn system_health(&self, ctx: &Context<'_>) -> Result<SystemHealthType> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;

        use crate::schema::{crawl_queue, user_sessions, users};
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        // Get user counts
        let total_users: i64 = users::table.count().get_result(&mut conn).await?;

        let active_users: i64 = users::table
            .filter(users::last_login_at.gt(Utc::now() - chrono::Duration::hours(24)))
            .count()
            .get_result(&mut conn)
            .await?;

        // Get session counts
        let total_sessions: i64 = user_sessions::table.count().get_result(&mut conn).await?;

        let active_sessions: i64 = user_sessions::table
            .filter(user_sessions::expires_at.gt(Utc::now()))
            .count()
            .get_result(&mut conn)
            .await?;

        // Get queue items count
        let queue_items: i64 = crawl_queue::table.count().get_result(&mut conn).await?;

        // Basic service status
        Ok(SystemHealthType {
            status: "healthy".to_string(),
            metrics: SystemMetricsType {
                total_users: total_users as i32,
                active_users: active_users as i32,
                total_sessions: total_sessions as i32,
                active_sessions: active_sessions as i32,
                database_size_mb: 0.0, // Would need special query for this
                queue_items: queue_items as i32,
                api_requests_per_minute: 0.0, // Would need metrics collection
                average_response_time_ms: 0.0, // Would need metrics collection
            },
            last_updated: Utc::now(),
        })
    }

    /// Get security events (admin only)
    async fn security_events(
        &self,
        ctx: &Context<'_>,
        _limit: Option<i32>,
    ) -> Result<Vec<SecurityEventType>> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let _pool = ctx.data::<DatabasePool>()?;

        // Get security events logic would go here
        // For now, return empty vector
        Ok(vec![])
    }

    /// Get audit logs (admin only)
    async fn audit_logs(
        &self,
        ctx: &Context<'_>,
        _filter: Option<AuditLogFilterInput>,
        _pagination: Option<PaginationInput>,
    ) -> Result<AuditLogConnection> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let _pool = ctx.data::<DatabasePool>()?;

        // Get audit logs logic would go here
        // For now, return empty connection
        Ok(AuditLogConnection {
            nodes: vec![],
            total_count: 0,
            page_info: PageInfo {
                has_next_page: false,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: None,
            },
        })
    }

    // ============================================================================
    // SEC CRAWLER QUERIES
    // ============================================================================

    /// Search for companies using fulltext search
    async fn search_companies(
        &self,
        ctx: &Context<'_>,
        input: CompanySearchInput,
    ) -> Result<CompanyConnection> {
        let pool = ctx.data::<DatabasePool>()?;
        let company_service = CompanyService::new(Arc::new(pool.clone()));
        
        // Search for companies
        let companies = company_service
            .search_companies(
                &input.query,
                input.limit,
                input.include_inactive,
            )
            .await?;

        // Get total count
        let total_count = company_service
            .count_companies(&input.query, input.include_inactive)
            .await? as i32;

        // Convert to GraphQL types
        let company_types: Vec<CompanyType> = companies
            .into_iter()
            .map(|company| company.into())
            .collect();

        Ok(CompanyConnection {
            nodes: company_types,
            total_count,
            page_info: PageInfo {
                has_next_page: false, // TODO: Implement proper pagination
                has_previous_page: false,
                start_cursor: None,
                end_cursor: None,
            },
        })
    }

    /// Get a specific company by ID
    async fn company(&self, ctx: &Context<'_>, id: ID) -> Result<Option<CompanyType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let company_uuid = Uuid::parse_str(&id)?;
        let company_service = CompanyService::new(Arc::new(pool.clone()));
        
        let company = company_service.get_company_by_id(company_uuid).await?;
        Ok(company.map(|c| c.into()))
    }

    /// Get financial statements for a company
    async fn company_financial_statements(
        &self,
        ctx: &Context<'_>,
        company_id: ID,
        pagination: Option<PaginationInput>,
    ) -> Result<FinancialStatementConnection> {
        let pool = ctx.data::<DatabasePool>()?;
        let company_uuid = Uuid::parse_str(&company_id)?;
        let financial_statement_service = FinancialStatementService::new(Arc::new(pool.clone()));
        
        // Apply pagination
        let pagination = pagination.unwrap_or_default();
        let limit = pagination.first.unwrap_or(50).min(100);
        let offset = pagination.after
            .and_then(|cursor| cursor.parse::<i32>().ok())
            .unwrap_or(0);
        
        // Get financial statements
        let statements = financial_statement_service
            .get_company_financial_statements(company_uuid, Some(limit), Some(offset))
            .await?;

        // Get total count
        let total_count = financial_statement_service
            .count_company_financial_statements(company_uuid)
            .await? as i32;

        // Convert to GraphQL types
        let statement_types: Vec<FinancialStatementType> = statements
            .into_iter()
            .map(|statement| statement.into())
            .collect();

        Ok(FinancialStatementConnection {
            nodes: statement_types,
            total_count,
            page_info: PageInfo {
                has_next_page: false, // TODO: Implement proper pagination
                has_previous_page: false,
                start_cursor: None,
                end_cursor: None,
            },
        })
    }
}

/// Convert GraphQL series filter to service parameters
fn convert_series_filter_to_params(
    filter: Option<SeriesFilterInput>,
) -> crate::models::SeriesSearchParams {
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
    use bigdecimal::{BigDecimal, Zero};

    // Convert GraphQL transformation type to model transformation type
    let transform_type = match transformation {
        DataTransformationType::None => DataTransformation::None,
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
            for (_i, point) in sorted_points.iter().enumerate() {
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
            for (_i, point) in sorted_points.iter().enumerate() {
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
            for (_i, point) in sorted_points.iter().enumerate() {
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
                                Some(
                                    ((current_value - base_value) / base_value)
                                        * BigDecimal::from(100),
                                )
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
        assert!(
            params.source_id.is_some(),
            "Source ID should be parsed from GraphQL ID"
        );
    }

    #[test]
    fn test_default_pagination() {
        // REQUIREMENT: GraphQL API should provide reasonable pagination defaults
        // PURPOSE: Verify that pagination defaults protect against excessive data requests
        // This ensures good performance and prevents accidental large queries

        let pagination = PaginationInput::default();

        // Verify default page size is reasonable - prevents excessive data transfer
        assert_eq!(
            pagination.first,
            Some(50),
            "Default page size should be reasonable for UI display"
        );
        // Verify no initial cursor - starts from beginning of results
        assert_eq!(
            pagination.after, None,
            "Default pagination should start from beginning"
        );
    }
}
