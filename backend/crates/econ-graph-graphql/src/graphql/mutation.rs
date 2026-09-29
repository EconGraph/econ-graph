//! # GraphQL Mutation Resolvers
//!
//! This module contains all GraphQL mutation resolvers for the EconGraph API.
//! It provides write operations for data management, user administration, and system configuration.
//!
//! # Design Principles
//!
//! 1. **Authorization**: All mutations implement strict role-based access control
//! 2. **Data Integrity**: All mutations validate input and maintain data consistency
//! 3. **Audit Trail**: All mutations are logged for security and compliance
//! 4. **Error Handling**: Comprehensive error handling with rollback capabilities
//!
//! # Quality Standards
//!
//! - All mutations must implement proper authorization checks
//! - Input validation must be comprehensive and secure
//! - Database transactions must be atomic and consistent
//! - All mutations must have comprehensive documentation

use crate::imports::*;
use crate::types::*;

/// Root mutation object
pub struct Mutation;

#[Object]
impl Mutation {
    /// Enqueue crawl jobs (requires `admin.crawlers:manage`). Nothing is crawled in the
    /// request: the jobs go to `crawl_queue` and the crawler workers process them.
    ///
    /// `series_ids` become `fetch_series` jobs for `source` (or for the single entry of
    /// `sources`); every listed source without series gets a `discover_catalog` job. Returns the
    /// current crawler status with `enqueuedCount` = jobs actually inserted (a job with an active
    /// duplicate in the queue is skipped and not counted).
    async fn trigger_crawl(
        &self,
        ctx: &Context<'_>,
        input: TriggerCrawlInput,
    ) -> Result<CrawlerStatusType> {
        let caller = require_role(ctx, Role::AdminCrawlersManage)?;
        let pool = ctx.data::<DatabasePool>()?;

        let jobs = plan_trigger_crawl(&input).map_err(GraphQLError::new)?;
        let mut enqueued = 0i32;
        for job in &jobs {
            if core_models::CrawlQueueItem::enqueue(pool, job)
                .await?
                .is_some()
            {
                enqueued += 1;
            }
        }
        tracing::info!(
            user_id = %caller.user_id,
            requested = jobs.len(),
            enqueued,
            "triggerCrawl enqueued jobs"
        );

        let snapshot = econ_graph_crawler::status::crawler_status(pool).await?;
        let mut status = CrawlerStatusType::from_snapshot(snapshot);
        status.enqueued_count = Some(enqueued);
        Ok(status)
    }

    /// Create a new chart annotation
    async fn create_annotation(
        &self,
        ctx: &Context<'_>,
        input: CreateAnnotationInput,
    ) -> Result<ChartAnnotationType> {
        // The author is the signed-in caller, never an id from the request.
        let user_id = require_role(ctx, Role::AnnotationCreate)?.user_id;
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = CollaborationService::new(pool.clone());

        let series_id = uuid::Uuid::parse_str(&input.series_id)?;
        let is_public = input.is_public.unwrap_or(false);

        let annotation = collaboration_service
            .create_annotation(
                user_id,
                series_id,
                input.annotation_date,
                input.annotation_value,
                input.title,
                input.content,
                input.annotation_type,
                input.color,
                is_public,
            )
            .await?;

        Ok(ChartAnnotationType::from(annotation))
    }

    /// Add a comment to an annotation
    async fn add_comment(
        &self,
        ctx: &Context<'_>,
        input: AddCommentInput,
    ) -> Result<AnnotationCommentType> {
        let user_id = require_role(ctx, Role::AnnotationComment)?.user_id;
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = CollaborationService::new(pool.clone());

        let annotation_id = uuid::Uuid::parse_str(&input.annotation_id)?;

        let comment = collaboration_service
            .add_comment(user_id, annotation_id, input.content)
            .await?;

        Ok(AnnotationCommentType::from(comment))
    }

    /// Share a chart with another user
    async fn share_chart(
        &self,
        ctx: &Context<'_>,
        input: ShareChartInput,
    ) -> Result<ChartCollaboratorType> {
        // The sharer is the signed-in caller, who must already be an admin on the chart.
        let owner_user_id = require_role(ctx, Role::ChartShare)?.user_id;
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = CollaborationService::new(pool.clone());

        let target_user_id = uuid::Uuid::parse_str(&input.target_user_id)?;
        let chart_id = uuid::Uuid::parse_str(&input.chart_id)?;

        let permission_level = match input.permission_level.to_lowercase().as_str() {
            "view" => PermissionLevel::View,
            "comment" => PermissionLevel::Comment,
            "edit" => PermissionLevel::Edit,
            "admin" => PermissionLevel::Admin,
            _ => PermissionLevel::View,
        };

        let collaborator = collaboration_service
            .share_chart(chart_id, owner_user_id, target_user_id, permission_level)
            .await?;

        Ok(ChartCollaboratorType::from(collaborator))
    }

    /// Update an annotation (only its author may update it)
    async fn update_annotation(
        &self,
        ctx: &Context<'_>,
        input: UpdateAnnotationInput,
    ) -> Result<ChartAnnotationType> {
        // Same role as creating one: updating can publish new (or newly public) content,
        // unlike delete, which only removes it.
        let user_id = require_role(ctx, Role::AnnotationCreate)?.user_id;
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = CollaborationService::new(pool.clone());

        let annotation_id = uuid::Uuid::parse_str(&input.annotation_id)?;
        let is_public = input.is_public;

        let annotation = collaboration_service
            .update_annotation(
                annotation_id,
                user_id,
                input.title,
                input.content,
                input.color,
                input.annotation_type,
                is_public,
            )
            .await?;

        Ok(ChartAnnotationType::from(annotation))
    }

    /// Delete an annotation
    async fn delete_annotation(
        &self,
        ctx: &Context<'_>,
        input: DeleteAnnotationInput,
    ) -> Result<bool> {
        let user_id = current_user(ctx)?.id;
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = CollaborationService::new(pool.clone());

        let annotation_id = uuid::Uuid::parse_str(&input.annotation_id)?;

        collaboration_service
            .delete_annotation(annotation_id, user_id)
            .await?;

        Ok(true)
    }

    // Admin User Management Mutations

    /// Update user information (requires `admin.users:update`; changing `isActive` also
    /// requires `admin.users:suspend`). Roles are assigned in the identity provider.
    async fn update_user(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdateUserInput,
    ) -> Result<UserType> {
        require_role(ctx, Role::AdminUsersUpdate)?;
        if input.is_active.is_some() {
            require_role(ctx, Role::AdminUsersSuspend)?;
        }
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::models::User;
        use econ_graph_core::schema::users;

        let changes = AdminUserChangeset {
            email: input.email,
            name: input.name,
            avatar_url: input.avatar_url,
            organization: input.organization,
            theme: input.theme,
            default_chart_type: input.default_chart_type,
            notifications_enabled: input.notifications_enabled,
            collaboration_enabled: input.collaboration_enabled,
            email_verified: input.email_verified,
            is_active: input.is_active,
        };
        if changes.is_empty() {
            return Err(GraphQLError::new("No fields to update"));
        }
        let mut conn = pool.get().await?;
        let final_user: User = diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(&changes)
            .returning(User::as_select())
            .get_result(&mut conn)
            .await
            .map_err(|error| match error {
                diesel::result::Error::NotFound => GraphQLError::new("User not found"),
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    info,
                ) if info.constraint_name() == Some("users_email_key") => {
                    GraphQLError::new("User with this email already exists")
                }
                other => other.into(),
            })?;

        Ok(UserType::from(final_user))
    }

    /// Delete a user (requires `admin.users:delete`)
    async fn delete_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        require_role(ctx, Role::AdminUsersDelete)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;

        let mut conn = pool.get().await?;

        // Delete user (cascade will handle related records)
        let deleted = diesel::delete(users::table.filter(users::id.eq(user_id)))
            .execute(&mut conn)
            .await?;
        if deleted == 0 {
            return Err(GraphQLError::new("User not found"));
        }
        Ok(true)
    }

    /// Suspend a user account (requires `admin.users:suspend`)
    async fn suspend_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        require_role(ctx, Role::AdminUsersSuspend)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;

        let mut conn = pool.get().await?;

        // Suspend user
        let updated = diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::is_active.eq(false))
            .execute(&mut conn)
            .await?;
        if updated == 0 {
            return Err(GraphQLError::new("User not found"));
        }
        Ok(true)
    }

    /// Activate a user account (requires `admin.users:suspend`)
    async fn activate_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        require_role(ctx, Role::AdminUsersSuspend)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;

        let mut conn = pool.get().await?;

        // Activate user
        let updated = diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::is_active.eq(true))
            .execute(&mut conn)
            .await?;
        if updated == 0 {
            return Err(GraphQLError::new("User not found"));
        }
        Ok(true)
    }
}

/// Turns a `triggerCrawl` input into queue rows, or a validation message.
///
/// Series are never guessed onto a source: they need `source`, or exactly one entry in `sources`.
fn plan_trigger_crawl(
    input: &TriggerCrawlInput,
) -> std::result::Result<Vec<core_models::NewCrawlQueueItem>, String> {
    use core_models::JobKind;
    use econ_graph_crawler::cli::{new_job, CATALOG_SERIES_ID, DEFAULT_PRIORITY};
    use econ_graph_crawler::SourceId;

    let priority = input.priority.unwrap_or(DEFAULT_PRIORITY);
    if !(1..=10).contains(&priority) {
        return Err(format!(
            "priority must be between 1 and 10 (got {priority})"
        ));
    }
    // Sources with no adapter in this build (release leaves out the static catalogs; IMF has
    // none at all) would otherwise queue a job the worker can only fail with "no adapter".
    // SEC has no adapter in the registry either: it's fetched via `fetch_filing`, not `plan_trigger_crawl`.
    let registry = econ_graph_crawler::sources::default_registry();
    let parse = |s: &str| {
        let trimmed = s.trim();
        let id = trimmed
            .parse::<SourceId>()
            .map_err(|_| format!("unknown source {trimmed:?}"))?;
        if id != SourceId::Sec && registry.get(id).is_none() {
            return Err(format!("source {trimmed:?} is not available in this build"));
        }
        Ok(id)
    };
    let mut sources: Vec<SourceId> = Vec::new();
    for s in input
        .sources
        .iter()
        .flatten()
        .filter(|s| !s.trim().is_empty())
    {
        let id = parse(s)?;
        if !sources.contains(&id) {
            sources.push(id);
        }
    }
    let explicit = match input.source.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => Some(parse(s)?),
        _ => None,
    };
    let mut series: Vec<&str> = Vec::new();
    for s in input.series_ids.iter().flatten().map(|s| s.trim()) {
        if !s.is_empty() && !series.contains(&s) {
            series.push(s);
        }
    }

    let mut jobs = Vec::new();
    let series_source = if series.is_empty() {
        if let Some(id) = explicit {
            if !sources.contains(&id) {
                sources.push(id);
            }
        }
        None
    } else {
        let id = match (explicit, sources.as_slice()) {
            (Some(id), _) => id,
            (None, [only]) => *only,
            (None, []) => {
                return Err("seriesIds need a source: set `source` (or list exactly one entry in `sources`)".into())
            }
            (None, _) => {
                return Err("seriesIds are ambiguous with several `sources`: set `source` to the source they belong to".into())
            }
        };
        for s in &series {
            jobs.push(new_job(id, s, JobKind::FetchSeries, priority));
        }
        Some(id)
    };
    for id in sources.into_iter().filter(|id| Some(*id) != series_source) {
        jobs.push(new_job(
            id,
            CATALOG_SERIES_ID,
            JobKind::DiscoverCatalog,
            priority,
        ));
    }
    if jobs.is_empty() {
        return Err("nothing to crawl: give `sources` and/or `seriesIds`".into());
    }
    Ok(jobs)
}

impl Default for Mutation {
    fn default() -> Self {
        Self
    }
}

/// `None` leaves a column alone. This never rewrites values from an earlier read.
#[derive(diesel::AsChangeset)]
#[diesel(table_name = econ_graph_core::schema::users)]
struct AdminUserChangeset {
    email: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
    organization: Option<String>,
    theme: Option<String>,
    default_chart_type: Option<String>,
    notifications_enabled: Option<bool>,
    collaboration_enabled: Option<bool>,
    email_verified: Option<bool>,
    is_active: Option<bool>,
}

impl AdminUserChangeset {
    fn is_empty(&self) -> bool {
        self.email.is_none()
            && self.name.is_none()
            && self.avatar_url.is_none()
            && self.organization.is_none()
            && self.theme.is_none()
            && self.default_chart_type.is_none()
            && self.notifications_enabled.is_none()
            && self.collaboration_enabled.is_none()
            && self.email_verified.is_none()
            && self.is_active.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_crawl_input() {
        // Test that the input type can be created
        let input = TriggerCrawlInput {
            sources: Some(vec!["FRED".to_string()]),
            series_ids: Some(vec!["GDP".to_string()]),
            priority: Some(8),
            source: None,
        };

        assert_eq!(input.sources, Some(vec!["FRED".to_string()]));
        assert_eq!(input.priority, Some(8));
    }

    fn input(
        sources: &[&str],
        series: &[&str],
        source: Option<&str>,
        priority: Option<i32>,
    ) -> TriggerCrawlInput {
        let v = |xs: &[&str]| (!xs.is_empty()).then(|| xs.iter().map(|s| s.to_string()).collect());
        TriggerCrawlInput {
            sources: v(sources),
            series_ids: v(series),
            priority,
            source: source.map(str::to_string),
        }
    }

    fn summary(jobs: &[core_models::NewCrawlQueueItem]) -> Vec<(String, String, String, i32)> {
        jobs.iter()
            .map(|j| {
                (
                    j.source.clone(),
                    j.series_id.clone(),
                    j.kind.clone(),
                    j.priority,
                )
            })
            .collect()
    }

    fn row(source: &str, series: &str, kind: &str, priority: i32) -> (String, String, String, i32) {
        (source.into(), series.into(), kind.into(), priority)
    }

    #[test]
    fn test_trigger_crawl_plan() {
        // Single listed source pairs unambiguously with the series.
        let jobs =
            plan_trigger_crawl(&input(&["fred"], &["GDP", " UNRATE ", "GDP"], None, None)).unwrap();
        assert_eq!(
            summary(&jobs),
            vec![
                row("FRED", "GDP", "fetch_series", 5),
                row("FRED", "UNRATE", "fetch_series", 5)
            ]
        );
        // Explicit source for the series; other listed sources get discovery.
        let jobs =
            plan_trigger_crawl(&input(&["FRED", "BLS"], &["GDP"], Some("FRED"), Some(9))).unwrap();
        assert_eq!(
            summary(&jobs),
            vec![
                row("FRED", "GDP", "fetch_series", 9),
                row("BLS", "catalog", "discover_catalog", 9)
            ]
        );
        // Sources alone: discovery for each.
        let jobs = plan_trigger_crawl(&input(&["WORLD_BANK"], &[], None, None)).unwrap();
        assert_eq!(
            summary(&jobs),
            vec![row("WORLD_BANK", "catalog", "discover_catalog", 5)]
        );
        // `source` without series is a discovery request too.
        let jobs = plan_trigger_crawl(&input(&[], &[], Some("BLS"), None)).unwrap();
        assert_eq!(
            summary(&jobs),
            vec![row("BLS", "catalog", "discover_catalog", 5)]
        );
    }

    #[test]
    fn test_trigger_crawl_plan_rejects_invalid_input() {
        let err = |i| plan_trigger_crawl(&i).unwrap_err();
        assert!(err(input(&[], &["GDP"], None, None)).contains("need a source"));
        assert!(err(input(&["FRED", "BLS"], &["GDP"], None, None)).contains("ambiguous"));
        assert!(err(input(&["NOPE"], &[], None, None)).contains("unknown source"));
        assert!(err(input(&[], &["GDP"], Some("NOPE"), None)).contains("unknown source"));
        // IMF has no adapter in any build (its series ids were made up). The static catalogs
        // (ECB and friends) can't be asserted against here: they register in this same test
        // binary's dev-profile build via `debug_assertions`, so only a --release build excludes
        // them (see the crawler crate's own `default_registry_holds_exactly_the_live_sources`).
        assert!(err(input(&["IMF"], &[], None, None)).contains("not available in this build"));
        assert!(err(input(&["FRED"], &[], None, Some(0))).contains("priority"));
        assert!(err(input(&["FRED"], &[], None, Some(11))).contains("priority"));
        assert!(err(input(&[], &[], None, None)).contains("nothing to crawl"));
    }

    // ---------------------------------------------------------------------
    // DB-backed: need DATABASE_URL (skipped otherwise); they empty crawl_queue.
    // ---------------------------------------------------------------------

    use crate::graphql::TEST_DB_LOCK as DB_LOCK;

    async fn db() -> Option<(DatabasePool, tokio::sync::MutexGuard<'static, ()>)> {
        use diesel_async::RunQueryDsl;
        let Ok(url) = std::env::var("DATABASE_URL") else {
            eprintln!("DATABASE_URL not set; skipping DB-backed triggerCrawl test");
            return None;
        };
        let guard = DB_LOCK.lock().await;
        econ_graph_core::database::run_migrations(&url)
            .await
            .expect("migrations");
        let pool = econ_graph_core::database::create_pool(&url)
            .await
            .expect("pool");
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM crawl_queue")
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        Some((pool, guard))
    }

    fn user(label: &str) -> User {
        let now = chrono::Utc::now();
        User {
            id: Uuid::new_v4(),
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

    async fn run_as(
        pool: &DatabasePool,
        user: Option<User>,
        query: &str,
    ) -> async_graphql::Response {
        let staff = user.as_ref().is_some_and(|u| is_staff_label(&u.name));
        let schema = crate::graphql::schema::create_schema_with_data(
            pool.clone(),
            Arc::new(crate::graphql::context::GraphQLContext::for_test_user(
                user, staff,
            )),
        );
        schema.execute(query).await
    }

    /// Whether a `user()` test helper label should hold every staff role.
    fn is_staff_label(label: &str) -> bool {
        matches!(label, "admin" | "super_admin")
    }

    async fn queue_rows(pool: &DatabasePool) -> Vec<(String, String, String)> {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::crawl_queue::dsl as q;
        let mut conn = pool.get().await.unwrap();
        q::crawl_queue
            .order((q::source.asc(), q::series_id.asc()))
            .select((q::source, q::series_id, q::kind))
            .load(&mut conn)
            .await
            .unwrap()
    }

    const TRIGGER: &str = r#"mutation { triggerCrawl(input: { source: "FRED", seriesIds: ["GDP", "UNRATE"], sources: ["BLS"], priority: 7 }) { isRunning activeWorkers enqueuedCount sources { source pending } } }"#;

    #[tokio::test]
    async fn test_trigger_crawl_rejects_non_admin() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        for u in [None, Some(user("viewer")), Some(user("analyst"))] {
            let resp = run_as(&pool, u, TRIGGER).await;
            assert_eq!(resp.errors.len(), 1, "{resp:?}");
            let msg = &resp.errors[0].message;
            assert!(
                msg.contains("Authentication required") || msg.contains("Insufficient permissions"),
                "{msg}"
            );
        }
        assert!(
            queue_rows(&pool).await.is_empty(),
            "nothing may be enqueued"
        );
    }

    #[tokio::test]
    async fn test_trigger_crawl_enqueues_for_admin_without_double_counting() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let resp = run_as(&pool, Some(user("admin")), TRIGGER).await;
        assert!(resp.errors.is_empty(), "{:?}", resp.errors);
        let data = resp.data.into_json().unwrap();
        let status = &data["triggerCrawl"];
        assert_eq!(status["enqueuedCount"], 3);
        assert_eq!(status["activeWorkers"], 0);
        assert_eq!(status["isRunning"], false);
        assert_eq!(
            status["sources"],
            serde_json::json!([{"source": "BLS", "pending": 1}, {"source": "FRED", "pending": 2}])
        );
        assert_eq!(
            queue_rows(&pool).await,
            vec![
                ("BLS".into(), "catalog".into(), "discover_catalog".into()),
                ("FRED".into(), "GDP".into(), "fetch_series".into()),
                ("FRED".into(), "UNRATE".into(), "fetch_series".into()),
            ]
        );

        // Same request again: every job already active -> nothing new, nothing counted.
        let resp = run_as(&pool, Some(user("admin")), TRIGGER).await;
        assert!(resp.errors.is_empty(), "{:?}", resp.errors);
        assert_eq!(
            resp.data.into_json().unwrap()["triggerCrawl"]["enqueuedCount"],
            0
        );

        // Partial overlap counts only the new job.
        let resp = run_as(
            &pool,
            Some(user("super_admin")),
            r#"mutation { triggerCrawl(input: { sources: ["FRED"], seriesIds: ["GDP", "PAYEMS"] }) { enqueuedCount } }"#,
        )
        .await;
        assert!(resp.errors.is_empty(), "{:?}", resp.errors);
        assert_eq!(
            resp.data.into_json().unwrap()["triggerCrawl"]["enqueuedCount"],
            1
        );
        assert_eq!(queue_rows(&pool).await.len(), 4);

        // The crawlerStatus query reads the same queue.
        let resp = run_as(&pool, None, "{ crawlerStatus { isRunning activeWorkers lastCrawl nextScheduledCrawl enqueuedCount sources { source pending failedLastDay } } }").await;
        assert!(resp.errors.is_empty(), "{:?}", resp.errors);
        let data = resp.data.into_json().unwrap();
        assert_eq!(
            data["crawlerStatus"]["enqueuedCount"],
            serde_json::Value::Null
        );
        assert_eq!(data["crawlerStatus"]["sources"][1]["pending"], 3);
        assert!(data["crawlerStatus"]["nextScheduledCrawl"].is_string());
    }

    #[tokio::test]
    async fn test_trigger_crawl_validation_error_for_unpaired_series() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let resp = run_as(
            &pool,
            Some(user("admin")),
            r#"mutation { triggerCrawl(input: { seriesIds: ["GDP"] }) { enqueuedCount } }"#,
        )
        .await;
        assert_eq!(resp.errors.len(), 1);
        assert!(
            resp.errors[0].message.contains("need a source"),
            "{:?}",
            resp.errors
        );
        assert!(queue_rows(&pool).await.is_empty());
    }

    /// A pool that never connects: the authorization check must reject the request before
    /// any database access, so these tests need no database.
    fn unreachable_pool() -> DatabasePool {
        let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
            diesel_async::AsyncPgConnection,
        >::new("postgres://nobody@127.0.0.1:1/none");
        DatabasePool::builder()
            .connection_timeout(std::time::Duration::from_secs(1))
            .build_unchecked(manager)
    }

    #[tokio::test]
    async fn test_trigger_crawl_rejects_non_admin_before_db_access() {
        let query = r#"mutation { triggerCrawl(input: { sources: ["FRED"], seriesIds: ["GDP"] }) { isRunning } }"#;
        for u in [
            None,
            Some(user("guest")),
            Some(user("viewer")),
            Some(user("analyst")),
        ] {
            let label = u.as_ref().map(|u| u.name.clone());
            let schema = crate::graphql::schema::create_schema_with_data(
                unreachable_pool(),
                Arc::new(crate::graphql::context::GraphQLContext::for_test_user(
                    u, false,
                )),
            );
            let resp = schema.execute(query).await;
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
}

/// The collaboration API acts as the signed-in caller, never as a user id from the request.
#[cfg(test)]
mod collaboration_auth_tests {
    use crate::graphql::context::GraphQLContext;
    use crate::graphql::schema::create_schema_with_data;
    use econ_graph_core::DatabasePool;
    use std::sync::Arc;

    /// A pool that never connects: every case here must stop before any database access.
    fn unreachable_pool() -> DatabasePool {
        let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
            diesel_async::AsyncPgConnection,
        >::new("postgres://nobody@127.0.0.1:1/none");
        DatabasePool::builder()
            .connection_timeout(std::time::Duration::from_secs(1))
            .build_unchecked(manager)
    }

    /// Anonymous callers are refused by every collaboration mutation, and by the
    /// `chartCollaborators` query, before it touches the database.
    #[tokio::test]
    async fn collaboration_mutations_require_a_signed_in_caller() {
        let id = uuid::Uuid::new_v4();
        let mutations = [
            format!(
                r#"mutation {{ createAnnotation(input: {{ seriesId: "{id}", annotationDate: "2024-01-01", title: "t", content: "c", annotationType: "note" }}) {{ id }} }}"#
            ),
            format!(
                r#"mutation {{ addComment(input: {{ annotationId: "{id}", content: "c" }}) {{ id }} }}"#
            ),
            format!(
                r#"mutation {{ shareChart(input: {{ targetUserId: "{id}", chartId: "{id}", permissionLevel: "admin" }}) {{ id }} }}"#
            ),
            format!(r#"mutation {{ deleteAnnotation(input: {{ annotationId: "{id}" }}) }}"#),
            format!(r#"{{ chartCollaborators(chartId: "{id}") {{ id }} }}"#),
        ];
        let schema =
            create_schema_with_data(unreachable_pool(), Arc::new(GraphQLContext::anonymous()));
        for mutation in mutations {
            let resp = schema.execute(mutation.as_str()).await;
            assert_eq!(resp.errors.len(), 1, "{mutation}: {:?}", resp.errors);
            assert!(
                resp.errors[0].message.contains("Authentication required"),
                "{mutation}: {}",
                resp.errors[0].message
            );
        }
    }

    /// The inputs no longer accept a user id, so a caller cannot act as someone else.
    #[tokio::test]
    async fn collaboration_inputs_reject_caller_supplied_user_ids() {
        let id = uuid::Uuid::new_v4();
        let mutations = [
            format!(
                r#"mutation {{ createAnnotation(input: {{ userId: "{id}", seriesId: "{id}", annotationDate: "2024-01-01", title: "t", content: "c", annotationType: "note" }}) {{ id }} }}"#
            ),
            format!(
                r#"mutation {{ addComment(input: {{ userId: "{id}", annotationId: "{id}", content: "c" }}) {{ id }} }}"#
            ),
            format!(
                r#"mutation {{ shareChart(input: {{ ownerUserId: "{id}", targetUserId: "{id}", chartId: "{id}", permissionLevel: "admin" }}) {{ id }} }}"#
            ),
            format!(
                r#"mutation {{ deleteAnnotation(input: {{ userId: "{id}", annotationId: "{id}" }}) }}"#
            ),
        ];
        let schema =
            create_schema_with_data(unreachable_pool(), Arc::new(GraphQLContext::anonymous()));
        for mutation in mutations {
            let resp = schema.execute(mutation.as_str()).await;
            assert!(
                resp.errors
                    .iter()
                    .any(|e| e.message.contains("userId") || e.message.contains("ownerUserId")),
                "{mutation}: {:?}",
                resp.errors
            );
        }
    }
}
