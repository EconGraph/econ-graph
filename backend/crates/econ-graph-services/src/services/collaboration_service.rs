use bigdecimal::BigDecimal;
use chrono::NaiveDate;
/**
 * REQUIREMENT: Collaboration service for chart annotations and sharing
 * PURPOSE: Provide business logic for collaborative features including permissions
 * This enables secure multi-user professional economic analysis features
 */
use diesel::prelude::*;
use diesel::SelectableHelper;
use diesel_async::{AsyncConnection, RunQueryDsl};
use std::fmt;
use uuid::Uuid;

use econ_graph_core::{
    database::DatabasePool,
    enums::AnnotationVisibility,
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

/// The collaborator list `viewer` may see: all of it if `viewer` is one of them, else none.
pub fn visible_collaborators<T>(
    collaborators: Vec<(ChartCollaborator, T)>,
    viewer: Uuid,
) -> Vec<(ChartCollaborator, T)> {
    if collaborators.iter().any(|(c, _)| c.user_id == viewer) {
        collaborators
    } else {
        Vec::new()
    }
}

/// Whether `viewer` may see `annotation`: public annotations are visible to everyone,
/// private ones only to their author.
pub fn can_view_annotation(annotation: &ChartAnnotation, viewer: Option<Uuid>) -> bool {
    annotation.visibility == AnnotationVisibility::Public || viewer == Some(annotation.user_id)
}

/// Whether a user's `chart_collaborators.role` lookup grants admin on the chart.
///
/// No collaborator row (`None`) or no role grants nothing: there is no chart ownership
/// record yet, so a user with no row must not be treated as the owner.
fn role_grants_admin(role: Option<Option<String>>) -> bool {
    matches!(role, Some(Some(role)) if PermissionLevel::from_string(&role).can_admin())
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
            visibility: AnnotationVisibility::from_is_public(is_public),
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
                    chart_annotations::visibility
                        .eq(AnnotationVisibility::Public)
                        .or(chart_annotations::user_id.eq(uid)),
                )
                .order_by(chart_annotations::created_at.desc())
                .select(ChartAnnotation::as_select())
                .load::<ChartAnnotation>(&mut conn)
        } else {
            chart_annotations::table
                .filter(chart_annotations::series_id.eq(series_id))
                .filter(chart_annotations::visibility.eq(AnnotationVisibility::Public))
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
            .filter(|annotation| can_view_annotation(annotation, Some(user_id)))
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

    /// Get comments for an annotation, if `viewer` may see that annotation.
    ///
    /// An annotation hidden from `viewer` (see [`can_view_annotation`]) is reported as not
    /// found, so its comments stay as private as the annotation itself.
    pub async fn get_comments_for_annotation(
        &self,
        annotation_id: Uuid,
        viewer: Option<Uuid>,
    ) -> AppResult<Vec<AnnotationComment>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let annotation = chart_annotations::table
            .filter(chart_annotations::id.eq(annotation_id))
            .select(ChartAnnotation::as_select())
            .first::<ChartAnnotation>(&mut conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .filter(|annotation| can_view_annotation(annotation, viewer))
            .ok_or_else(|| AppError::NotFound("Annotation not found".to_string()))?;

        let comments = annotation_comments::table
            .filter(annotation_comments::annotation_id.eq(annotation.id))
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

    /// Get a chart's collaborators, as seen by `viewer`.
    ///
    /// Only the chart's own collaborators may see who else is on it; anyone else gets an
    /// empty list, which also avoids revealing whether the chart exists.
    pub async fn get_collaborators(
        &self,
        chart_id: Uuid,
        viewer: Uuid,
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

        Ok(visible_collaborators(collaborators, viewer))
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

        Ok(role_grants_admin(permission))
    }

    /// Update an annotation's own fields (only by its author).
    ///
    /// A private annotation the caller cannot see is reported as not found, as in
    /// [`Self::delete_annotation`]; a caller who can see it but isn't its author gets
    /// `Unauthorized` rather than a silent no-op, since the field arguments are `Some` only
    /// when the caller actually asked to change them.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_annotation(
        &self,
        annotation_id: Uuid,
        user_id: Uuid,
        title: Option<String>,
        content: Option<String>,
        color: Option<String>,
        annotation_type: Option<String>,
        is_public: Option<bool>,
    ) -> AppResult<ChartAnnotation> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        let annotation = chart_annotations::table
            .filter(chart_annotations::id.eq(annotation_id))
            .select(ChartAnnotation::as_select())
            .first::<ChartAnnotation>(&mut conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Annotation not found".to_string()))?;

        if !can_view_annotation(&annotation, Some(user_id)) {
            return Err(AppError::NotFound("Annotation not found".to_string()));
        }

        if annotation.user_id != user_id {
            return Err(AppError::Unauthorized("Unauthorized".to_string()));
        }

        let updated = diesel::update(
            chart_annotations::table.filter(chart_annotations::id.eq(annotation_id)),
        )
        .set((
            title.map(|t| chart_annotations::title.eq(t)),
            content.map(|c| chart_annotations::description.eq(Some(c))),
            color.map(|c| chart_annotations::color.eq(Some(c))),
            annotation_type.map(|t| chart_annotations::annotation_type.eq(Some(t))),
            is_public
                .map(|p| chart_annotations::visibility.eq(AnnotationVisibility::from_is_public(p))),
            chart_annotations::updated_at.eq(chrono::Utc::now()),
        ))
        .returning(ChartAnnotation::as_select())
        .get_result::<ChartAnnotation>(&mut conn)
        .await
        .map_err(|e| match e {
            // The annotation was deleted between the lookup above and this update.
            diesel::result::Error::NotFound => {
                AppError::NotFound("Annotation not found".to_string())
            }
            e => AppError::DatabaseError(e.to_string()),
        })?;

        Ok(updated)
    }

    /// Delete an annotation (only by owner or admin)
    pub async fn delete_annotation(&self, annotation_id: Uuid, user_id: Uuid) -> AppResult<bool> {
        let mut conn = self.pool.get().await.map_err(|e| {
            econ_graph_core::error::AppError::DatabaseError(format!(
                "Failed to get database connection: {}",
                e
            ))
        })?;

        // Keep the annotation and any admin grant stable through authorization and deletion.
        conn.transaction::<bool, AppError, _>(async move |conn| {
            // Get the annotation to check ownership
            let annotation = chart_annotations::table
                .filter(chart_annotations::id.eq(annotation_id))
                .select(ChartAnnotation::as_select())
                .for_update()
                .first::<ChartAnnotation>(conn)
                .await
                .optional()
                .map_err(|e| AppError::DatabaseError(e.to_string()))?
                .ok_or_else(|| AppError::NotFound("Annotation not found".to_string()))?;

            // A private annotation the caller cannot see is reported as not found, as in
            // add_comment and get_comments_for_annotation, even to a chart admin: its
            // existence stays hidden and nobody but its author can delete it.
            if !can_view_annotation(&annotation, Some(user_id)) {
                return Err(AppError::NotFound("Annotation not found".to_string()));
            }

            // Check if user owns the annotation or has admin permission
            if annotation.user_id != user_id {
                // If not owner, check admin permission
                if let Some(chart_id) = annotation.chart_id {
                    let role = chart_collaborators::table
                        .filter(chart_collaborators::chart_id.eq(chart_id))
                        .filter(chart_collaborators::user_id.eq(user_id))
                        .select(chart_collaborators::role)
                        .for_share()
                        .first::<Option<String>>(conn)
                        .await
                        .optional()
                        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
                    if !role_grants_admin(role) {
                        return Err(AppError::Unauthorized("Unauthorized".to_string()));
                    }
                } else {
                    return Err(AppError::Unauthorized("Unauthorized".to_string()));
                }
            }

            // The foreign key cascades comments in the same statement. If deletion fails,
            // PostgreSQL preserves both the annotation and its comments.
            let deleted = diesel::delete(
                chart_annotations::table.filter(chart_annotations::id.eq(annotation_id)),
            )
            .execute(conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            Ok(deleted > 0)
        })
        .await
        .map_err(|e| match e {
            AppError::Database(e) => AppError::DatabaseError(e.to_string()),
            e => e,
        })
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

    #[tokio::test]
    #[serial]
    async fn test_delete_annotation_cascades_and_preserves_comments_on_failure() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let service = CollaborationService::new(container.pool().clone());
        let mut conn = container.pool().get().await.unwrap();
        let owner = Uuid::new_v4();
        let outsider = Uuid::new_v4();
        let annotation_id = Uuid::new_v4();
        diesel::sql_query("INSERT INTO users (id, email, name) VALUES ($1, 'delete-test@example.com', 'Owner')")
            .bind::<diesel::sql_types::Uuid, _>(owner)
            .execute(&mut conn).await.unwrap();
        diesel::sql_query("INSERT INTO chart_annotations (id, user_id, annotation_date, title, visibility) VALUES ($1, $2, CURRENT_DATE, 'Delete test', 'public')")
            .bind::<diesel::sql_types::Uuid, _>(annotation_id)
            .bind::<diesel::sql_types::Uuid, _>(owner)
            .execute(&mut conn).await.unwrap();
        diesel::insert_into(annotation_comments::table)
            .values(NewAnnotationComment { annotation_id, user_id: owner, content: "Keep until deletion succeeds".into() })
            .execute(&mut conn).await.unwrap();

        assert!(matches!(service.delete_annotation(annotation_id, outsider).await,
            Err(AppError::Unauthorized(_))));
        diesel::update(chart_annotations::table.find(annotation_id))
            .set(chart_annotations::visibility.eq(AnnotationVisibility::Private))
            .execute(&mut conn).await.unwrap();
        assert!(matches!(service.delete_annotation(annotation_id, outsider).await,
            Err(AppError::NotFound(_))));

        // Fail the parent DELETE after authorization, reproducing the partial-delete risk.
        diesel::sql_query("CREATE FUNCTION reject_annotation_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'forced delete failure'; END $$")
            .execute(&mut conn).await.unwrap();
        diesel::sql_query("CREATE TRIGGER reject_annotation_delete BEFORE DELETE ON chart_annotations FOR EACH ROW EXECUTE FUNCTION reject_annotation_delete()")
            .execute(&mut conn).await.unwrap();
        assert!(matches!(service.delete_annotation(annotation_id, owner).await,
            Err(AppError::DatabaseError(_))));
        assert_eq!(chart_annotations::table.find(annotation_id).count()
            .get_result::<i64>(&mut conn).await.unwrap(), 1);
        assert_eq!(annotation_comments::table.filter(annotation_comments::annotation_id.eq(annotation_id)).count()
            .get_result::<i64>(&mut conn).await.unwrap(), 1);

        diesel::sql_query("DROP TRIGGER reject_annotation_delete ON chart_annotations")
            .execute(&mut conn).await.unwrap();
        assert!(service.delete_annotation(annotation_id, owner).await.unwrap());
        assert_eq!(chart_annotations::table.find(annotation_id).count()
            .get_result::<i64>(&mut conn).await.unwrap(), 0);
        assert_eq!(annotation_comments::table.filter(annotation_comments::annotation_id.eq(annotation_id)).count()
            .get_result::<i64>(&mut conn).await.unwrap(), 0);
        assert!(matches!(service.delete_annotation(annotation_id, owner).await,
            Err(AppError::NotFound(_))));
    }

    fn annotation(owner: Uuid, visibility: AnnotationVisibility) -> ChartAnnotation {
        ChartAnnotation {
            id: Uuid::new_v4(),
            user_id: owner,
            series_id: Some("GDP".to_string()),
            chart_id: None,
            annotation_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            annotation_value: None,
            title: "t".to_string(),
            description: None,
            color: None,
            annotation_type: None,
            visibility,
            is_pinned: None,
            tags: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn collaborator(chart_id: Uuid, user_id: Uuid) -> (ChartCollaborator, ()) {
        let collaborator = ChartCollaborator {
            id: Uuid::new_v4(),
            chart_id,
            user_id,
            invited_by: None,
            role: Some("view".to_string()),
            permissions: None,
            created_at: None,
            last_accessed_at: None,
        };
        (collaborator, ())
    }

    /// A chart's collaborators see the whole list; anyone else sees nothing.
    #[test]
    fn test_visible_collaborators_only_for_collaborators() {
        let chart = Uuid::new_v4();
        let (alice, bob, stranger) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let list = || vec![collaborator(chart, alice), collaborator(chart, bob)];

        assert_eq!(visible_collaborators(list(), alice).len(), 2);
        assert_eq!(visible_collaborators(list(), bob).len(), 2);
        assert!(visible_collaborators(list(), stranger).is_empty());
        assert!(visible_collaborators(Vec::<(ChartCollaborator, ())>::new(), alice).is_empty());
    }

    /// Public annotations are visible to anyone; private ones only to their author.
    #[test]
    fn test_can_view_annotation() {
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();

        let public = annotation(owner, AnnotationVisibility::Public);
        assert!(can_view_annotation(&public, None));
        assert!(can_view_annotation(&public, Some(other)));

        let private = annotation(owner, AnnotationVisibility::Private);
        assert!(can_view_annotation(&private, Some(owner)));
        assert!(!can_view_annotation(&private, Some(other)));
        assert!(!can_view_annotation(&private, None));
    }

    /// Only an explicit admin role grants admin; a missing row or role grants nothing.
    #[test]
    fn test_role_grants_admin_fails_closed() {
        assert!(role_grants_admin(Some(Some("admin".to_string()))));
        assert!(!role_grants_admin(Some(Some("edit".to_string()))));
        assert!(!role_grants_admin(Some(None)));
        assert!(!role_grants_admin(None));
    }
}
