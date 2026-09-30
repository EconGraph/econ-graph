use crate::database::DatabasePool;
use crate::enums::AnnotationVisibility;
use crate::error::{AppError, AppResult};
/**
 * User account models for chart collaboration.
 * Sign-in itself is handled by Keycloak; the backend only verifies its tokens (see
 * `econ_graph_auth::oidc`) and keeps a `users` row per subject for ownership and preferences.
 */
use crate::schema::{annotation_comments, chart_annotations, chart_collaborators, users};

use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use serde_json;
use uuid::Uuid;
// IP addresses stored as strings for Diesel compatibility

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Identifiable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub organization: Option<String>,
    pub theme: String,
    pub default_chart_type: String,
    pub notifications_enabled: bool,
    pub collaboration_enabled: bool,
    pub is_active: bool,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

/// A new `users` row.
#[derive(Debug, Serialize, Deserialize, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub organization: Option<String>,
    pub theme: String,
    pub default_chart_type: String,
    pub notifications_enabled: bool,
    pub collaboration_enabled: bool,
    pub email_verified: bool,
}

#[derive(Debug, Serialize, Deserialize, AsChangeset)]
#[diesel(table_name = users)]
pub struct UpdateUser {
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub organization: Option<String>,
    pub theme: Option<String>,
    pub default_chart_type: Option<String>,
    pub notifications_enabled: Option<bool>,
    pub collaboration_enabled: Option<bool>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl User {
    /// Update user profile
    pub async fn update_profile(
        pool: &DatabasePool,
        user_id: Uuid,
        updates: UpdateUser,
    ) -> AppResult<User> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let user = diesel::update(users::table.find(user_id))
            .set(&updates)
            .get_result::<User>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(user)
    }

    /// Get user by ID
    pub async fn get_by_id(pool: &DatabasePool, user_id: Uuid) -> AppResult<User> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(user)
    }

    /// The `users` row for identity-provider subject `id`, created on first sight.
    ///
    /// Users sign in through the identity provider, so the API first meets them on their first
    /// request. The row takes the token's email when it is verified, fits the column and no
    /// other row uses it; otherwise an unguessable placeholder `<id>.<random>@users.invalid`,
    /// because `users.email` is required and unique. An unverified address is never stored. A
    /// row that already exists is returned unchanged, whatever the token now says.
    /// Concurrent first requests are safe: the insert does nothing if the row appeared.
    pub async fn get_or_create_for_subject(
        pool: &DatabasePool,
        id: Uuid,
        email: Option<&str>,
        email_verified: bool,
        name: Option<&str>,
    ) -> AppResult<User> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;
        async fn find(
            conn: &mut diesel_async::AsyncPgConnection,
            id: Uuid,
        ) -> AppResult<Option<User>> {
            users::table
                .find(id)
                .select(User::as_select())
                .first::<User>(conn)
                .await
                .optional()
                .map_err(|e| AppError::DatabaseError(e.to_string()))
        }

        if let Some(user) = find(&mut conn, id).await? {
            return Ok(user);
        }

        let placeholder = format!("{id}.{}@users.invalid", Uuid::new_v4().simple());
        let name = name.unwrap_or("New user");
        let email = email.filter(|email| email_verified && email.len() <= 255);
        let mut candidates = vec![(placeholder.as_str(), false)];
        if let Some(email) = email {
            candidates.insert(0, (email, true));
        }
        for (email, verified) in candidates {
            diesel::insert_into(users::table)
                .values((
                    users::id.eq(id),
                    users::email.eq(email),
                    users::name.eq(truncate_chars(name, 255)),
                    users::email_verified.eq(verified),
                ))
                .on_conflict_do_nothing()
                .execute(&mut conn)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            if let Some(user) = find(&mut conn, id).await? {
                return Ok(user);
            }
        }
        Err(AppError::DatabaseError(format!(
            "could not create a users row for subject {id}"
        )))
    }
}

/// Chart annotation model for collaborative features
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Identifiable)]
#[diesel(table_name = chart_annotations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChartAnnotation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub series_id: Option<String>,
    pub chart_id: Option<Uuid>,
    pub annotation_date: NaiveDate,
    pub annotation_value: Option<BigDecimal>,
    pub title: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub annotation_type: Option<String>,
    pub visibility: AnnotationVisibility,
    pub is_pinned: Option<bool>,
    pub tags: Option<Vec<Option<String>>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// New chart annotation for insertion
#[derive(Debug, Serialize, Deserialize, Insertable)]
#[diesel(table_name = chart_annotations)]
pub struct NewChartAnnotation {
    pub user_id: Uuid,
    pub series_id: Option<String>,
    pub chart_id: Option<Uuid>,
    pub annotation_date: NaiveDate,
    pub annotation_value: Option<BigDecimal>,
    pub title: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub annotation_type: Option<String>,
    pub visibility: AnnotationVisibility,
    pub is_pinned: Option<bool>,
    pub tags: Option<Vec<Option<String>>>,
}

/// Annotation comment model for discussion threads
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Identifiable)]
#[diesel(table_name = annotation_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AnnotationComment {
    pub id: Uuid,
    pub annotation_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub is_resolved: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// New annotation comment for insertion
#[derive(Debug, Serialize, Deserialize, Insertable)]
#[diesel(table_name = annotation_comments)]
pub struct NewAnnotationComment {
    pub annotation_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
}

/// Chart collaborator model for sharing permissions
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Identifiable)]
#[diesel(table_name = chart_collaborators)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChartCollaborator {
    pub id: Uuid,
    pub chart_id: Uuid, // Could be series_id or a chart collection
    pub user_id: Uuid,
    pub invited_by: Option<Uuid>,
    pub role: Option<String>, // "view", "comment", "edit", "admin"
    pub permissions: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub last_accessed_at: Option<DateTime<Utc>>,
}

/// New chart collaborator for insertion
#[derive(Debug, Serialize, Deserialize, Insertable)]
#[diesel(table_name = chart_collaborators)]
pub struct NewChartCollaborator {
    pub chart_id: Uuid,
    pub user_id: Uuid,
    pub invited_by: Option<Uuid>,
    pub role: Option<String>,
    pub permissions: Option<serde_json::Value>,
}

/// The first `max` characters of `s`.
fn truncate_chars(s: &str, max: usize) -> &str {
    s.char_indices().nth(max).map_or(s, |(end, _)| &s[..end])
}
