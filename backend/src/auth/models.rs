/**
 * REQUIREMENT: OAuth authentication models for multi-user collaboration
 * PURPOSE: Define user authentication data structures and JWT tokens
 * This enables secure user management with OAuth providers
 */
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// User model for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub avatar: Option<String>,
    pub provider: AuthProvider,
    pub provider_id: String,
    pub role: UserRole,
    pub organization: Option<String>,
    pub preferences: UserPreferences,
    pub created_at: DateTime<Utc>,
    pub last_login_at: DateTime<Utc>,
    pub is_active: bool,
}

/// Authentication provider types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthProvider {
    Google,
    Facebook,
    Email,
}

/// User role for authorization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Analyst,
    #[default]
    Viewer,
}

/// User preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub theme: String,
    pub default_chart_type: String,
    pub notifications: bool,
    pub collaboration_enabled: bool,
}

impl Default for UserPreferences {
    fn default() -> Self {
        UserPreferences {
            theme: "light".to_string(),
            default_chart_type: "line".to_string(),
            notifications: true,
            collaboration_enabled: true,
        }
    }
}

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub email: String,
    pub name: String,
    pub role: UserRole,
    pub exp: usize,  // Expiration time
    pub iat: usize,  // Issued at
    pub iss: String, // Issuer
}

/// Google OAuth user info
#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub name: String,
    #[serde(rename = "picture")]
    pub avatar: Option<String>,
    pub verified_email: bool,
}

/// Facebook OAuth user info
#[derive(Debug, Deserialize)]
pub struct FacebookUserInfo {
    pub id: String,
    pub email: Option<String>,
    pub name: String,
    pub picture: Option<FacebookPicture>,
}

#[derive(Debug, Deserialize)]
pub struct FacebookPicture {
    pub data: FacebookPictureData,
}

#[derive(Debug, Deserialize)]
pub struct FacebookPictureData {
    pub url: String,
}

/// Login request
#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}

/// Registration request
#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
    #[validate(length(min = 2))]
    pub name: String,
}

/// Google OAuth request
#[derive(Debug, Deserialize)]
pub struct GoogleAuthRequest {
    pub token: String,
    pub user_info: GoogleUserInfo,
}

/// Facebook OAuth request
#[derive(Debug, Deserialize)]
pub struct FacebookAuthRequest {
    pub token: String,
}

/// Authentication response
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}

/// User response (without sensitive data)
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub avatar: Option<String>,
    pub provider: AuthProvider,
    pub role: UserRole,
    pub organization: Option<String>,
    pub preferences: UserPreferences,
    pub created_at: String,
    pub last_login_at: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        UserResponse {
            id: user.id.to_string(),
            email: user.email,
            name: user.name,
            avatar: user.avatar,
            provider: user.provider,
            role: user.role,
            organization: user.organization,
            preferences: user.preferences,
            created_at: user.created_at.to_rfc3339(),
            last_login_at: user.last_login_at.to_rfc3339(),
        }
    }
}

/// Profile update request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ProfileUpdateRequest {
    #[validate(length(min = 2))]
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub organization: Option<String>,
    pub preferences: Option<UserPreferences>,
}

/// Password hash for database storage
#[derive(Debug)]
pub struct PasswordHash {
    pub hash: String,
}

impl PasswordHash {
    pub fn new(password: &str) -> Result<Self, bcrypt::BcryptError> {
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
        Ok(PasswordHash { hash })
    }

    pub fn verify(&self, password: &str) -> Result<bool, bcrypt::BcryptError> {
        bcrypt::verify(password, &self.hash)
    }
}
