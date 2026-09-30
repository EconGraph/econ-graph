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

        // A read-committed transaction alone is insufficient: hold shared row locks
        // through the insert so visibility edits and role revocations cannot interleave.
        // All queries use this connection, including the chart permission lookup.
        conn.transaction::<AppResult<AnnotationComment>, diesel::result::Error, _>(
            async move |conn| {
                let annotation = chart_annotations::table
                    .filter(chart_annotations::id.eq(annotation_id))
                    .select(ChartAnnotation::as_select())
                    .for_share()
                    .first::<ChartAnnotation>(conn)
                    .await
                    .optional()?
                    .filter(|annotation| can_view_annotation(annotation, Some(user_id)));
                let Some(annotation) = annotation else {
                    return Ok(Err(AppError::NotFound("Annotation not found".to_string())));
                };

                // Series annotations retain the existing authenticated-user policy.
                // Chart annotations require an explicit admin role, even for the author.
                if annotation.series_id.is_none() {
                    let Some(chart_id) = annotation.chart_id else {
                        return Ok(Err(AppError::Unauthorized("Unauthorized".to_string())));
                    };
                    let role = chart_collaborators::table
                        .filter(chart_collaborators::chart_id.eq(chart_id))
                        .filter(chart_collaborators::user_id.eq(user_id))
                        .select(chart_collaborators::role)
                        .for_share()
                        .first::<Option<String>>(conn)
                        .await
                        .optional()?;
                    if !role_grants_admin(role) {
                        return Ok(Err(AppError::Unauthorized("Unauthorized".to_string())));
                    }
                }

                let new_comment = NewAnnotationComment {
                    annotation_id,
                    user_id,
                    content,
                };
                let comment = diesel::insert_into(annotation_comments::table)
                    .values(&new_comment)
                    .returning(AnnotationComment::as_select())
                    .get_result::<AnnotationComment>(conn)
                    .await?;
                Ok(Ok(comment))
            },
        )
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
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

        // The left join includes visible annotations without comments. Filtering
        // visibility in the same statement as the comment read removes the check/use
        // race while retaining NotFound for both private and missing annotations.
        let rows = chart_annotations::table
            .left_join(annotation_comments::table)
            .filter(chart_annotations::id.eq(annotation_id))
            .filter(
                chart_annotations::visibility
                    .eq(AnnotationVisibility::Public)
                    .or(chart_annotations::user_id.nullable().eq(viewer)),
            )
            .order_by(annotation_comments::created_at.asc())
            .select((
                chart_annotations::id,
                Option::<AnnotationComment>::as_select(),
            ))
            .load::<(Uuid, Option<AnnotationComment>)>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        if rows.is_empty() {
            return Err(AppError::NotFound("Annotation not found".to_string()));
        }
        Ok(rows
            .into_iter()
            .filter_map(|(_, comment)| comment)
            .collect())
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

        // Get the annotation to check ownership
        let annotation = chart_annotations::table
            .filter(chart_annotations::id.eq(annotation_id))
            .select(ChartAnnotation::as_select())
            .first::<ChartAnnotation>(&mut conn)
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

    // Run against the same migrated external Postgres database as TestContainer.
    // A one-connection pool catches accidental nested permission pool acquisition.
    #[tokio::test]
    #[serial]
    async fn test_atomic_comment_privacy_and_revocation() {
        use diesel_async::pooled_connection::AsyncDieselConnectionManager;
        use std::time::Duration;

        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/econ_graph_test".to_string());
        let single = DatabasePool::builder()
            .max_size(1)
            .connection_timeout(Duration::from_secs(1))
            .build(AsyncDieselConnectionManager::new(url))
            .await
            .unwrap();
        let service = CollaborationService::new(single.clone());
        let owner = Uuid::new_v4();
        let admin = Uuid::new_v4();
        let chart = Uuid::new_v4();
        let mut conn = container.pool().get().await.unwrap();
        for id in [owner, admin] {
            diesel::insert_into(users::table)
                .values((
                    users::id.eq(id),
                    users::email.eq(format!("{id}@test.invalid")),
                    users::name.eq("test"),
                ))
                .execute(&mut conn)
                .await
                .unwrap();
        }
        let mut record = annotation(owner, AnnotationVisibility::Public);
        record.series_id = None;
        record.chart_id = Some(chart);
        let new = NewChartAnnotation {
            user_id: owner,
            chart_id: Some(chart),
            series_id: None,
            annotation_date: record.annotation_date,
            annotation_value: None,
            title: "test".into(),
            description: None,
            color: None,
            annotation_type: None,
            visibility: record.visibility,
            is_pinned: None,
            tags: None,
        };
        record = diesel::insert_into(chart_annotations::table)
            .values(&new)
            .returning(ChartAnnotation::as_select())
            .get_result(&mut conn)
            .await
            .unwrap();
        diesel::insert_into(chart_collaborators::table)
            .values(&NewChartCollaborator {
                chart_id: chart,
                user_id: admin,
                invited_by: None,
                role: Some("ADMIN".into()),
                permissions: None,
            })
            .execute(&mut conn)
            .await
            .unwrap();
        assert!(service
            .get_comments_for_annotation(record.id, None)
            .await
            .unwrap()
            .is_empty());
        assert!(matches!(
            service.add_comment(owner, record.id, "denied".into()).await,
            Err(AppError::Unauthorized(_))
        ));
        service
            .add_comment(admin, record.id, "visible".into())
            .await
            .unwrap();
        assert_eq!(
            service
                .get_comments_for_annotation(record.id, None)
                .await
                .unwrap()
                .len(),
            1
        );

        // The comment attempt waits for a visibility edit's row lock; after the
        // edit commits, SELECT FOR SHARE sees the new private value and denies.
        let attempt = conn
            .transaction::<_, diesel::result::Error, _>(async |conn| {
                diesel::update(chart_annotations::table.find(record.id))
                    .set(chart_annotations::visibility.eq(AnnotationVisibility::Private))
                    .execute(conn)
                    .await?;
                let blocked_service = CollaborationService::new(single.clone());
                let annotation_id = record.id;
                let mut attempt = tokio::spawn(async move {
                    blocked_service
                        .add_comment(admin, annotation_id, "private".into())
                        .await
                });
                assert!(
                    tokio::time::timeout(Duration::from_millis(100), &mut attempt)
                        .await
                        .is_err()
                );
                Ok(attempt)
            })
            .await
            .unwrap();
        assert!(matches!(attempt.await.unwrap(), Err(AppError::NotFound(_))));
        assert!(matches!(
            service
                .get_comments_for_annotation(record.id, Some(admin))
                .await,
            Err(AppError::NotFound(_))
        ));
        assert_eq!(
            service
                .get_comments_for_annotation(record.id, Some(owner))
                .await
                .unwrap()
                .len(),
            1
        );
        diesel::update(chart_annotations::table.find(record.id))
            .set(chart_annotations::visibility.eq(AnnotationVisibility::Public))
            .execute(&mut conn)
            .await
            .unwrap();

        let attempt = conn
            .transaction::<_, diesel::result::Error, _>(async |conn| {
                diesel::update(
                    chart_collaborators::table
                        .filter(chart_collaborators::chart_id.eq(chart))
                        .filter(chart_collaborators::user_id.eq(admin)),
                )
                .set(chart_collaborators::role.eq("view"))
                .execute(conn)
                .await?;
                let blocked_service = CollaborationService::new(single.clone());
                let annotation_id = record.id;
                let mut attempt = tokio::spawn(async move {
                    blocked_service
                        .add_comment(admin, annotation_id, "revoked".into())
                        .await
                });
                assert!(
                    tokio::time::timeout(Duration::from_millis(100), &mut attempt)
                        .await
                        .is_err()
                );
                Ok(attempt)
            })
            .await
            .unwrap();
        assert!(matches!(
            attempt.await.unwrap(),
            Err(AppError::Unauthorized(_))
        ));
        assert_eq!(
            service
                .get_comments_for_annotation(record.id, None)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(matches!(
            service
                .get_comments_for_annotation(Uuid::new_v4(), None)
                .await,
            Err(AppError::NotFound(_))
        ));
    }
}
