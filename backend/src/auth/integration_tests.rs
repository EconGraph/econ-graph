/**
 * REQUIREMENT: Integration tests for Google authentication and OAuth functionality
 * PURPOSE: Test complete authentication flows including Google OAuth, Facebook OAuth, and email auth
 * This ensures authentication works end-to-end and catches bugs before they reach users
 */

#[cfg(test)]
mod tests {
    use crate::auth::models::*;
    use crate::auth::services::AuthService;
    use crate::test_utils::TestContainer;
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;
    use warp::test;

    /// Test authentication service creation
    #[tokio::test]
    async fn test_auth_service_creation() {
        let container = TestContainer::new().await;
        let auth_service = AuthService::new(container.pool);

        // Service should be created successfully
        assert!(!auth_service.google_client_id.is_empty());
        assert!(!auth_service.facebook_app_id.is_empty());
    }

    /// Test JWT token generation and verification
    #[tokio::test]
    async fn test_jwt_token_flow() {
        let container = TestContainer::new().await;
        let auth_service = AuthService::new(container.pool);

        // Create a test user
        let user = User {
            id: Uuid::new_v4(),
            email: "test@econgraph.com".to_string(),
            name: "Test User".to_string(),
            avatar: None,
            provider: AuthProvider::Email,
            provider_id: "test-provider-id".to_string(),
            role: UserRole::Analyst,
            organization: Some("Test Org".to_string()),
            preferences: UserPreferences::default(),
            created_at: chrono::Utc::now(),
            last_login_at: chrono::Utc::now(),
            is_active: true,
        };

        // Generate token
        let token = auth_service
            .generate_token(&user)
            .expect("Should generate token successfully");

        assert!(!token.is_empty());
        println!("Generated JWT token: {}", token);

        // Verify token
        let claims = auth_service
            .verify_token(&token)
            .expect("Should verify token successfully");

        assert_eq!(claims.email, user.email);
        assert_eq!(claims.name, user.name);
        assert_eq!(claims.role, user.role);
        assert_eq!(claims.sub, user.id.to_string());
    }

    /// Test Google OAuth token verification (mocked)
    #[tokio::test]
    async fn test_google_oauth_flow() {
        let container = TestContainer::new().await;
        let auth_service = AuthService::new(container.pool);

        // Note: In a real test, you would mock the HTTP client
        // For now, we'll test the user creation flow

        let google_user_info = GoogleUserInfo {
            id: "google-123456789".to_string(),
            email: "googleuser@gmail.com".to_string(),
            name: "Google User".to_string(),
            avatar: Some("https://lh3.googleusercontent.com/avatar.jpg".to_string()),
            verified_email: true,
        };

        let user = auth_service
            .create_or_update_oauth_user(
                AuthProvider::Google,
                google_user_info.id.clone(),
                google_user_info.email.clone(),
                google_user_info.name.clone(),
                google_user_info.avatar.clone(),
            )
            .await
            .expect("Should create Google user successfully");

        assert_eq!(user.email, google_user_info.email);
        assert_eq!(user.name, google_user_info.name);
        assert_eq!(user.provider, AuthProvider::Google);
        assert_eq!(user.provider_id, google_user_info.id);
        assert_eq!(user.avatar, google_user_info.avatar);
        assert_eq!(user.role, UserRole::Viewer); // Default role
        assert!(user.is_active);
    }

    /// Test Facebook OAuth flow
    #[tokio::test]
    async fn test_facebook_oauth_flow() {
        let container = TestContainer::new().await;
        let auth_service = AuthService::new(container.pool);

        let facebook_user_info = FacebookUserInfo {
            id: "facebook-987654321".to_string(),
            email: Some("fbuser@facebook.com".to_string()),
            name: "Facebook User".to_string(),
            picture: Some(FacebookPicture {
                data: FacebookPictureData {
                    url: "https://graph.facebook.com/picture.jpg".to_string(),
                },
            }),
        };

        let user = auth_service
            .create_or_update_oauth_user(
                AuthProvider::Facebook,
                facebook_user_info.id.clone(),
                facebook_user_info.email.clone().unwrap(),
                facebook_user_info.name.clone(),
                facebook_user_info
                    .picture
                    .as_ref()
                    .map(|p| p.data.url.clone()),
            )
            .await
            .expect("Should create Facebook user successfully");

        assert_eq!(user.email, facebook_user_info.email.unwrap());
        assert_eq!(user.name, facebook_user_info.name);
        assert_eq!(user.provider, AuthProvider::Facebook);
        assert_eq!(user.provider_id, facebook_user_info.id);
        assert!(user.avatar.is_some());
        assert_eq!(user.role, UserRole::Viewer); // Default role
        assert!(user.is_active);
    }

    /// Test email/password authentication
    #[tokio::test]
    async fn test_email_password_auth() {
        let container = TestContainer::new().await;
        let auth_service = AuthService::new(container.pool);

        // Test user creation
        let email = "newuser@econgraph.com".to_string();
        let password = "securepassword123".to_string();
        let name = "New User".to_string();

        let user = auth_service
            .create_email_user(email.clone(), password.clone(), name.clone())
            .await
            .expect("Should create email user successfully");

        assert_eq!(user.email, email);
        assert_eq!(user.name, name);
        assert_eq!(user.provider, AuthProvider::Email);
        assert!(user.is_active);

        // Create demo user first
        let demo_user_created = auth_service
            .create_email_user(
                "demo@econgraph.com".to_string(),
                "demo123456".to_string(),
                "Demo User".to_string(),
            )
            .await
            .expect("Should create demo user successfully");

        // Test authentication with demo credentials
        let demo_user = auth_service
            .authenticate_email_user("demo@econgraph.com".to_string(), "demo123456".to_string())
            .await
            .expect("Should authenticate demo user successfully");

        assert_eq!(demo_user.email, "demo@econgraph.com");
        assert_eq!(demo_user.name, "Demo User");
        assert_eq!(demo_user.role, UserRole::Viewer); // Default role for new users
    }

    /// Test authentication failure scenarios
    #[tokio::test]
    async fn test_authentication_failures() {
        let container = TestContainer::new().await;
        let auth_service = AuthService::new(container.pool);

        // Test invalid email/password
        let result = auth_service
            .authenticate_email_user(
                "invalid@econgraph.com".to_string(),
                "wrongpassword".to_string(),
            )
            .await;

        assert!(result.is_err());

        // Test invalid JWT token
        let result = auth_service.verify_token("invalid.jwt.token");
        assert!(result.is_err());

        // Test malformed JWT token
        let result = auth_service.verify_token("not-a-jwt-token");
        assert!(result.is_err());
    }

    /// Test profile update functionality
    #[tokio::test]
    async fn test_profile_update() {
        let container = TestContainer::new().await;
        let auth_service = AuthService::new(container.pool);

        // First create a user
        let user = auth_service
            .create_email_user(
                "test@example.com".to_string(),
                "Test User".to_string(),
                "password123".to_string(),
            )
            .await
            .expect("Should create user successfully");

        let update_request = ProfileUpdateRequest {
            name: Some("Updated Name".to_string()),
            avatar: Some("https://example.com/new-avatar.jpg".to_string()),
            organization: Some("Updated Organization".to_string()),
            preferences: Some(UserPreferences {
                theme: "dark".to_string(),
                default_chart_type: "bar".to_string(),
                notifications: false,
                collaboration_enabled: true,
            }),
        };

        let updated_user = auth_service
            .update_user_profile(user.id, update_request.clone())
            .await
            .expect("Should update profile successfully");

        assert_eq!(updated_user.name, update_request.name.unwrap());
        assert_eq!(updated_user.avatar, update_request.avatar);
        assert_eq!(updated_user.organization, update_request.organization);
        assert_eq!(updated_user.preferences.theme, "dark");
        assert!(!updated_user.preferences.notifications);
    }

    /// Test user response serialization
    #[tokio::test]
    async fn test_user_response_serialization() {
        let user = User {
            id: Uuid::new_v4(),
            email: "test@econgraph.com".to_string(),
            name: "Test User".to_string(),
            avatar: Some("https://example.com/avatar.jpg".to_string()),
            provider: AuthProvider::Google,
            provider_id: "google-123".to_string(),
            role: UserRole::Analyst,
            organization: Some("Test Org".to_string()),
            preferences: UserPreferences::default(),
            created_at: chrono::Utc::now(),
            last_login_at: chrono::Utc::now(),
            is_active: true,
        };

        let user_response = UserResponse::from(user.clone());

        assert_eq!(user_response.id, user.id.to_string());
        assert_eq!(user_response.email, user.email);
        assert_eq!(user_response.name, user.name);
        assert_eq!(user_response.avatar, user.avatar);
        assert_eq!(user_response.provider, user.provider);
        assert_eq!(user_response.role, user.role);
        assert_eq!(user_response.organization, user.organization);

        // Test JSON serialization
        let json =
            serde_json::to_string(&user_response).expect("Should serialize UserResponse to JSON");

        assert!(json.contains(&user.email));
        assert!(json.contains(&user.name));
        println!("UserResponse JSON: {}", json);
    }

    /// Test request validation
    #[tokio::test]
    async fn test_request_validation() {
        use validator::Validate;

        // Test valid login request
        let valid_login = LoginRequest {
            email: "valid@econgraph.com".to_string(),
            password: "validpassword123".to_string(),
        };
        assert!(valid_login.validate().is_ok());

        // Test invalid email
        let invalid_email = LoginRequest {
            email: "not-an-email".to_string(),
            password: "validpassword123".to_string(),
        };
        assert!(invalid_email.validate().is_err());

        // Test short password
        let short_password = LoginRequest {
            email: "valid@econgraph.com".to_string(),
            password: "short".to_string(),
        };
        assert!(short_password.validate().is_err());

        // Test valid registration request
        let valid_register = RegisterRequest {
            email: "valid@econgraph.com".to_string(),
            password: "validpassword123".to_string(),
            name: "Valid User".to_string(),
        };
        assert!(valid_register.validate().is_ok());

        // Test short name
        let short_name = RegisterRequest {
            email: "valid@econgraph.com".to_string(),
            password: "validpassword123".to_string(),
            name: "A".to_string(),
        };
        assert!(short_name.validate().is_err());
    }

    /// Test password hashing and verification
    #[tokio::test]
    async fn test_password_hashing() {
        let password = "testsecurepassword123";

        // Create password hash
        let hash = PasswordHash::new(password).expect("Should create password hash successfully");

        assert!(!hash.hash.is_empty());
        assert_ne!(hash.hash, password); // Hash should be different from original

        // Verify correct password
        let is_valid = hash
            .verify(password)
            .expect("Should verify password successfully");
        assert!(is_valid);

        // Verify incorrect password
        let is_invalid = hash
            .verify("wrongpassword")
            .expect("Should verify password successfully");
        assert!(!is_invalid);
    }

    /// Test authentication models deserialization
    #[tokio::test]
    async fn test_model_deserialization() {
        // Test GoogleAuthRequest
        let google_json = json!({
            "token": "google-oauth-token",
            "user_info": {
                "id": "google-123",
                "email": "user@gmail.com",
                "name": "Google User",
                "picture": "https://lh3.googleusercontent.com/avatar.jpg",
                "verified_email": true
            }
        });

        let google_request: GoogleAuthRequest =
            serde_json::from_value(google_json).expect("Should deserialize GoogleAuthRequest");

        assert_eq!(google_request.token, "google-oauth-token");
        assert_eq!(google_request.user_info.id, "google-123");
        assert_eq!(google_request.user_info.email, "user@gmail.com");

        // Test FacebookAuthRequest
        let facebook_json = json!({
            "token": "facebook-access-token"
        });

        let facebook_request: FacebookAuthRequest =
            serde_json::from_value(facebook_json).expect("Should deserialize FacebookAuthRequest");

        assert_eq!(facebook_request.token, "facebook-access-token");
    }
}
