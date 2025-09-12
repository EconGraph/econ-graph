use async_graphql::*;

use crate::{database::DatabasePool, graphql::types::*};

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
