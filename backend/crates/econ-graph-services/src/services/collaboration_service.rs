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

        conn.transaction::<ChartCollaborator, AppError, _>(async move |conn| {
            // Lock the caller's grant until the write commits. A concurrent revocation
            // either commits first (and is observed here), or waits for this share.
            // Lock both existing rows in a stable order to avoid reciprocal-share deadlocks.
            let grants = chart_collaborators::table
                .filter(chart_collaborators::chart_id.eq(chart_id))
                .filter(chart_collaborators::user_id.eq_any([owner_user_id, target_user_id]))
                .order(chart_collaborators::user_id.asc())
                .for_update()
                .select(ChartCollaborator::as_select())
                .load::<ChartCollaborator>(conn)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            let role = grants
                .iter()
                .find(|grant| grant.user_id == owner_user_id)
                .map(|grant| grant.role.clone());
            if !role_grants_admin(role) {
                return Err(AppError::Unauthorized("Unauthorized".to_string()));
            }

            let role = permission_level.to_string();
            let new_collaborator = NewChartCollaborator {
                chart_id,
                user_id: target_user_id,
                invited_by: Some(owner_user_id),
                role: Some(role.clone()),
                permissions: None,
            };

            diesel::insert_into(chart_collaborators::table)
                .values(&new_collaborator)
                .on_conflict((chart_collaborators::chart_id, chart_collaborators::user_id))
                .do_update()
                .set(chart_collaborators::role.eq(role))
                .returning(ChartCollaborator::as_select())
                .get_result::<ChartCollaborator>(conn)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))
        })
        .await
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

    // Give every test a private schema on DATABASE_URL, including every pooled
    // connection used by the service. Only the collaboration schema is needed.
    async fn sharing_fixture() -> (DatabasePool, Uuid, Uuid, Uuid) {
        use diesel_async::SimpleAsyncConnection;
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must point to the test PostgreSQL database");
        let base_pool = econ_graph_core::database::create_pool(&database_url)
            .await
            .unwrap();
        let schema = format!("sharing_{}", Uuid::new_v4().simple());
        let mut base_conn = base_pool.get().await.unwrap();
        base_conn
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        let separator = if database_url.contains('?') { '&' } else { '?' };
        let pool = econ_graph_core::database::create_pool(&format!(
            "{database_url}{separator}options=-csearch_path%3D{schema}"
        ))
        .await
        .unwrap();
        let mut conn = pool.get().await.unwrap();
        conn.batch_execute(
            "CREATE TABLE chart_collaborators (
            id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
            chart_id uuid NOT NULL, user_id uuid NOT NULL, invited_by uuid,
            role varchar(20), permissions jsonb, created_at timestamptz DEFAULT now(),
            last_accessed_at timestamptz);",
        )
        .await
        .unwrap();
        let (chart, admin, target) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        diesel::insert_into(chart_collaborators::table)
            .values(NewChartCollaborator {
                chart_id: chart,
                user_id: admin,
                invited_by: None,
                role: Some("admin".into()),
                permissions: None,
            })
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        (pool, chart, admin, target)
    }

    async fn migrate_sharing(pool: &DatabasePool) {
        use diesel_async::SimpleAsyncConnection;
        let mut conn = pool.get().await.unwrap();
        conn.transaction::<(), diesel::result::Error, _>(async |conn| {
            conn.batch_execute(include_str!(
                "../../../../migrations/2026-09-30-000100_unique_chart_collaborators/up.sql"
            ))
            .await
        })
        .await
        .unwrap();
    }

    async fn cleanup_sharing(pool: &DatabasePool) {
        use diesel_async::SimpleAsyncConnection;
        let mut conn = pool.get().await.unwrap();
        let schema: String = diesel::select(diesel::dsl::sql::<diesel::sql_types::Text>(
            "current_schema()",
        ))
        .get_result(&mut conn)
        .await
        .unwrap();
        assert!(schema.starts_with("sharing_"));
        conn.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn concurrent_shares_keep_one_grant_and_identity() {
        let (pool, chart, admin, target) = sharing_fixture().await;
        migrate_sharing(&pool).await;
        let second_admin = Uuid::new_v4();
        let mut conn = pool.get().await.unwrap();
        diesel::insert_into(chart_collaborators::table)
            .values(NewChartCollaborator {
                chart_id: chart,
                user_id: second_admin,
                invited_by: None,
                role: Some("admin".into()),
                permissions: None,
            })
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        let service = CollaborationService::new(pool.clone());
        let (first, second) = tokio::join!(
            service.share_chart(chart, admin, target, PermissionLevel::View),
            service.share_chart(chart, second_admin, target, PermissionLevel::Edit)
        );
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(first.id, second.id);
        let mut conn = pool.get().await.unwrap();
        let grants = chart_collaborators::table
            .filter(chart_collaborators::chart_id.eq(chart))
            .filter(chart_collaborators::user_id.eq(target))
            .load::<ChartCollaborator>(&mut conn)
            .await
            .unwrap();
        assert_eq!(grants.len(), 1);
        assert!(matches!(grants[0].role.as_deref(), Some("view" | "edit")));
        drop(conn);
        cleanup_sharing(&pool).await;
    }

    #[tokio::test]
    #[serial]
    async fn share_waits_for_revocation_and_rechecks_locked_grant() {
        let (pool, chart, admin, target) = sharing_fixture().await;
        migrate_sharing(&pool).await;
        let service = CollaborationService::new(pool.clone());
        let mut revoker = pool.get().await.unwrap();
        use diesel_async::SimpleAsyncConnection;
        revoker.batch_execute("BEGIN").await.unwrap();
        diesel::update(chart_collaborators::table.filter(chart_collaborators::user_id.eq(admin)))
            .set(chart_collaborators::role.eq("view"))
            .execute(&mut revoker)
            .await
            .unwrap();
        let mut share = tokio::spawn(async move {
            service
                .share_chart(chart, admin, target, PermissionLevel::Admin)
                .await
        });
        // Revocation owns the grant lock before the share begins. Keep it open
        // while the competing request reaches its locking read.
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), &mut share)
                .await
                .is_err(),
            "sharing must wait for the locked grant"
        );
        revoker.batch_execute("COMMIT").await.unwrap();
        assert!(matches!(
            share.await.unwrap(),
            Err(AppError::Unauthorized(_))
        ));
        let count: i64 = chart_collaborators::table
            .filter(chart_collaborators::user_id.eq(target))
            .count()
            .get_result(&mut revoker)
            .await
            .unwrap();
        assert_eq!(count, 0);
        drop(revoker);
        cleanup_sharing(&pool).await;
    }

    #[tokio::test]
    #[serial]
    async fn migration_reconciles_duplicates_without_promoting_grants() {
        let (pool, chart, admin, _) = sharing_fixture().await;
        let mut conn = pool.get().await.unwrap();
        let original: ChartCollaborator =
            chart_collaborators::table.first(&mut conn).await.unwrap();
        diesel::update(chart_collaborators::table.find(original.id))
            .set(chart_collaborators::created_at.eq(chrono::Utc::now() - chrono::Duration::days(1)))
            .execute(&mut conn)
            .await
            .unwrap();
        for role in [Some("edit"), None, Some("unknown")] {
            diesel::insert_into(chart_collaborators::table)
                .values(NewChartCollaborator {
                    chart_id: chart,
                    user_id: admin,
                    invited_by: None,
                    role: role.map(str::to_owned),
                    permissions: Some(serde_json::json!({"edit": true})),
                })
                .execute(&mut conn)
                .await
                .unwrap();
        }
        drop(conn);
        migrate_sharing(&pool).await;
        let mut conn = pool.get().await.unwrap();
        let grants = chart_collaborators::table
            .load::<ChartCollaborator>(&mut conn)
            .await
            .unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].id, original.id);
        assert_eq!(grants[0].role.as_deref(), Some("view"));
        assert_eq!(grants[0].permissions, None);
        let duplicate = diesel::insert_into(chart_collaborators::table)
            .values(NewChartCollaborator {
                chart_id: chart,
                user_id: admin,
                invited_by: None,
                role: Some("admin".into()),
                permissions: None,
            })
            .execute(&mut conn)
            .await;
        assert!(matches!(
            duplicate,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _
            ))
        ));
        drop(conn);
        cleanup_sharing(&pool).await;
    }

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
}
