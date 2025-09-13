/**
 * REQUIREMENT: OAuth authentication services for multi-user collaboration
 * PURPOSE: Provide JWT token generation, OAuth verification, and user management
 * This enables secure authentication with Google and Facebook OAuth backends
 */
use crate::auth::models::*;
use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use reqwest::Client;
use std::env;
use uuid::Uuid;

/// JWT secret key from environment
fn get_jwt_secret() -> String {
    env::var("JWT_SECRET").unwrap_or_else(|_| "your-secret-key-change-in-production".to_string())
}

/// JWT issuer
const JWT_ISSUER: &str = "econ-graph";

/// Token expiration time (24 hours)
const TOKEN_EXPIRATION_HOURS: i64 = 24;

/// Authentication service
#[derive(Clone)]
pub struct AuthService {
    pub db_pool: DatabasePool,
    pub http_client: Client,
    pub google_client_id: String,
    pub facebook_app_id: String,
}

impl AuthService {
    /// Create new authentication service
    pub fn new(db_pool: DatabasePool) -> Self {
        let google_client_id =
            env::var("GOOGLE_CLIENT_ID").unwrap_or_else(|_| "your-google-client-id".to_string());
        let facebook_app_id =
            env::var("FACEBOOK_APP_ID").unwrap_or_else(|_| "your-facebook-app-id".to_string());

        AuthService {
            db_pool,
            http_client: Client::new(),
            google_client_id,
            facebook_app_id,
        }
    }

    /// Generate JWT token for user
    pub fn generate_token(&self, user: &User) -> AppResult<String> {
        let now = Utc::now();
        let expiration = now + Duration::hours(TOKEN_EXPIRATION_HOURS);

        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            name: user.name.clone(),
            role: user.role.clone(),
            exp: expiration.timestamp() as usize,
            iat: now.timestamp() as usize,
            iss: JWT_ISSUER.to_string(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(get_jwt_secret().as_ref()),
        )
        .map_err(|e| AppError::AuthenticationError(format!("Failed to generate token: {}", e)))?;

        Ok(token)
    }

    /// Verify JWT token and extract claims
    pub fn verify_token(&self, token: &str) -> AppResult<Claims> {
        let mut validation = Validation::default();
        validation.set_issuer(&[JWT_ISSUER]);

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(get_jwt_secret().as_ref()),
            &validation,
        )
        .map_err(|e| AppError::AuthenticationError(format!("Invalid token: {}", e)))?;

        Ok(token_data.claims)
    }

    /// Verify Google OAuth ID token
    pub async fn verify_google_token(&self, id_token: &str) -> AppResult<GoogleUserInfo> {
        // First, verify the ID token with Google's tokeninfo endpoint
        let url = format!(
            "https://oauth2.googleapis.com/tokeninfo?id_token={}",
            id_token
        );

        let response = self.http_client.get(&url).send().await.map_err(|e| {
            let error = AppError::AuthenticationError(format!(
                "Google ID token verification failed - HTTP client error: {}",
                e
            ));
            error.log_with_context("Google OAuth ID token verification");
            error
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            let error = AppError::AuthenticationError(format!(
                "Google ID token verification failed - HTTP status: {} - {}",
                status, error_text
            ));
            error.log_with_context("Google OAuth ID token verification");
            return Err(error);
        }

        let token_info: serde_json::Value = response.json().await.map_err(|e| {
            let error = AppError::AuthenticationError(format!(
                "Google ID token verification failed - JSON parsing error: {}",
                e
            ));
            error.log_with_context("Google OAuth response parsing");
            error
        })?;

        // Verify audience (client ID) - only if we have a real client ID configured
        if !self.google_client_id.starts_with("your-") {
            if let Some(audience) = token_info.get("aud") {
                if audience.as_str() != Some(&self.google_client_id) {
                    let error = AppError::AuthenticationError(
                        "Google ID token audience mismatch - token not intended for this application"
                            .to_string(),
                    );
                    error.log_with_context("Google OAuth audience verification");
                    return Err(error);
                }
            } else {
                let error = AppError::AuthenticationError(
                    "Google ID token missing audience claim".to_string(),
                );
                error.log_with_context("Google OAuth audience verification");
                return Err(error);
            }
        }

        // Extract user info from the token info response
        let user_info = GoogleUserInfo {
            id: token_info
                .get("sub")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AppError::AuthenticationError(
                        "Google ID token missing subject claim".to_string(),
                    )
                })?
                .to_string(),
            email: token_info
                .get("email")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AppError::AuthenticationError("Google ID token missing email claim".to_string())
                })?
                .to_string(),
            name: token_info
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AppError::AuthenticationError("Google ID token missing name claim".to_string())
                })?
                .to_string(),
            avatar: token_info
                .get("picture")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            verified_email: token_info
                .get("email_verified")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        };

        Ok(user_info)
    }

    /// Verify Facebook OAuth token
    pub async fn verify_facebook_token(&self, access_token: &str) -> AppResult<FacebookUserInfo> {
        // Verify the Facebook access token and get user info from Facebook Graph API
        let user_info_url = format!(
            "https://graph.facebook.com/me?access_token={}&fields=id,name,email,picture",
            access_token
        );

        let user_response = self
            .http_client
            .get(&user_info_url)
            .send()
            .await
            .map_err(|e| {
                let error = AppError::AuthenticationError(format!(
                    "Facebook token verification failed - HTTP client error: {}",
                    e
                ));
                error.log_with_context("Facebook OAuth HTTP client error");
                error
            })?;

        if !user_response.status().is_success() {
            let status = user_response.status();
            let error_text = user_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            let error = AppError::AuthenticationError(format!(
                "Facebook token verification failed - HTTP status: {} - {}",
                status, error_text
            ));
            error.log_with_context("Facebook OAuth HTTP status error");
            return Err(error);
        }

        let user_info: FacebookUserInfo = user_response.json().await.map_err(|e| {
            let error = AppError::AuthenticationError(format!(
                "Facebook token verification failed - JSON parsing error: {}",
                e
            ));
            error.log_with_context("Facebook OAuth JSON parsing error");
            error
        })?;

        Ok(user_info)
    }

    /// Create or update user from OAuth provider using actual database
    pub async fn create_or_update_oauth_user(
        &self,
        provider: AuthProvider,
        provider_id: String,
        email: String,
        name: String,
        avatar: Option<String>,
    ) -> AppResult<User> {
        let provider_str = match provider {
            AuthProvider::Google => "google",
            AuthProvider::Facebook => "facebook",
            AuthProvider::Email => "email",
        };

        // Use the actual database User model methods and convert to auth User
        let db_user = crate::models::user::User::create_or_get_oauth(
            &self.db_pool,
            provider_str.to_string(),
            provider_id,
            email,
            name,
            avatar,
        )
        .await?;

        Ok(db_user.to_auth_user())
    }

    /// Create user with email/password using actual database
    pub async fn create_email_user(
        &self,
        email: String,
        password: String,
        name: String,
    ) -> AppResult<User> {
        // Use the actual database User model method and convert to auth User
        let db_user =
            crate::models::user::User::create_with_email(&self.db_pool, email, password, name)
                .await?;
        Ok(db_user.to_auth_user())
    }

    /// Authenticate user with email/password using actual database
    pub async fn authenticate_email_user(
        &self,
        email: String,
        password: String,
    ) -> AppResult<User> {
        // Use the actual database User model method for authentication and convert to auth User
        let db_user =
            crate::models::user::User::authenticate(&self.db_pool, email, password).await?;
        Ok(db_user.to_auth_user())
    }

    /// Get user by ID using actual database lookup
    pub async fn get_user_by_id(&self, user_id: Uuid) -> AppResult<Option<User>> {
        match crate::models::user::User::get_by_id(&self.db_pool, user_id).await {
            Ok(db_user) => Ok(Some(db_user.to_auth_user())),
            Err(AppError::DatabaseError(_)) => Ok(None), // User not found
            Err(e) => Err(e),                            // Other errors
        }
    }

    /// Update user profile using actual database
    pub async fn update_user_profile(
        &self,
        user_id: Uuid,
        updates: ProfileUpdateRequest,
    ) -> AppResult<User> {
        // Convert ProfileUpdateRequest to UpdateUser for database
        let db_updates = crate::models::user::UpdateUser {
            name: updates.name,
            avatar_url: updates.avatar,
            organization: updates.organization,
            theme: updates.preferences.as_ref().map(|p| p.theme.clone()),
            default_chart_type: updates
                .preferences
                .as_ref()
                .map(|p| p.default_chart_type.clone()),
            notifications_enabled: updates.preferences.as_ref().map(|p| p.notifications),
            collaboration_enabled: updates
                .preferences
                .as_ref()
                .map(|p| p.collaboration_enabled),
            last_login_at: Some(Utc::now()),
        };

        // Use the actual database User model method and convert to auth User
        let db_user =
            crate::models::user::User::update_profile(&self.db_pool, user_id, db_updates).await?;
        Ok(db_user.to_auth_user())
    }

    /// Refresh user data
    pub async fn refresh_user(&self, user_id: Uuid) -> AppResult<User> {
        // Fetch fresh user data from database
        self.get_user_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::AuthenticationError("User not found".to_string()))
    }
}
