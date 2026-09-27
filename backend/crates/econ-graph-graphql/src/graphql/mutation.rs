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
        // `isPublic` is canonical; `isVisible` is a deprecated alias kept for older clients.
        let is_public = input.is_public.or(input.is_visible).unwrap_or(false);

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
        // `isPublic` is canonical; `isVisible` is a deprecated alias kept for older clients.
        let is_public = input.is_public.or(input.is_visible);

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

    /// Create a new user (requires `admin.users:create`, and every staff role the new user's
    /// `role` grants)
    async fn create_user(&self, ctx: &Context<'_>, input: CreateUserInput) -> Result<UserType> {
        let caller = require_role(ctx, Role::AdminUsersCreate)?;
        let role = require_grantable(ctx, caller, &input.role)?;
        let pool = ctx.data::<DatabasePool>()?;

        use bcrypt::{hash, DEFAULT_COST};
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::models::User;
        use econ_graph_core::schema::users;

        let mut conn = pool.get().await?;

        // Check if user already exists
        let existing_user: Option<User> = users::table
            .filter(users::email.eq(&input.email))
            .select(User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        if existing_user.is_some() {
            return Err(GraphQLError::new("User with this email already exists"));
        }

        // Hash password if provided
        let password_hash = if let Some(password) = &input.password {
            Some(
                hash(password, DEFAULT_COST)
                    .map_err(|e| GraphQLError::new(format!("Password hashing failed: {}", e)))?,
            )
        } else {
            None
        };

        // Create new user
        let new_user = models::NewUser {
            email: input.email,
            name: input.name,
            avatar_url: None,
            provider: "email".to_string(),
            provider_id: None,
            password_hash,
            role,
            organization: input.organization,
            theme: "light".to_string(),
            default_chart_type: "line".to_string(),
            notifications_enabled: true,
            collaboration_enabled: true,
            email_verified: false,
        };

        let user = diesel::insert_into(users::table)
            .values(&new_user)
            .returning(User::as_select())
            .get_result(&mut conn)
            .await?;

        Ok(UserType::from(user))
    }

    /// Update user information (requires `admin.users:update`; changing `role` also requires
    /// every staff role it grants, and changing `isActive` requires `admin.users:suspend`)
    async fn update_user(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdateUserInput,
    ) -> Result<UserType> {
        let caller = require_role(ctx, Role::AdminUsersUpdate)?;
        let role = input
            .role
            .as_deref()
            .map(|role| require_grantable(ctx, caller, role))
            .transpose()?;
        if input.is_active.is_some() {
            require_role(ctx, Role::AdminUsersSuspend)?;
        }
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::models::user::UpdateUser;
        use econ_graph_core::models::User;
        use econ_graph_core::schema::users;

        let mut conn = pool.get().await?;

        // Check if user exists
        let existing_user: Option<User> = users::table
            .filter(users::id.eq(user_id))
            .select(User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        let existing_user = existing_user.ok_or_else(|| GraphQLError::new("User not found"))?;
        require_manageable(ctx, caller, &existing_user)?;

        // Check if email is being changed and if it already exists
        if let Some(new_email) = &input.email {
            if new_email != &existing_user.email {
                let email_exists: Option<User> = users::table
                    .filter(users::email.eq(new_email))
                    .filter(users::id.ne(user_id))
                    .select(User::as_select())
                    .first(&mut conn)
                    .await
                    .optional()?;

                if email_exists.is_some() {
                    return Err(GraphQLError::new("User with this email already exists"));
                }
            }
        }

        // Build update struct with only provided fields
        let update_data = UpdateUser {
            name: input.name.or(Some(existing_user.name)),
            avatar_url: input.avatar_url.or(existing_user.avatar_url),
            organization: input.organization.or(existing_user.organization),
            theme: input.theme.or(Some(existing_user.theme)),
            default_chart_type: input
                .default_chart_type
                .or(Some(existing_user.default_chart_type)),
            notifications_enabled: input
                .notifications_enabled
                .or(Some(existing_user.notifications_enabled)),
            collaboration_enabled: input
                .collaboration_enabled
                .or(Some(existing_user.collaboration_enabled)),
            last_login_at: existing_user.last_login_at,
        };

        // Update user
        let updated_user = diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(&update_data)
            .returning(User::as_select())
            .get_result(&mut conn)
            .await?;

        // Update role and email_verified if provided (these require separate updates)
        let mut final_user = updated_user;

        if let Some(role) = role {
            final_user = diesel::update(users::table.filter(users::id.eq(user_id)))
                .set(users::role.eq(role))
                .returning(User::as_select())
                .get_result(&mut conn)
                .await?;
        }

        if let Some(email_verified) = input.email_verified {
            final_user = diesel::update(users::table.filter(users::id.eq(user_id)))
                .set(users::email_verified.eq(email_verified))
                .returning(User::as_select())
                .get_result(&mut conn)
                .await?;
        }

        if let Some(email) = input.email {
            final_user = diesel::update(users::table.filter(users::id.eq(user_id)))
                .set(users::email.eq(email))
                .returning(User::as_select())
                .get_result(&mut conn)
                .await?;
        }

        if let Some(is_active) = input.is_active {
            final_user = diesel::update(users::table.filter(users::id.eq(user_id)))
                .set(users::is_active.eq(is_active))
                .returning(User::as_select())
                .get_result(&mut conn)
                .await?;
        }

        Ok(UserType::from(final_user))
    }

    /// Delete a user (requires `admin.users:delete`)
    async fn delete_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let caller = require_role(ctx, Role::AdminUsersDelete)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;

        let mut conn = pool.get().await?;

        // Check if user exists
        let user_exists: Option<models::User> = users::table
            .filter(users::id.eq(user_id))
            .select(models::User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        let target = user_exists.ok_or_else(|| GraphQLError::new("User not found"))?;
        require_manageable(ctx, caller, &target)?;

        // Delete user (cascade will handle related records)
        diesel::delete(users::table.filter(users::id.eq(user_id)))
            .execute(&mut conn)
            .await?;

        Ok(true)
    }

    /// Suspend a user account (requires `admin.users:suspend`)
    async fn suspend_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let caller = require_role(ctx, Role::AdminUsersSuspend)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;

        let mut conn = pool.get().await?;

        // Check if user exists
        let user_exists: Option<models::User> = users::table
            .filter(users::id.eq(user_id))
            .select(models::User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        let target = user_exists.ok_or_else(|| GraphQLError::new("User not found"))?;
        require_manageable(ctx, caller, &target)?;

        // Suspend user
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::is_active.eq(false))
            .execute(&mut conn)
            .await?;

        Ok(true)
    }

    /// Activate a user account (requires `admin.users:suspend`)
    async fn activate_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let caller = require_role(ctx, Role::AdminUsersSuspend)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;

        let mut conn = pool.get().await?;

        // Check if user exists
        let user_exists: Option<models::User> = users::table
            .filter(users::id.eq(user_id))
            .select(models::User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        let target = user_exists.ok_or_else(|| GraphQLError::new("User not found"))?;
        require_manageable(ctx, caller, &target)?;

        // Activate user
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::is_active.eq(true))
            .execute(&mut conn)
            .await?;

        Ok(true)
    }

    /// Force logout a user (requires `admin.sessions:revoke`)
    async fn force_logout_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let caller = require_role(ctx, Role::AdminSessionsRevoke)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::{user_sessions, users};

        let mut conn = pool.get().await?;

        // Check if user exists
        let user_exists: Option<models::User> = users::table
            .filter(users::id.eq(user_id))
            .select(models::User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        let target = user_exists.ok_or_else(|| GraphQLError::new("User not found"))?;
        require_manageable(ctx, caller, &target)?;

        // Delete all active sessions for the user
        diesel::delete(user_sessions::table.filter(user_sessions::user_id.eq(user_id)))
            .execute(&mut conn)
            .await?;

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
    let parse = |s: &str| {
        s.trim()
            .parse::<SourceId>()
            .map_err(|_| format!("unknown source {:?}", s.trim()))
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

/// Whether `roles` includes a staff role the caller lacks.
fn exceeds_staff_roles(caller: &Principal, roles: &std::collections::BTreeSet<Role>) -> bool {
    roles
        .iter()
        .any(|&role| role.is_staff() && !caller.has_role(role))
}

/// Refuse with "Insufficient permissions", logging the denial like `require_role` does.
fn deny_user_admin(ctx: &Context<'_>, caller: &Principal, why: &str) -> GraphQLError {
    let context = ctx.data::<Arc<GraphQLContext>>().ok();
    tracing::warn!(
        request_id = context.map_or("unknown", |c| c.request_id.as_str()),
        client_ip = context
            .and_then(|c| c.client_ip.as_deref())
            .unwrap_or("unknown"),
        "User {} denied: {why}",
        caller.user_id
    );
    GraphQLError::new("Insufficient permissions")
}

/// Check a legacy `users.role` value the caller wants to give a user, and return it lowercased
/// for storage. A narrow staff role (say, `admin.users:update` alone) must not mint or become a
/// full admin, so the value may grant only staff roles the caller holds. Unknown values are
/// refused: the context would give such a user no roles at all.
fn require_grantable(ctx: &Context<'_>, caller: &Principal, legacy_role: &str) -> Result<String> {
    let granted = econ_graph_auth::roles_for_legacy(legacy_role)
        .map_err(|_| GraphQLError::new(format!("Unknown role: {legacy_role}")))?;
    if exceeds_staff_roles(caller, &granted) {
        return Err(deny_user_admin(
            ctx,
            caller,
            &format!("may not grant legacy role {legacy_role}"),
        ));
    }
    Ok(legacy_role.to_ascii_lowercase())
}

/// Refuse to act on a user who holds a staff role the caller lacks, so a narrow staff role
/// cannot edit, delete, suspend or log out a fuller admin. An unknown legacy role grants
/// nothing, so it never blocks. The target's roles come from `users.role`; when AUTH-5 drops
/// that column, this must read the target's roles from Keycloak instead.
fn require_manageable(ctx: &Context<'_>, caller: &Principal, target: &models::User) -> Result<()> {
    let target_roles = econ_graph_auth::roles_for_legacy(&target.role).unwrap_or_default();
    if exceeds_staff_roles(caller, &target_roles) {
        return Err(deny_user_admin(
            ctx,
            caller,
            &format!("user {} holds staff roles the caller lacks", target.id),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod user_admin_scope_tests {
    use super::*;

    fn target_roles(legacy: &str) -> std::collections::BTreeSet<Role> {
        econ_graph_auth::roles_for_legacy(legacy).unwrap_or_default()
    }

    #[test]
    fn a_narrow_staff_role_cannot_act_on_a_full_admin() {
        let caller = Principal::new(uuid::Uuid::new_v4(), [Role::AdminUsersUpdate]);
        for legacy in ["admin", "super_admin", "ADMIN"] {
            assert!(
                exceeds_staff_roles(&caller, &target_roles(legacy)),
                "{legacy}"
            );
        }
        for legacy in ["viewer", "analyst", "guest", "root"] {
            assert!(
                !exceeds_staff_roles(&caller, &target_roles(legacy)),
                "{legacy}"
            );
        }
    }

    #[test]
    fn a_full_admin_can_act_on_anyone() {
        let caller = Principal::new(uuid::Uuid::new_v4(), Role::all().iter().copied());
        for legacy in ["admin", "super_admin", "viewer"] {
            assert!(
                !exceeds_staff_roles(&caller, &target_roles(legacy)),
                "{legacy}"
            );
        }
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

    fn user(role: &str) -> User {
        let now = chrono::Utc::now();
        User {
            id: Uuid::new_v4(),
            email: format!("{role}@example.test"),
            name: role.into(),
            avatar_url: None,
            provider: "email".into(),
            provider_id: None,
            password_hash: None,
            role: role.into(),
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
        let schema = crate::graphql::schema::create_schema_with_data(
            pool.clone(),
            Arc::new(crate::graphql::context::GraphQLContext::new(user)),
        );
        schema.execute(query).await
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
            let role = u.as_ref().map(|u| u.role.clone());
            let schema = crate::graphql::schema::create_schema_with_data(
                unreachable_pool(),
                Arc::new(crate::graphql::context::GraphQLContext::new(u)),
            );
            let resp = schema.execute(query).await;
            assert_eq!(resp.errors.len(), 1, "{role:?}: {:?}", resp.errors);
            let msg = &resp.errors[0].message;
            let expected = if role.is_some() {
                "Insufficient permissions"
            } else {
                "Authentication required"
            };
            assert!(
                msg.contains(expected),
                "{role:?}: expected {expected}, got {msg}"
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
            create_schema_with_data(unreachable_pool(), Arc::new(GraphQLContext::new(None)));
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
            create_schema_with_data(unreachable_pool(), Arc::new(GraphQLContext::new(None)));
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
