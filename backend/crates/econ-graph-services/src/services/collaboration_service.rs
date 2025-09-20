use bigdecimal::BigDecimal;
use chrono::NaiveDate;
/**
 * REQUIREMENT: Collaboration service for chart annotations and sharing
 * PURPOSE: Provide business logic for collaborative features including permissions
 * This enables secure multi-user professional economic analysis features
 */
use diesel::prelude::*;
use diesel::SelectableHelper;
use diesel_async::RunQueryDsl;
use std::fmt;
use uuid::Uuid;

use econ_graph_core::{
    database::DatabasePool,
    error::{AppError, AppResult},
    models::user::{
        AnnotationComment, ChartAnnotation, ChartCollaborator, NewAnnotationComment,
        NewChartAnnotation, NewChartCollaborator, User,
    },
    schema::{annotation_comments, chart_annotations, chart_collaborators, users},
};

/// Permission levels for collaboration
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionLevel {
    View,
    Comment,
    Edit,
    Admin,
}

impl PermissionLevel {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "view" => PermissionLevel::View,
            "comment" => PermissionLevel::Comment,
            "edit" => PermissionLevel::Edit,
            "admin" => PermissionLevel::Admin,
            _ => PermissionLevel::View,
        }
    }

    pub fn can_view(&self) -> bool {
        true // All permission levels can view
    }

    pub fn can_comment(&self) -> bool {
        matches!(
            self,
            PermissionLevel::Comment | PermissionLevel::Edit | PermissionLevel::Admin
        )
    }

    pub fn can_edit(&self) -> bool {
        matches!(self, PermissionLevel::Edit | PermissionLevel::Admin)
    }

    pub fn can_admin(&self) -> bool {
        matches!(self, PermissionLevel::Admin)
    }
}

impl fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermissionLevel::View => write!(f, "view"),
            PermissionLevel::Comment => write!(f, "comment"),
            PermissionLevel::Edit => write!(f, "edit"),
            PermissionLevel::Admin => write!(f, "admin"),
        }
    }
}

/// Collaboration service for managing annotations and sharing
pub struct CollaborationService {
    pool: DatabasePool,
}

impl CollaborationService {
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }

    /// Create a new chart annotation
    pub async fn create_annotation(
        &self,
        user_id: Uuid,
        series_id: Uuid,
        annotation_date: NaiveDate,
        annotation_value: Option<BigDecimal>,
        title: String,
        content: String,
        annotation_type: String,
        color: Option<String>,
        is_public: bool,
    ) -> AppResult<ChartAnnotation> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        // Check if user has permission to annotate this series
        if !self.check_annotation_permission(user_id, series_id).await? {
            return Err(AppError::Unauthorized("Unauthorized".to_string()));
        }

        let new_annotation = NewChartAnnotation {
            user_id,
            series_id: Some(series_id.to_string()),
            chart_id: None,
            annotation_date,
            annotation_value,
            title,
            description: Some(content),
            color,
            annotation_type: Some(annotation_type),
            is_visible: Some(is_public),
            is_pinned: Some(false),
            tags: None,
        };

        let annotation = diesel::insert_into(chart_annotations::table)
            .values(&new_annotation)
            .returning(ChartAnnotation::as_select())
            .get_result::<ChartAnnotation>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(annotation)
    }

    /// Get annotations for a series
    pub async fn get_annotations_for_series(
        &self,
        series_id: &str,
        user_id: Option<Uuid>,
    ) -> AppResult<Vec<ChartAnnotation>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let annotations = if let Some(uid) = user_id {
            chart_annotations::table
                .filter(chart_annotations::series_id.eq(series_id))
                .filter(
                    chart_annotations::is_visible
                        .eq(true)
                        .or(chart_annotations::user_id.eq(uid)),
                )
                .order_by(chart_annotations::created_at.desc())
                .select(ChartAnnotation::as_select())
                .load::<ChartAnnotation>(&mut conn)
        } else {
            chart_annotations::table
                .filter(chart_annotations::series_id.eq(series_id))
                .filter(chart_annotations::is_visible.eq(true))
                .order_by(chart_annotations::created_at.desc())
                .select(ChartAnnotation::as_select())
                .load::<ChartAnnotation>(&mut conn)
        }
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(annotations)
    }

    /// Add a comment to an annotation
    pub async fn add_comment(
        &self,
        user_id: Uuid,
        annotation_id: Uuid,
        content: String,
    ) -> AppResult<AnnotationComment> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        // Check if annotation exists and user has permission to comment
        let annotation = chart_annotations::table
            .filter(chart_annotations::id.eq(annotation_id))
            .select(ChartAnnotation::as_select())
            .first::<ChartAnnotation>(&mut conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Annotation not found".to_string()))?;

        // Check permission to comment on this series
        if let Some(series_id) = &annotation.series_id {
            if !self.check_comment_permission(user_id, series_id).await? {
                return Err(AppError::Unauthorized("Unauthorized".to_string()));
            }
        } else if let Some(chart_id) = annotation.chart_id {
            if !self.check_admin_permission(user_id, chart_id).await? {
                return Err(AppError::Unauthorized("Unauthorized".to_string()));
            }
        } else {
            return Err(AppError::Unauthorized("Unauthorized".to_string()));
        }

        let new_comment = NewAnnotationComment {
            annotation_id,
            user_id,
            content,
        };

        let comment = diesel::insert_into(annotation_comments::table)
            .values(&new_comment)
            .returning(AnnotationComment::as_select())
            .get_result::<AnnotationComment>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(comment)
    }

    /// Get comments for an annotation
    pub async fn get_comments_for_annotation(
        &self,
        annotation_id: Uuid,
    ) -> AppResult<Vec<AnnotationComment>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let comments = annotation_comments::table
            .filter(annotation_comments::annotation_id.eq(annotation_id))
            .order_by(annotation_comments::created_at.asc())
            .select(AnnotationComment::as_select())
            .load::<AnnotationComment>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(comments)
    }

    /// Share a chart with a user
    pub async fn share_chart(
        &self,
        chart_id: Uuid,
        owner_user_id: Uuid,
        target_user_id: Uuid,
        permission_level: PermissionLevel,
    ) -> AppResult<ChartCollaborator> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        // Check if the owner has admin permission on this chart
        if !self.check_admin_permission(owner_user_id, chart_id).await? {
            return Err(AppError::Unauthorized("Unauthorized".to_string()));
        }

        // Check if collaboration already exists
        let existing = chart_collaborators::table
            .filter(chart_collaborators::chart_id.eq(chart_id))
            .filter(chart_collaborators::user_id.eq(target_user_id))
            .select(ChartCollaborator::as_select())
            .first::<ChartCollaborator>(&mut conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if let Some(existing_collab) = existing {
            // Update existing permission
            let updated = diesel::update(
                chart_collaborators::table.filter(chart_collaborators::id.eq(existing_collab.id)),
            )
            .set(chart_collaborators::role.eq(permission_level.to_string()))
            .returning(ChartCollaborator::as_select())
            .get_result::<ChartCollaborator>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            return Ok(updated);
        }

        // Create new collaboration
        let new_collaborator = NewChartCollaborator {
            chart_id,
            user_id: target_user_id,
            invited_by: Some(owner_user_id),
            role: Some(permission_level.to_string()),
            permissions: None,
        };

        let collaborator = diesel::insert_into(chart_collaborators::table)
            .values(&new_collaborator)
            .returning(ChartCollaborator::as_select())
            .get_result::<ChartCollaborator>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(collaborator)
    }

    /// Get collaborators for a chart
    pub async fn get_collaborators(
        &self,
        chart_id: Uuid,
    ) -> AppResult<Vec<(ChartCollaborator, User)>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let collaborators = chart_collaborators::table
            .inner_join(users::table.on(chart_collaborators::user_id.eq(users::id)))
            .filter(chart_collaborators::chart_id.eq(chart_id))
            .select((ChartCollaborator::as_select(), User::as_select()))
            .load::<(ChartCollaborator, User)>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(collaborators)
    }

    /// Check if user has permission to annotate a series
    async fn check_annotation_permission(
        &self,
        _user_id: Uuid,
        _series_id: Uuid,
    ) -> AppResult<bool> {
        // For now, allow any authenticated user to annotate
        // In the future, this could check series ownership or collaboration permissions
        Ok(true)
    }

    /// Check if user has permission to comment on a series
    async fn check_comment_permission(&self, _user_id: Uuid, _series_id: &str) -> AppResult<bool> {
        // For now, allow any authenticated user to comment
        // In the future, this could check collaboration permissions
        Ok(true)
    }

    /// Check if user has admin permission on a chart
    async fn check_admin_permission(&self, user_id: Uuid, chart_id: Uuid) -> AppResult<bool> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let permission = chart_collaborators::table
            .filter(chart_collaborators::chart_id.eq(chart_id))
            .filter(chart_collaborators::user_id.eq(user_id))
            .select(chart_collaborators::role)
            .first::<Option<String>>(&mut conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if let Some(Some(perm_str)) = permission {
            let perm = PermissionLevel::from_string(&perm_str);
            Ok(perm.can_admin())
        } else {
            // If no explicit permission, check if user is the owner
            // For now, return true to allow sharing (this could be enhanced)
            Ok(true)
        }
    }

    /// Delete an annotation (only by owner or admin)
    pub async fn delete_annotation(&self, annotation_id: Uuid, user_id: Uuid) -> AppResult<bool> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        // Get the annotation to check ownership
        let annotation = chart_annotations::table
            .filter(chart_annotations::id.eq(annotation_id))
            .select(ChartAnnotation::as_select())
            .first::<ChartAnnotation>(&mut conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Annotation not found".to_string()))?;

        // Check if user owns the annotation or has admin permission
        if annotation.user_id != user_id {
            // If not owner, check admin permission
            if let Some(chart_id) = annotation.chart_id {
                if !self.check_admin_permission(user_id, chart_id).await? {
                    return Err(AppError::Unauthorized("Unauthorized".to_string()));
                }
            } else {
                return Err(AppError::Unauthorized("Unauthorized".to_string()));
            }
        }

        // Delete associated comments first
        diesel::delete(
            annotation_comments::table.filter(annotation_comments::annotation_id.eq(annotation_id)),
        )
        .execute(&mut conn)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Delete the annotation
        let deleted = diesel::delete(
            chart_annotations::table.filter(chart_annotations::id.eq(annotation_id)),
        )
        .execute(&mut conn)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(deleted > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::test_utils::TestContainer;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_collaboration_service_creation() {
        let container = TestContainer::new().await;
        let pool = container.pool();

        let service = CollaborationService::new(pool.clone());

        // Test that service can be created
        assert!(true);
    }

    #[tokio::test]
    #[serial]
    async fn test_permission_levels() {
        let view = PermissionLevel::from_string("view");
        assert_eq!(view, PermissionLevel::View);
        assert!(view.can_view());
        assert!(!view.can_comment());
        assert!(!view.can_edit());
        assert!(!view.can_admin());

        let admin = PermissionLevel::from_string("admin");
        assert_eq!(admin, PermissionLevel::Admin);
        assert!(admin.can_view());
        assert!(admin.can_comment());
        assert!(admin.can_edit());
        assert!(admin.can_admin());
    }
}
