use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
/**
 * REQUIREMENT: User authentication models for OAuth and collaboration
 * PURPOSE: Provide user account management and authentication for chart collaboration
 * This enables secure multi-user professional economic analysis features
 */
use crate::schema::{
    annotation_comments, chart_annotations, chart_collaborators, user_sessions, users,
};

use bcrypt::{hash, verify, DEFAULT_COST};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
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
    pub provider: String,
    pub provider_id: Option<String>,
    pub password_hash: Option<String>,
    pub role: String,
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

#[derive(Debug, Serialize, Deserialize, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub provider: String,
    pub provider_id: Option<String>,
    pub password_hash: Option<String>,
    pub role: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Identifiable)]
#[diesel(table_name = user_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Insertable)]
#[diesel(table_name = user_sessions)]
pub struct NewUserSession {
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // User ID
    pub email: String,
    pub name: String,
    pub role: String,
    pub exp: usize, // Expiration time
    pub iat: usize, // Issued at
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub user: UserProfile,
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub avatar: Option<String>,
    pub provider: String,
    pub role: String,
    pub organization: Option<String>,
    pub preferences: UserPreferences,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserPreferences {
    pub theme: String,
    pub default_chart_type: String,
    pub notifications: bool,
    pub collaboration_enabled: bool,
}

impl User {
    /// Create a new user with email and password
    pub async fn create_with_email(
        pool: &DatabasePool,
        email: String,
        password: String,
        name: String,
    ) -> AppResult<User> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        // Check if user already exists
        let existing = users::table
            .filter(users::email.eq(&email))
            .first::<User>(&mut conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if existing.is_some() {
            return Err(AppError::ValidationError(
                "Email already registered".to_string(),
            ));
        }

        // Hash password
        let password_hash = hash(password, DEFAULT_COST)
            .map_err(|e| AppError::InternalError(format!("Password hashing failed: {}", e)))?;

        let new_user = NewUser {
            email,
            name,
            avatar_url: None,
            provider: "email".to_string(),
            provider_id: None,
            password_hash: Some(password_hash),
            role: "viewer".to_string(),
            organization: None,
            theme: "light".to_string(),
            default_chart_type: "line".to_string(),
            notifications_enabled: true,
            collaboration_enabled: true,
            email_verified: false,
        };

        let user = diesel::insert_into(users::table)
            .values(&new_user)
            .get_result::<User>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(user)
    }

    /// Create or get user from OAuth provider
    pub async fn create_or_get_oauth(
        pool: &DatabasePool,
        provider: String,
        provider_id: String,
        email: String,
        name: String,
        avatar_url: Option<String>,
    ) -> AppResult<User> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        // Try to find existing user by provider ID
        if let Some(user) = users::table
            .filter(users::provider.eq(&provider))
            .filter(users::provider_id.eq(&provider_id))
            .first::<User>(&mut conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
        {
            // Update last login
            let updated_user = diesel::update(users::table.find(user.id))
                .set(users::last_login_at.eq(Some(Utc::now())))
                .get_result::<User>(&mut conn)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            return Ok(updated_user);
        }

        // Try to find existing user by email
        if let Some(existing_user) = users::table
            .filter(users::email.eq(&email))
            .first::<User>(&mut conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
        {
            // Link OAuth account to existing user
            let updated_user = diesel::update(users::table.find(existing_user.id))
                .set((
                    users::provider.eq(&provider),
                    users::provider_id.eq(&provider_id),
                    users::avatar_url.eq(&avatar_url),
                    users::last_login_at.eq(Some(Utc::now())),
                ))
                .get_result::<User>(&mut conn)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            return Ok(updated_user);
        }

        // Create new user
        let new_user = NewUser {
            email,
            name,
            avatar_url,
            provider,
            provider_id: Some(provider_id),
            password_hash: None,
            role: "viewer".to_string(),
            organization: None,
            theme: "light".to_string(),
            default_chart_type: "line".to_string(),
            notifications_enabled: true,
            collaboration_enabled: true,
            email_verified: true, // OAuth accounts are considered verified
        };

        let user = diesel::insert_into(users::table)
            .values(&new_user)
            .get_result::<User>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(user)
    }

    /// Authenticate user with email and password
    pub async fn authenticate(
        pool: &DatabasePool,
        email: String,
        password: String,
    ) -> AppResult<User> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let user = users::table
            .filter(users::email.eq(&email))
            .filter(users::provider.eq("email"))
            .first::<User>(&mut conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AppError::AuthenticationError("Invalid credentials".to_string()))?;

        let password_hash = user
            .password_hash
            .as_ref()
            .ok_or_else(|| AppError::AuthenticationError("Invalid credentials".to_string()))?;

        if !verify(&password, password_hash)
            .map_err(|e| AppError::InternalError(format!("Password verification failed: {}", e)))?
        {
            return Err(AppError::AuthenticationError(
                "Invalid credentials".to_string(),
            ));
        }

        // Update last login
        let updated_user = diesel::update(users::table.find(user.id))
            .set(users::last_login_at.eq(Some(Utc::now())))
            .get_result::<User>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(updated_user)
    }

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

    /// Convert to user profile for API responses
    pub fn to_profile(&self) -> UserProfile {
        UserProfile {
            id: self.id,
            email: self.email.clone(),
            name: self.name.clone(),
            avatar: self.avatar_url.clone(),
            provider: self.provider.clone(),
            role: self.role.clone(),
            organization: self.organization.clone(),
            preferences: UserPreferences {
                theme: self.theme.clone(),
                default_chart_type: self.default_chart_type.clone(),
                notifications: self.notifications_enabled,
                collaboration_enabled: self.collaboration_enabled,
            },
            created_at: self.created_at,
            last_login_at: self.last_login_at,
        }
    }

    /// Convert database User to auth User model
    pub fn to_auth_user(&self) -> crate::auth::models::User {
        use crate::auth::models::*;

        let provider = match self.provider.as_str() {
            "google" => AuthProvider::Google,
            "facebook" => AuthProvider::Facebook,
            "email" => AuthProvider::Email,
            _ => AuthProvider::Email, // Default fallback
        };

        let role = match self.role.as_str() {
            "admin" => UserRole::Admin,
            "analyst" => UserRole::Analyst,
            "viewer" => UserRole::Viewer,
            _ => UserRole::Viewer, // Default fallback
        };

        crate::auth::models::User {
            id: self.id,
            email: self.email.clone(),
            name: self.name.clone(),
            avatar: self.avatar_url.clone(),
            provider,
            provider_id: self.provider_id.clone().unwrap_or_default(),
            role,
            organization: self.organization.clone(),
            preferences: UserPreferences {
                theme: self.theme.clone(),
                default_chart_type: self.default_chart_type.clone(),
                notifications: self.notifications_enabled,
                collaboration_enabled: self.collaboration_enabled,
            },
            created_at: self.created_at,
            last_login_at: self.last_login_at.unwrap_or_else(Utc::now),
            is_active: self.is_active,
        }
    }
}

impl UserSession {
    /// Create a new user session with JWT token
    pub async fn create(
        pool: &DatabasePool,
        user_id: Uuid,
        user_agent: Option<String>,
        ip_address: Option<String>,
        jwt_secret: &str,
    ) -> AppResult<(UserSession, String)> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        // Get user for JWT claims
        let user = User::get_by_id(pool, user_id).await?;

        // Create JWT token
        let exp = (Utc::now() + chrono::Duration::days(7)).timestamp() as usize;
        let iat = Utc::now().timestamp() as usize;

        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            name: user.name.clone(),
            role: user.role.clone(),
            exp,
            iat,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(jwt_secret.as_ref()),
        )
        .map_err(|e| AppError::InternalError(format!("JWT encoding failed: {}", e)))?;

        // Hash token for storage
        let token_hash = hash(&token, DEFAULT_COST)
            .map_err(|e| AppError::InternalError(format!("Token hashing failed: {}", e)))?;

        let new_session = NewUserSession {
            user_id,
            token_hash,
            expires_at: DateTime::from_timestamp(exp as i64, 0).ok_or_else(|| {
                AppError::InternalError("Invalid expiration timestamp".to_string())
            })?,
            user_agent,
            ip_address,
        };

        let session = diesel::insert_into(user_sessions::table)
            .values(&new_session)
            .returning(UserSession::as_select())
            .get_result::<UserSession>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok((session, token))
    }

    /// Validate JWT token and get user
    pub async fn validate_token(
        pool: &DatabasePool,
        token: &str,
        jwt_secret: &str,
    ) -> AppResult<User> {
        // Decode JWT token
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(jwt_secret.as_ref()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|e| AppError::AuthenticationError(format!("Invalid token: {}", e)))?;

        let user_id = Uuid::parse_str(&token_data.claims.sub).map_err(|e| {
            AppError::AuthenticationError(format!("Invalid user ID in token: {}", e))
        })?;

        // Get user
        let user = User::get_by_id(pool, user_id).await?;

        // Update session last used time
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let _ = diesel::update(user_sessions::table)
            .filter(user_sessions::user_id.eq(user_id))
            .filter(user_sessions::expires_at.gt(Utc::now()))
            .set(user_sessions::last_used_at.eq(Utc::now()))
            .execute(&mut conn)
            .await;

        Ok(user)
    }

    /// Logout user by invalidating sessions
    pub async fn logout(pool: &DatabasePool, user_id: Uuid) -> AppResult<()> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        diesel::delete(user_sessions::table)
            .filter(user_sessions::user_id.eq(user_id))
            .execute(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired(pool: &DatabasePool) -> AppResult<usize> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let deleted = diesel::delete(user_sessions::table)
            .filter(user_sessions::expires_at.lt(Utc::now()))
            .execute(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(deleted)
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
    pub is_visible: Option<bool>,
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
    pub is_visible: Option<bool>,
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
