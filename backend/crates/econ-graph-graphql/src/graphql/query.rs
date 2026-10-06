//! # GraphQL Query Resolvers
//!
//! This module contains all GraphQL query resolvers for the EconGraph API.
//! It provides read-only access to economic data with proper filtering, pagination, and authorization.
//!
//! # Design Principles
//!
//! 1. **Authorization**: All resolvers implement proper role-based access control
//! 2. **Performance**: Queries are optimized for efficiency and caching
//! 3. **Error Handling**: Comprehensive error handling with user-friendly messages
//! 4. **Type Safety**: Strong typing throughout with proper validation
//!
//! # Quality Standards
//!
//! - All resolvers must implement proper authorization checks
//! - Database queries must be optimized and use proper indexing
//! - Error messages must be user-friendly and actionable
//! - All resolvers must have comprehensive documentation

use crate::imports::*;
use crate::types::*;

use crate::graphql::cross_section::{self, CrossSectionEntry, DimensionFilterInput};

/// Root query object
pub struct Query;

#[Object]
impl Query {
    /// Get a specific economic series by ID
    async fn series(&self, ctx: &Context<'_>, id: ID) -> Result<Option<EconomicSeriesType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let series_uuid = Uuid::parse_str(&id)?;

        match series_service::get_series_by_id(&pool, series_uuid).await? {
            // A series with no data points (e.g. a discovered series whose source adapter was
            // removed) is treated as not found, the same as an unknown id.
            Some(series) if series.end_date.is_some() => Ok(Some(series.into())),
            _ => Ok(None),
        }
    }

    /// Get an economic series by its source's name and the source's own id for it
    ///
    /// `sourceName` is the data source's `name` exactly as `dataSources` returns it (for
    /// example "Federal Reserve Economic Data (FRED)"), and `externalId` is the source's series
    /// id (for example "GDP"). Returns null when no such series exists.
    async fn series_by_external_id(
        &self,
        ctx: &Context<'_>,
        source_name: String,
        external_id: String,
    ) -> Result<Option<EconomicSeriesType>> {
        let pool = ctx.data::<DatabasePool>()?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::{data_sources, economic_series};

        let mut conn = pool.get().await?;
        let series = economic_series::table
            .inner_join(data_sources::table)
            .filter(data_sources::name.eq(&source_name))
            .filter(economic_series::external_id.eq(&external_id))
            .select(EconomicSeries::as_select())
            .first::<EconomicSeries>(&mut conn)
            .await
            .optional()?;

        Ok(series.map(EconomicSeriesType::from))
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
        let series = series_service::list_series(&pool, search_params).await?;

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

        use diesel::{ExpressionMethods, OptionalExtension, QueryDsl};
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::data_sources::dsl;

        let mut conn = pool.get().await?;
        let source = dsl::data_sources
            .filter(dsl::id.eq(source_uuid))
            .select(DataSource::as_select())
            .first::<econ_graph_core::models::DataSource>(&mut conn)
            .await
            .optional()?;

        Ok(source.map(|s| s.into()))
    }

    /// One measure of one dataset, for every value of one dimension, at one date.
    ///
    /// `filter` pins every dataset dimension except `across`, e.g.
    /// `crossSection(datasetId: $wdi, filter: [{dimension: "indicator", value:
    /// "NY.GDP.PCAP.CD"}], across: "area", latest: true)`. Give exactly one of `date` and
    /// `latest: true`; `latest` returns each key's most recent non-null value with its own
    /// date. `measure` defaults to the dataset's default measure. Every active matching series
    /// is returned, ordered by key, with a null value where it has none.
    ///
    /// `asOf` reads each series as it was known on that day: its newest revision published on
    /// or before it, as data-point vintages record (see the series `asOf` filter). Observations
    /// first published after `asOf` are treated as not yet known. Omit it to read the current
    /// revision.
    #[allow(clippy::too_many_arguments)]
    async fn cross_section(
        &self,
        ctx: &Context<'_>,
        dataset_id: ID,
        measure: Option<String>,
        #[graphql(default)] filter: Vec<DimensionFilterInput>,
        across: String,
        date: Option<chrono::NaiveDate>,
        latest: Option<bool>,
        as_of: Option<chrono::NaiveDate>,
    ) -> Result<Vec<CrossSectionEntry>> {
        cross_section::resolve(
            ctx, dataset_id, measure, filter, across, date, latest, as_of,
        )
        .await
    }

    /// List all data sources
    async fn data_sources(&self, ctx: &Context<'_>) -> Result<Vec<DataSourceType>> {
        let pool = ctx.data::<DatabasePool>()?;

        use diesel::dsl::exists;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::{data_sources, economic_series};

        let mut conn = pool.get().await?;
        // Excludes sources with no series that has data (e.g. a source whose crawler adapter
        // was removed before any series it discovered ever got data points).
        let sources = data_sources::table
            .filter(exists(
                economic_series::table.filter(
                    economic_series::source_id
                        .eq(data_sources::id)
                        .and(economic_series::end_date.is_not_null()),
                ),
            ))
            .order_by(data_sources::name.asc())
            .select(DataSource::as_select())
            .load::<econ_graph_core::models::DataSource>(&mut *conn)
            .await?;

        Ok(sources.into_iter().map(DataSourceType::from).collect())
    }

    /// List datasets, optionally only one source's, grouped by source id (in no particular
    /// order across sources) and by code within a source. Unpaginated: sources publish a
    /// handful of datasets each.
    async fn datasets(
        &self,
        ctx: &Context<'_>,
        source_id: Option<ID>,
    ) -> Result<Vec<crate::graphql::datasets::DatasetType>> {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::datasets;

        let pool = ctx.data::<DatabasePool>()?;
        let mut query = datasets::table.into_boxed();
        if let Some(source_id) = source_id {
            let source_id = Uuid::parse_str(&source_id)
                .map_err(|_| async_graphql::Error::new("invalid data source id"))?;
            query = query.filter(datasets::source_id.eq(source_id));
        }

        let mut conn = pool.get().await?;
        let rows = query
            .order_by((datasets::source_id.asc(), datasets::code.asc()))
            .select(models::Dataset::as_select())
            .load::<models::Dataset>(&mut conn)
            .await?;

        Ok(rows
            .into_iter()
            .map(crate::graphql::datasets::DatasetType)
            .collect())
    }

    /// Get a dataset by ID
    async fn dataset(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> Result<Option<crate::graphql::datasets::DatasetType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let id =
            Uuid::parse_str(&id).map_err(|_| async_graphql::Error::new("invalid dataset id"))?;
        let dataset = models::Dataset::find_by_id(pool, id).await?;
        Ok(dataset.map(crate::graphql::datasets::DatasetType))
    }

    /// Get data points for a specific series with filtering and transformation, one page at a
    /// time in date order. `totalCount` counts every matching point; read the next page with
    /// `after: pageInfo.endCursor` until `pageInfo.hasNextPage` is false. A transformed page
    /// has the values it would have in the whole series.
    ///
    /// A series can hold several revisions of one date's value. A `transformation` is computed
    /// over the latest revisions unless the filter picks a revision mode (`asOf`, `originalOnly`,
    /// or `latestRevisionOnly: false` for every stored revision).
    async fn series_data(
        &self,
        ctx: &Context<'_>,
        series_id: ID,
        filter: Option<DataFilterInput>,
        transformation: Option<DataTransformationType>,
        #[graphql(desc = "Page size. Defaults to and is capped at 10000; must not be negative.")]
        first: Option<i32>,
        #[graphql(
            desc = "Start after this cursor (a previous page's endCursor). A cursor is the number of points up to and including the one it names."
        )]
        after: Option<String>,
    ) -> Result<DataPointConnection> {
        let pool = ctx.data::<DatabasePool>()?;
        let series_uuid = Uuid::parse_str(&series_id)?;

        if first.is_some_and(|first| first < 0) {
            return Err(async_graphql::Error::new("first must not be negative"));
        }
        // A cursor is the number of matching points up to and including the one it names, so
        // `after: endCursor` continues where the last page stopped.
        let offset = match after {
            Some(cursor) => match cursor.parse::<i64>() {
                Ok(offset) if offset >= 0 => offset,
                _ => {
                    return Err(async_graphql::Error::new(format!(
                        "Invalid cursor: {cursor}"
                    )))
                }
            },
            None => 0,
        };

        let transformation =
            transformation.filter(|&transformation| transformation != DataTransformationType::None);

        // A transformation compares neighbouring points, so over every stored revision it would
        // compare a date's revisions with one another. Unless the caller chose a revision mode
        // (`asOf`, `originalOnly`, or an explicit `latestRevisionOnly`), transform the latest
        // revisions.
        let latest_revision_only = filter
            .as_ref()
            .and_then(|f| f.latest_revision_only)
            .or_else(|| {
                let chose_mode = filter
                    .as_ref()
                    .is_some_and(|f| f.as_of.is_some() || f.original_only == Some(true));
                (transformation.is_some() && !chose_mode).then_some(true)
            });

        // Convert GraphQL inputs to service parameters
        let query_params = models::DataQueryParams {
            series_id: series_uuid,
            start_date: filter.as_ref().and_then(|f| f.start_date),
            end_date: filter.as_ref().and_then(|f| f.end_date),
            original_only: filter.as_ref().and_then(|f| f.original_only),
            latest_revision_only,
            as_of: filter.as_ref().and_then(|f| f.as_of),
            limit: first.map(i64::from),
            offset: Some(offset),
        };

        let context = transformation.and_then(transformation_context);
        let page = series_service::get_series_data(pool, query_params, context).await?;
        let has_next_page = page.has_next_page();
        let page_len = page.points.len() as i64;

        let points = match transformation {
            Some(transformation) => {
                // A page after the first transforms with the earlier points it compares against,
                // so each point gets the value it would have in the whole series.
                let context_len = page.context.len();
                let mut points = page.context;
                points.extend(page.points);
                apply_data_transformation(points, transformation)
                    .await?
                    .into_iter()
                    .skip(context_len)
                    .collect()
            }
            None => page.points,
        };

        let cursor = |position: i64| Some(position.to_string());
        Ok(DataPointConnection {
            nodes: points.into_iter().map(DataPointType::from).collect(),
            total_count: i32::try_from(page.total_count).unwrap_or(i32::MAX),
            page_info: PageInfo {
                has_next_page,
                has_previous_page: page.offset > 0,
                start_cursor: if page_len > 0 {
                    cursor(page.offset + 1)
                } else {
                    None
                },
                // Set even on an empty page, so `after: endCursor` never moves backwards.
                end_cursor: cursor(page.offset + page_len),
            },
        })
    }

    /// Crawler status, derived from the crawl queue (workers keep no other state; requires
    /// `admin.system:read`).
    async fn crawler_status(&self, ctx: &Context<'_>) -> Result<CrawlerStatusType> {
        require_role(ctx, Role::AdminSystemRead)?;
        let pool = ctx.data::<DatabasePool>()?;
        let snapshot = econ_graph_crawler::status::crawler_status(pool).await?;
        Ok(CrawlerStatusType::from_snapshot(snapshot))
    }

    /// Get queue statistics (requires `admin.system:read`)
    async fn queue_statistics(&self, ctx: &Context<'_>) -> Result<QueueStatisticsType> {
        require_role(ctx, Role::AdminSystemRead)?;
        let pool = ctx.data::<DatabasePool>()?;

        let stats = queue_service::get_queue_statistics(&pool).await?;

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
        let search_params = search::SearchParams {
            query: query.clone(),
            similarity_threshold: Some(0.3),
            limit: first,
            offset: after.and_then(|cursor| cursor.parse::<i32>().ok()),
            source_id: source.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            frequency: frequency.map(|f| format!("{:?}", f)),
            include_inactive: Some(false),
            sort_by: Some(SearchSortOrder::Relevance),
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

    /// Get annotations for a specific series: public ones, plus the caller's own private ones.
    async fn annotations_for_series(
        &self,
        ctx: &Context<'_>,
        series_id: String,
        #[graphql(deprecation = "Ignored: the viewer is the signed-in caller")] user_id: Option<ID>,
    ) -> Result<Vec<ChartAnnotationType>> {
        let _ = user_id;
        let viewer = current_user_id_opt(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = CollaborationService::new(pool.clone());

        let annotations = collaboration_service
            .get_annotations_for_series(&series_id, viewer)
            .await?;
        Ok(annotations
            .into_iter()
            .map(ChartAnnotationType::from)
            .collect())
    }

    /// Get comments for an annotation the caller can see (public, or their own)
    async fn comments_for_annotation(
        &self,
        ctx: &Context<'_>,
        annotation_id: ID,
    ) -> Result<Vec<AnnotationCommentType>> {
        let viewer = current_user_id_opt(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = CollaborationService::new(pool.clone());

        let annotation_uuid = uuid::Uuid::parse_str(&annotation_id)?;
        let comments = collaboration_service
            .get_comments_for_annotation(annotation_uuid, viewer)
            .await?;
        Ok(comments
            .into_iter()
            .map(AnnotationCommentType::from)
            .collect())
    }

    /// Get collaborators for a specific chart. Requires sign-in; only the chart's own
    /// collaborators see the list, everyone else gets an empty one.
    async fn chart_collaborators(
        &self,
        ctx: &Context<'_>,
        chart_id: ID,
    ) -> Result<Vec<ChartCollaboratorType>> {
        let viewer = current_user(ctx)?.id;
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = CollaborationService::new(pool.clone());

        let chart_uuid = uuid::Uuid::parse_str(&chart_id)?;
        let collaborators = collaboration_service
            .get_collaborators(chart_uuid, viewer)
            .await?;
        Ok(collaborators
            .into_iter()
            .map(|(collaborator, _user)| ChartCollaboratorType::from(collaborator))
            .collect())
    }

    /// Get user information by ID. Callers may read their own record; reading anyone else's
    /// requires `admin.users:read`.
    async fn user(&self, ctx: &Context<'_>, user_id: ID) -> Result<Option<UserType>> {
        // Authenticate before parsing, so anonymous callers never see input validation errors.
        let caller_id = current_user(ctx)?.id;
        let user_uuid = uuid::Uuid::parse_str(&user_id)?;

        if caller_id != user_uuid {
            require_role(ctx, Role::AdminUsersRead)?;
        }

        let pool = ctx.data::<DatabasePool>()?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;

        let mut conn = pool.get().await?;

        let user = users::table
            .filter(users::id.eq(user_uuid))
            .select(models::User::as_select())
            .first::<models::User>(&mut conn)
            .await
            .optional()?;

        Ok(user.map(UserType::from))
    }

    // Admin Queries

    /// Get all users (requires `admin.users:read`)
    async fn users(
        &self,
        ctx: &Context<'_>,
        filter: Option<UserFilterInput>,
        pagination: Option<PaginationInput>,
    ) -> Result<UserConnection> {
        require_role(ctx, Role::AdminUsersRead)?;
        let pool = ctx.data::<DatabasePool>()?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;

        let mut conn = pool.get().await?;

        // Build query with filters
        let mut query = users::table.into_boxed();

        if let Some(filter) = &filter {
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

        let users_list: Vec<models::User> = query
            .select(models::User::as_select())
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

    /// Get system health metrics (requires `admin.system:read`)
    async fn system_health(&self, ctx: &Context<'_>) -> Result<SystemHealthType> {
        require_role(ctx, Role::AdminSystemRead)?;
        let pool = ctx.data::<DatabasePool>()?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::{crawl_queue, users};

        let mut conn = pool.get().await?;

        // Get user counts
        let total_users: i64 = users::table.count().get_result(&mut conn).await?;

        let active_users: i64 = users::table
            .filter(users::last_login_at.gt(Utc::now() - chrono::Duration::hours(24)))
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
                database_size_mb: 0.0, // Would need special query for this
                queue_items: queue_items as i32,
                api_requests_per_minute: 0.0, // Would need metrics collection
                average_response_time_ms: 0.0, // Would need metrics collection
            },
            last_updated: Utc::now(),
        })
    }

    /// Get security events (requires `admin.security:read`)
    async fn security_events(
        &self,
        ctx: &Context<'_>,
        _limit: Option<i32>,
    ) -> Result<Vec<SecurityEventType>> {
        require_role(ctx, Role::AdminSecurityRead)?;
        let _context = ctx.data::<crate::graphql::schema::SchemaResources>()?;

        // Get security events logic would go here
        // For now, return empty vector
        Ok(vec![])
    }

    /// Get audit logs (requires `admin.audit:read`)
    async fn audit_logs(
        &self,
        ctx: &Context<'_>,
        _filter: Option<AuditLogFilterInput>,
        _pagination: Option<PaginationInput>,
    ) -> Result<AuditLogConnection> {
        require_role(ctx, Role::AdminAuditRead)?;
        let _context = ctx.data::<crate::graphql::schema::SchemaResources>()?;

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
}

/// Convert GraphQL series filter to service parameters
fn convert_series_filter_to_params(
    filter: Option<SeriesFilterInput>,
) -> econ_graph_core::models::SeriesSearchParams {
    let filter = filter.unwrap_or_default();

    econ_graph_core::models::SeriesSearchParams {
        query: filter.search_query,
        source_id: filter
            .source_id
            .and_then(|id| Uuid::parse_str(id.as_ref()).ok()),
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
    data_points: Vec<econ_graph_core::models::DataPoint>,
    transformation: DataTransformationType,
) -> Result<Vec<econ_graph_core::models::DataPoint>> {
    use bigdecimal::{BigDecimal, Zero};
    use econ_graph_core::models::data_point::DataTransformation;

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
    sorted_points.sort_by_key(|a| a.date);

    let mut transformed_points = Vec::new();

    match transform_type {
        DataTransformation::YearOverYear => {
            // For YoY, we need to find the value from exactly one year ago
            for (_i, point) in sorted_points.iter().enumerate() {
                // A point approximately one year earlier (some flexibility for exact dates)
                let previous_year_value =
                    earliest_in_window(&sorted_points, point.date, YOY_WINDOW.0, YOY_WINDOW.1)
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
                // ~3 months with flexibility
                let previous_quarter_value =
                    earliest_in_window(&sorted_points, point.date, QOQ_WINDOW.0, QOQ_WINDOW.1)
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
                // ~1 month with flexibility
                let previous_month_value =
                    earliest_in_window(&sorted_points, point.date, MOM_WINDOW.0, MOM_WINDOW.1)
                        .and_then(|p| p.value.as_ref());

                let transformed_value = point.calculate_mom_change(previous_month_value);

                let mut transformed_point = point.clone();
                transformed_point.value = transformed_value;
                transformed_points.push(transformed_point);
            }
        }

        DataTransformation::PercentChange => {
            // For percent change, compare each point with the series' earliest usable value: a
            // null or zero reading at the very first date (or several) would otherwise blank the
            // whole series, so skip leading points without one. Without any usable base every
            // point is empty, but still returned.
            let base_value = sorted_points
                .iter()
                .find_map(|p| p.value.clone().filter(|base| !base.is_zero()));
            for point in &sorted_points {
                let transformed_value = match (&point.value, &base_value) {
                    (Some(current_value), Some(base_value)) => {
                        Some(((current_value - base_value) / base_value) * BigDecimal::from(100))
                    }
                    _ => None,
                };

                let mut transformed_point = point.clone();
                transformed_point.value = transformed_value;
                transformed_points.push(transformed_point);
            }
        }

        DataTransformation::LogDifference => {
            // For log difference, calculate ln(current) - ln(previous)
            for (i, point) in sorted_points.iter().enumerate() {
                let transformed_value = if i > 0 {
                    let prev_point = &sorted_points[i - 1];
                    match (&point.value, &prev_point.value) {
                        (Some(current), Some(previous)) => log_difference(current, previous),
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

/// The first point in `sorted` (ordered by date) dated `min_days` to `max_days` days before
/// `date`. A binary search, so a transform of a full page stays linear-logarithmic.
fn earliest_in_window(
    sorted: &[econ_graph_core::models::DataPoint],
    date: chrono::NaiveDate,
    min_days: i64,
    max_days: i64,
) -> Option<&econ_graph_core::models::DataPoint> {
    let days_before = |days| {
        date.checked_sub_signed(chrono::Duration::days(days))
            .unwrap_or(chrono::NaiveDate::MIN)
    };
    let (from, to) = (days_before(max_days), days_before(min_days));
    let first = sorted.partition_point(|p| p.date < from);
    sorted.get(first).filter(|p| p.date <= to)
}

/// Look-back window (min, max days before), shared by [`apply_data_transformation`]'s lookup and
/// [`transformation_context`]'s page context, so a page always sees enough history to match.
const YOY_WINDOW: (i64, i64) = (360, 370);
const QOQ_WINDOW: (i64, i64) = (85, 95);
const MOM_WINDOW: (i64, i64) = (28, 32);

/// The earlier points `transformation` compares a point with, if any. The windows cover the
/// look-back ranges in [`apply_data_transformation`].
fn transformation_context(
    transformation: DataTransformationType,
) -> Option<series_service::PageContext> {
    use series_service::PageContext;
    match transformation {
        DataTransformationType::None => None,
        DataTransformationType::YearOverYear => Some(PageContext::Days(YOY_WINDOW.1)),
        DataTransformationType::QuarterOverQuarter => Some(PageContext::Days(QOQ_WINDOW.1)),
        DataTransformationType::MonthOverMonth => Some(PageContext::Days(MOM_WINDOW.1)),
        DataTransformationType::LogDifference => Some(PageContext::PreviousPoint),
        DataTransformationType::PercentChange => Some(PageContext::FirstPoint),
    }
}

/// `ln(current) - ln(previous)`, or `None` unless both values are positive. Computed in `f64`,
/// which is plenty for a displayed growth rate (a positive value too small for `f64` counts as
/// zero).
fn log_difference(
    current: &bigdecimal::BigDecimal,
    previous: &bigdecimal::BigDecimal,
) -> Option<bigdecimal::BigDecimal> {
    use bigdecimal::ToPrimitive;
    let (current, previous) = (current.to_f64()?, previous.to_f64()?);
    if !(current > 0.0 && previous > 0.0) {
        return None;
    }
    let diff = current.ln() - previous.ln();
    if !diff.is_finite() {
        return None;
    }
    // f64's Display is the shortest string that round-trips, so no binary noise digits.
    diff.to_string().parse().ok()
}

impl Default for Query {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod series_data_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn test_user(label: &str) -> models::User {
        let now = chrono::Utc::now();
        models::User {
            id: uuid::Uuid::new_v4(),
            email: format!("{label}@example.test"),
            name: label.into(),
            avatar_url: None,
            organization: None,
            theme: "light".into(),
            default_chart_type: "line".into(),
            notifications_enabled: false,
            collaboration_enabled: false,
            is_active: true,
            email_verified: true,
            created_at: now,
            updated_at: now,
            last_login_at: None,
        }
    }

    /// A pool that never connects: the authorization check must reject the request before
    /// any database access, so this test needs no database.
    fn unreachable_pool() -> DatabasePool {
        let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
            diesel_async::AsyncPgConnection,
        >::new("postgres://nobody@127.0.0.1:1/none");
        DatabasePool::builder()
            .connection_timeout(std::time::Duration::from_secs(1))
            .build_unchecked(manager)
    }

    #[tokio::test]
    async fn test_user_query_rejects_other_users_before_db_access() {
        let other_user_id = uuid::Uuid::new_v4();
        let query = format!(r#"{{ user(userId: "{other_user_id}") {{ id email }} }}"#);
        for u in [
            None,
            Some(test_user("guest")),
            Some(test_user("viewer")),
            Some(test_user("analyst")),
        ] {
            let label = u.as_ref().map(|u| u.name.clone());
            let schema = crate::graphql::schema::create_schema_with_data(
                unreachable_pool(),
                std::sync::Arc::new(crate::graphql::context::GraphQLContext::for_test_user(
                    u, false,
                )),
            );
            let resp = schema.execute(query.as_str()).await;
            assert_eq!(resp.errors.len(), 1, "{label:?}: {:?}", resp.errors);
            let msg = &resp.errors[0].message;
            let expected = if label.is_some() {
                "Insufficient permissions"
            } else {
                "Authentication required"
            };
            assert!(
                msg.contains(expected),
                "{label:?}: expected {expected}, got {msg}"
            );
        }
    }

    #[tokio::test]
    async fn test_user_query_requires_authentication_before_parsing_the_id() {
        let schema = crate::graphql::schema::create_schema_with_data(
            unreachable_pool(),
            std::sync::Arc::new(crate::graphql::context::GraphQLContext::anonymous()),
        );
        let resp = schema
            .execute(r#"{ user(userId: "not-a-uuid") { id } }"#)
            .await;
        assert_eq!(resp.errors.len(), 1, "{:?}", resp.errors);
        assert!(
            resp.errors[0].message.contains("Authentication required"),
            "{}",
            resp.errors[0].message
        );
    }

    #[tokio::test]
    async fn test_user_query_lets_self_and_admin_past_the_check() {
        // Both reach the database lookup, which fails here because the pool never connects.
        let viewer = test_user("viewer");
        let viewer_id = viewer.id;
        for (u, target, staff) in [
            (viewer, viewer_id, false),
            (test_user("admin"), uuid::Uuid::new_v4(), true),
        ] {
            let label = u.name.clone();
            let schema = crate::graphql::schema::create_schema_with_data(
                unreachable_pool(),
                std::sync::Arc::new(crate::graphql::context::GraphQLContext::for_test_user(
                    Some(u),
                    staff,
                )),
            );
            let query = format!(r#"{{ user(userId: "{target}") {{ id }} }}"#);
            let resp = schema.execute(query.as_str()).await;
            assert_eq!(resp.errors.len(), 1, "{label}: {:?}", resp.errors);
            let msg = &resp.errors[0].message;
            assert!(
                !msg.contains("permissions") && !msg.contains("Authentication"),
                "{label}: should pass the authorization check, got {msg}"
            );
        }
    }

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
    fn test_series_frequency_type_round_trips_through_normalizer() {
        // REQUIREMENT: every SeriesFrequencyType variant must still be recognized by
        // SeriesFrequency::from after going through the `format!("{:?}", f)` conversion used
        // here and in search_series, so a future rename of a variant can't silently break
        // frequency filtering by falling through to Irregular.
        use econ_graph_core::models::economic_series::SeriesFrequency;

        for (variant, expected) in [
            (SeriesFrequencyType::Daily, SeriesFrequency::Daily),
            (SeriesFrequencyType::Weekly, SeriesFrequency::Weekly),
            (SeriesFrequencyType::Monthly, SeriesFrequency::Monthly),
            (SeriesFrequencyType::Quarterly, SeriesFrequency::Quarterly),
            (SeriesFrequencyType::SemiAnnual, SeriesFrequency::SemiAnnual),
            (SeriesFrequencyType::Annual, SeriesFrequency::Annual),
        ] {
            let debug_name = format!("{:?}", variant);
            assert_eq!(
                SeriesFrequency::from(debug_name.clone()),
                expected,
                "{debug_name:?} should normalize to {expected:?}"
            );
        }
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

/// DB-backed tests for hiding series (and their sources) that have no data points, e.g. a
/// series discovered by a since-removed source adapter that was never crawled.
#[cfg(test)]
mod empty_series_tests {
    use crate::graphql::schema::create_schema;
    use async_graphql::Request;
    use econ_graph_core::models::{DataSource, EconomicSeries, NewDataSource, NewEconomicSeries};
    use econ_graph_core::test_utils::{get_test_db, test_dataset_id};
    use serial_test::serial;
    use uuid::Uuid;

    /// Creates a data source and, under it, two series sharing `word` in their title: one with
    /// an `end_date` (has data) and one without (discovered but never crawled).
    async fn seed(
        pool: &econ_graph_core::database::DatabasePool,
        word: &str,
    ) -> (DataSource, EconomicSeries, EconomicSeries) {
        let unique = Uuid::new_v4();
        let source = DataSource::create(
            pool,
            NewDataSource {
                name: format!("empty-series-test-{unique}"),
                description: None,
                base_url: "https://example.test".to_string(),
                api_key_required: false,
                rate_limit_per_minute: 60,
                is_visible: true,
                is_enabled: true,
                requires_admin_approval: false,
                crawl_frequency_hours: 24,
                api_documentation_url: None,
                api_key_name: None,
            },
        )
        .await
        .expect("create data source");
        let dataset_id =
            test_dataset_id(&mut pool.get().await.expect("connection"), source.id).await;

        let with_data = EconomicSeries::create(
            pool,
            &NewEconomicSeries {
                source_id: source.id,
                external_id: format!("{word}_with_data_{unique}"),
                title: format!("{word} With Data {unique}"),
                description: None,
                units: None,
                frequency: "Monthly".to_string(),
                seasonal_adjustment: None,
                start_date: chrono::NaiveDate::from_ymd_opt(2020, 1, 1),
                end_date: chrono::NaiveDate::from_ymd_opt(2020, 12, 1),
                is_active: true,
                first_discovered_at: None,
                last_crawled_at: None,
                first_missing_date: None,
                crawl_status: None,
                crawl_error_message: None,
                dataset_id,
                dimensions: Default::default(),
                default_measure: None,
            },
        )
        .await
        .expect("create series with data");

        let without_data = EconomicSeries::create(
            pool,
            &NewEconomicSeries {
                source_id: source.id,
                external_id: format!("{word}_no_data_{unique}"),
                title: format!("{word} No Data {unique}"),
                description: None,
                units: None,
                frequency: "Monthly".to_string(),
                seasonal_adjustment: None,
                start_date: None,
                end_date: None,
                is_active: true,
                first_discovered_at: None,
                last_crawled_at: None,
                first_missing_date: None,
                crawl_status: None,
                crawl_error_message: None,
                dataset_id,
                dimensions: Default::default(),
                default_measure: None,
            },
        )
        .await
        .expect("create series without data");

        (source, with_data, without_data)
    }

    #[tokio::test]
    #[serial]
    async fn search_series_excludes_series_with_no_data() {
        let container = get_test_db().await;
        let pool = container.pool().clone();
        let schema = create_schema(pool.clone());

        let (_source, with_data, without_data) = seed(&pool, "Zyxquartile").await;

        let query = r#"{ searchSeries(query: "Zyxquartile") { series { id } } }"#;
        let resp = schema.execute(Request::new(query)).await;
        assert!(resp.errors.is_empty(), "{:?}", resp.errors);

        let ids: Vec<String> = resp.data.into_json().unwrap()["searchSeries"]["series"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap().to_string())
            .collect();

        assert!(ids.contains(&with_data.id.to_string()));
        assert!(!ids.contains(&without_data.id.to_string()));
    }

    #[tokio::test]
    #[serial]
    async fn series_list_excludes_series_with_no_data() {
        let container = get_test_db().await;
        let pool = container.pool().clone();
        let schema = create_schema(pool.clone());

        let (source, with_data, without_data) = seed(&pool, "Wavequotient").await;

        let query = format!(
            r#"{{ seriesList(filter: {{ sourceId: "{}" }}) {{ nodes {{ id }} }} }}"#,
            source.id
        );
        let resp = schema.execute(Request::new(query.as_str())).await;
        assert!(resp.errors.is_empty(), "{:?}", resp.errors);

        let ids: Vec<String> = resp.data.into_json().unwrap()["seriesList"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap().to_string())
            .collect();

        assert!(ids.contains(&with_data.id.to_string()));
        assert!(!ids.contains(&without_data.id.to_string()));
    }

    #[tokio::test]
    #[serial]
    async fn data_sources_excludes_source_with_no_series_that_has_data() {
        let container = get_test_db().await;
        let pool = container.pool().clone();
        let schema = create_schema(pool.clone());

        // A source whose only series has data.
        let (source_with_data, _with_data, _without_data) = seed(&pool, "Rhoultimeter").await;

        // A second source whose only series has no data.
        let unique = Uuid::new_v4();
        let empty_source = DataSource::create(
            &pool,
            NewDataSource {
                name: format!("empty-series-test-empty-source-{unique}"),
                description: None,
                base_url: "https://example.test".to_string(),
                api_key_required: false,
                rate_limit_per_minute: 60,
                is_visible: true,
                is_enabled: true,
                requires_admin_approval: false,
                crawl_frequency_hours: 24,
                api_documentation_url: None,
                api_key_name: None,
            },
        )
        .await
        .expect("create empty data source");
        let dataset_id =
            test_dataset_id(&mut pool.get().await.expect("connection"), empty_source.id).await;
        EconomicSeries::create(
            &pool,
            &NewEconomicSeries {
                source_id: empty_source.id,
                external_id: format!("empty_source_series_{unique}"),
                title: format!("Empty Source Series {unique}"),
                description: None,
                units: None,
                frequency: "Monthly".to_string(),
                seasonal_adjustment: None,
                start_date: None,
                end_date: None,
                is_active: true,
                first_discovered_at: None,
                last_crawled_at: None,
                first_missing_date: None,
                crawl_status: None,
                crawl_error_message: None,
                dataset_id,
                dimensions: Default::default(),
                default_measure: None,
            },
        )
        .await
        .expect("create series for empty source");

        let query = r#"{ dataSources { id } }"#;
        let resp = schema.execute(Request::new(query)).await;
        assert!(resp.errors.is_empty(), "{:?}", resp.errors);

        let ids: Vec<String> = resp.data.into_json().unwrap()["dataSources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap().to_string())
            .collect();

        assert!(ids.contains(&source_with_data.id.to_string()));
        assert!(!ids.contains(&empty_source.id.to_string()));
    }

    #[tokio::test]
    #[serial]
    async fn data_source_series_and_series_count_exclude_series_with_no_data() {
        let container = get_test_db().await;
        let pool = container.pool().clone();
        let schema = create_schema(pool.clone());

        // One source with a populated series and an empty one, mirroring a source whose
        // adapter was removed after discovering (but never crawling) some of its series.
        let (source, with_data, without_data) = seed(&pool, "Deltamixture").await;

        let query = r#"{ dataSources { id seriesCount series { nodes { id } } } }"#;
        let resp = schema.execute(Request::new(query)).await;
        assert!(resp.errors.is_empty(), "{:?}", resp.errors);

        let json = resp.data.into_json().unwrap();
        let entry = json["dataSources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"].as_str().unwrap() == source.id.to_string())
            .expect("seeded source present in dataSources");

        assert_eq!(entry["seriesCount"].as_i64().unwrap(), 1);

        let ids: Vec<String> = entry["series"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap().to_string())
            .collect();
        assert!(ids.contains(&with_data.id.to_string()));
        assert!(!ids.contains(&without_data.id.to_string()));
    }

    #[tokio::test]
    #[serial]
    async fn series_returns_a_clean_404_for_a_series_with_no_data() {
        let container = get_test_db().await;
        let pool = container.pool().clone();
        let schema = create_schema(pool.clone());

        let (_source, with_data, without_data) = seed(&pool, "Sigmajitter").await;

        for (id, should_exist) in [(with_data.id, true), (without_data.id, false)] {
            let query = format!(r#"{{ series(id: "{id}") {{ id }} }}"#);
            let resp = schema.execute(Request::new(query.as_str())).await;
            assert!(resp.errors.is_empty(), "{:?}", resp.errors);
            let json = resp.data.into_json().unwrap();
            assert_eq!(
                json["series"].is_null(),
                !should_exist,
                "series({id}) should{} be found",
                if should_exist { "" } else { " not" }
            );
        }
    }
}
