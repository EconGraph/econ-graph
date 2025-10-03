use async_graphql::*;

use crate::{
    database::DatabasePool,
    graphql::{context::require_admin, types::*},
    services::SecCrawlerService,
};
use std::sync::Arc;

/// Root mutation object
pub struct Mutation;

#[Object]
impl Mutation {
    /// Trigger a manual crawl for specific sources or series
    async fn trigger_crawl(
        &self,
        ctx: &Context<'_>,
        input: TriggerCrawlInput,
    ) -> Result<CrawlerStatusType> {
        let pool = ctx.data::<DatabasePool>()?;

        let mut _queued_items = Vec::new();

        // Handle multiple sources and series
        let sources = input.sources.unwrap_or_else(|| vec!["FRED".to_string()]);
        let series_ids = input.series_ids.unwrap_or_else(|| vec!["GDP".to_string()]);

        let items = crate::services::crawler::simple_crawler_service::trigger_manual_crawl(
            pool,
            Some(sources),
            Some(series_ids),
            1, // priority
        )
        .await?;
        _queued_items.push(items);

        // Return updated crawler status
        Ok(CrawlerStatusType {
            is_running: true,
            active_workers: 5,
            last_crawl: Some(chrono::Utc::now()),
            next_scheduled_crawl: Some(chrono::Utc::now() + chrono::Duration::hours(4)),
        })
    }

    /// Create a new chart annotation
    async fn create_annotation(
        &self,
        ctx: &Context<'_>,
        input: CreateAnnotationInput,
    ) -> Result<ChartAnnotationType> {
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = crate::services::CollaborationService::new(pool.clone());

        let user_id = uuid::Uuid::parse_str(&input.user_id)?;
        let series_id = uuid::Uuid::parse_str(&input.series_id)?;

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
                input.is_public.unwrap_or(false),
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
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = crate::services::CollaborationService::new(pool.clone());

        let user_id = uuid::Uuid::parse_str(&input.user_id)?;
        let annotation_id = uuid::Uuid::parse_str(&input.annotation_id)?;

        let comment = collaboration_service
            .add_comment(annotation_id, user_id, input.content)
            .await?;

        Ok(AnnotationCommentType::from(comment))
    }

    /// Share a chart with another user
    async fn share_chart(
        &self,
        ctx: &Context<'_>,
        input: ShareChartInput,
    ) -> Result<ChartCollaboratorType> {
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = crate::services::CollaborationService::new(pool.clone());

        let owner_user_id = uuid::Uuid::parse_str(&input.owner_user_id)?;
        let target_user_id = uuid::Uuid::parse_str(&input.target_user_id)?;
        let chart_id = uuid::Uuid::parse_str(&input.chart_id)?;

        let permission_level = match input.permission_level.to_lowercase().as_str() {
            "view" => crate::services::collaboration_service::PermissionLevel::View,
            "comment" => crate::services::collaboration_service::PermissionLevel::Comment,
            "edit" => crate::services::collaboration_service::PermissionLevel::Edit,
            "admin" => crate::services::collaboration_service::PermissionLevel::Admin,
            _ => crate::services::collaboration_service::PermissionLevel::View,
        };

        let collaborator = collaboration_service
            .share_chart(chart_id, owner_user_id, target_user_id, permission_level)
            .await?;

        Ok(ChartCollaboratorType::from(collaborator))
    }

    /// Delete an annotation
    async fn delete_annotation(
        &self,
        ctx: &Context<'_>,
        input: DeleteAnnotationInput,
    ) -> Result<bool> {
        let pool = ctx.data::<DatabasePool>()?;
        let collaboration_service = crate::services::CollaborationService::new(pool.clone());

        let user_id = uuid::Uuid::parse_str(&input.user_id)?;
        let annotation_id = uuid::Uuid::parse_str(&input.annotation_id)?;

        collaboration_service
            .delete_annotation(annotation_id, user_id)
            .await?;

        Ok(true)
    }

    // Admin User Management Mutations

    /// Create a new user (admin only)
    async fn create_user(&self, ctx: &Context<'_>, input: CreateUserInput) -> Result<UserType> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;

        use crate::models::User;
        use crate::schema::users;
        use bcrypt::{hash, DEFAULT_COST};
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        // Check if user already exists
        let existing_user: Option<User> = users::table
            .filter(users::email.eq(&input.email))
            .select(User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        if existing_user.is_some() {
            return Err(Error::new("User with this email already exists"));
        }

        // Hash password if provided
        let password_hash = if let Some(password) = &input.password {
            Some(
                hash(password, DEFAULT_COST)
                    .map_err(|e| Error::new(format!("Password hashing failed: {}", e)))?,
            )
        } else {
            None
        };

        // Create new user
        let new_user = crate::models::NewUser {
            email: input.email,
            name: input.name,
            avatar_url: None,
            provider: "email".to_string(),
            provider_id: None,
            password_hash,
            role: input.role,
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

    /// Update user information (admin only)
    async fn update_user(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdateUserInput,
    ) -> Result<UserType> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use crate::models::user::UpdateUser;
        use crate::models::User;
        use crate::schema::users;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        // Check if user exists
        let existing_user: Option<User> = users::table
            .filter(users::id.eq(user_id))
            .select(User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        let existing_user = existing_user.ok_or_else(|| Error::new("User not found"))?;

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
                    return Err(Error::new("User with this email already exists"));
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

        if let Some(role) = input.role {
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

    /// Delete a user (admin only)
    async fn delete_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use crate::schema::users;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        // Check if user exists
        let user_exists: Option<crate::models::User> = users::table
            .filter(users::id.eq(user_id))
            .select(crate::models::User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        if user_exists.is_none() {
            return Err(Error::new("User not found"));
        }

        // Delete user (cascade will handle related records)
        diesel::delete(users::table.filter(users::id.eq(user_id)))
            .execute(&mut conn)
            .await?;

        Ok(true)
    }

    /// Suspend a user account (admin only)
    async fn suspend_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use crate::schema::users;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        // Check if user exists
        let user_exists: Option<crate::models::User> = users::table
            .filter(users::id.eq(user_id))
            .select(crate::models::User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        if user_exists.is_none() {
            return Err(Error::new("User not found"));
        }

        // Suspend user
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::is_active.eq(false))
            .execute(&mut conn)
            .await?;

        Ok(true)
    }

    /// Activate a user account (admin only)
    async fn activate_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use crate::schema::users;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        // Check if user exists
        let user_exists: Option<crate::models::User> = users::table
            .filter(users::id.eq(user_id))
            .select(crate::models::User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        if user_exists.is_none() {
            return Err(Error::new("User not found"));
        }

        // Activate user
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::is_active.eq(true))
            .execute(&mut conn)
            .await?;

        Ok(true)
    }

    /// Force logout a user (admin only)
    async fn force_logout_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        // Require admin role
        let _admin_user = require_admin(ctx)?;
        let pool = ctx.data::<DatabasePool>()?;
        let user_id = uuid::Uuid::parse_str(&id)?;

        use crate::schema::{user_sessions, users};
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await?;

        // Check if user exists
        let user_exists: Option<crate::models::User> = users::table
            .filter(users::id.eq(user_id))
            .select(crate::models::User::as_select())
            .first(&mut conn)
            .await
            .optional()?;

        if user_exists.is_none() {
            return Err(Error::new("User not found"));
        }

        // Delete all active sessions for the user
        diesel::delete(user_sessions::table.filter(user_sessions::user_id.eq(user_id)))
            .execute(&mut conn)
            .await?;

        Ok(true)
    }

    // ============================================================================
    // SEC CRAWLER MUTATIONS
    // ============================================================================

    /// Trigger SEC crawl for a specific company
    async fn trigger_sec_crawl(
        &self,
        ctx: &Context<'_>,
        input: SecCrawlInput,
    ) -> Result<SecCrawlResult> {
        let pool = ctx.data::<DatabasePool>()?;
        let sec_crawler_service = SecCrawlerService::new(Arc::new(pool.clone()));
        
        // Trigger the crawl
        let result = sec_crawler_service
            .crawl_company(
                &input.cik,
                input.form_types,
                input.start_date,
                input.end_date,
                input.exclude_amended,
                input.exclude_restated,
                input.max_file_size,
            )
            .await?;

        // Convert to GraphQL result
        Ok(SecCrawlResult {
            operation_id: result.operation_id.into(),
            cik: result.cik,
            filings_downloaded: result.filings_downloaded,
            filings_processed: result.filings_processed,
            errors: result.errors,
            start_time: result.start_time,
            end_time: result.end_time,
            status: result.status,
        })
    }

    /// Import SEC EDGAR RSS feed
    async fn import_sec_rss(
        &self,
        ctx: &Context<'_>,
        input: SecRssImportInput,
    ) -> Result<SecRssImportResult> {
        let pool = ctx.data::<DatabasePool>()?;
        let sec_crawler_service = SecCrawlerService::new(Arc::new(pool.clone()));
        
        // Import RSS feed
        let result = sec_crawler_service
            .import_rss_feed(
                input.rss_url,
                input.max_filings,
                input.form_types,
            )
            .await?;

        // Convert to GraphQL result
        Ok(SecRssImportResult {
            operation_id: result.operation_id.into(),
            filings_imported: result.filings_imported,
            companies_added: result.companies_added,
            errors: result.errors,
            start_time: result.start_time,
            end_time: result.end_time,
            status: result.status,
        })
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
        };

        assert_eq!(input.sources, Some(vec!["FRED".to_string()]));
        assert_eq!(input.priority, Some(8));
    }
}
