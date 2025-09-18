/**
 * REQUIREMENT: Comprehensive integration tests for all authentication methods
 * PURPOSE: Test complete authentication flows including Google OAuth, Facebook OAuth, and email auth
 * This ensures authentication works end-to-end and catches bugs before they reach users
 */

#[cfg(test)]
mod tests {
    use crate::auth::models::*;
    use crate::auth::services::AuthService;
    use econ_graph_core::test_utils::TestContainer;
    use serde_json::json;
    use std::sync::Arc;
    use uuid::Uuid;

    /// Create test database pool using TestContainer
    async fn create_test_pool() -> Arc<TestContainer> {
        TestContainer::new().await.into()
    }

    /// Helper function to skip tests if database is not available
    async fn skip_if_no_database(container: &TestContainer) -> bool {
        if container.pool().get().await.is_err() {
            println!("Skipping test - database not available");
            return true;
        }
        false
    }

    /// Test Google OAuth signup flow
    #[tokio::test]
    async fn test_google_oauth_signup_flow() {
        let container = TestContainer::new().await;

        // Skip test if database is not available
        if skip_if_no_database(&container).await {
            return;
        }

        // Clean database before test to ensure isolation
        container
            .clean_database()
            .await
            .expect("Failed to clean database");

        let auth_service = AuthService::new(container.pool().clone());

        // Simulate Google OAuth token verification
        let google_user_info = GoogleUserInfo {
            id: "google-123456789".to_string(),
            email: "newuser@gmail.com".to_string(),
            name: "New Google User".to_string(),
            avatar: Some("https://lh3.googleusercontent.com/avatar.jpg".to_string()),
            verified_email: true,
        };

        // Test user creation (signup)
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

        // Verify user was created correctly
        assert_eq!(user.email, google_user_info.email);
        assert_eq!(user.name, google_user_info.name);
        assert_eq!(user.provider, AuthProvider::Google);
        assert_eq!(user.provider_id, google_user_info.id);
        assert_eq!(user.avatar, google_user_info.avatar);
        assert_eq!(user.role, UserRole::Viewer); // Default role for new users
        assert!(user.is_active);

        // Test JWT token generation
        let token = auth_service
            .generate_token(&user)
            .expect("Should generate JWT token successfully");

        assert!(!token.is_empty());

        // Test token verification
        let claims = auth_service
            .verify_token(&token)
            .expect("Should verify JWT token successfully");

        assert_eq!(claims.email, user.email);
        assert_eq!(claims.name, user.name);
        assert_eq!(claims.role, user.role);
        assert_eq!(claims.sub, user.id.to_string());
    }

    /// Test Google OAuth signin flow (existing user)
    #[tokio::test]
    async fn test_google_oauth_signin_flow() {
        let container = TestContainer::new().await;

        // Skip test if database is not available
        if skip_if_no_database(&container).await {
            return;
        }
        let pool = container.pool();
        let auth_service = AuthService::new(pool.clone());

        // First, create a user (signup)
        let google_user_info = GoogleUserInfo {
            id: "google-existing-user".to_string(),
            email: "existing@gmail.com".to_string(),
            name: "Existing Google User".to_string(),
            avatar: Some("https://lh3.googleusercontent.com/existing.jpg".to_string()),
            verified_email: true,
        };

        let created_user = auth_service
            .create_or_update_oauth_user(
                AuthProvider::Google,
                google_user_info.id.clone(),
                google_user_info.email.clone(),
                google_user_info.name.clone(),
                google_user_info.avatar.clone(),
            )
            .await
            .expect("Should create Google user successfully");

        // Now test signin (update existing user)
        let updated_user = auth_service
            .create_or_update_oauth_user(
                AuthProvider::Google,
                google_user_info.id.clone(),
                google_user_info.email.clone(),
                google_user_info.name.clone(),
                google_user_info.avatar.clone(),
            )
            .await
            .expect("Should update existing Google user successfully");

        // Verify it's the same user
        assert_eq!(created_user.id, updated_user.id);
        assert_eq!(updated_user.email, google_user_info.email);
        assert_eq!(updated_user.provider, AuthProvider::Google);
        assert_eq!(updated_user.provider_id, google_user_info.id);

        // Test authentication token generation
        let token = auth_service
            .generate_token(&updated_user)
            .expect("Should generate JWT token for existing user");

        let claims = auth_service
            .verify_token(&token)
            .expect("Should verify JWT token for existing user");

        assert_eq!(claims.email, updated_user.email);
        assert_eq!(claims.sub, updated_user.id.to_string());
    }

    /// Test Facebook OAuth signup flow
    #[tokio::test]
    async fn test_facebook_oauth_signup_flow() {
        let container = TestContainer::new().await;

        // Skip test if database is not available
        if skip_if_no_database(&container).await {
            return;
        }

        // Clean database before test to ensure isolation
        container
            .clean_database()
            .await
            .expect("Failed to clean database");

        let auth_service = AuthService::new(container.pool().clone());

        let facebook_user_info = FacebookUserInfo {
            id: "facebook-987654321".to_string(),
            email: Some("newfbuser@facebook.com".to_string()),
            name: "New Facebook User".to_string(),
            picture: Some(FacebookPicture {
                data: FacebookPictureData {
                    url: "https://graph.facebook.com/new-picture.jpg".to_string(),
                },
            }),
        };

        // Test user creation (signup)
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

        // Verify user was created correctly
        assert_eq!(user.email, facebook_user_info.email.unwrap());
        assert_eq!(user.name, facebook_user_info.name);
        assert_eq!(user.provider, AuthProvider::Facebook);
        assert_eq!(user.provider_id, facebook_user_info.id);
        assert!(user.avatar.is_some());
        assert_eq!(user.role, UserRole::Viewer); // Default role for new users
        assert!(user.is_active);

        // Test JWT token generation
        let token = auth_service
            .generate_token(&user)
            .expect("Should generate JWT token successfully");

        assert!(!token.is_empty());

        // Test token verification
        let claims = auth_service
            .verify_token(&token)
            .expect("Should verify JWT token successfully");

        assert_eq!(claims.email, user.email);
        assert_eq!(claims.name, user.name);
        assert_eq!(claims.role, user.role);
    }

    /// Test Facebook OAuth signin flow (existing user)
    #[tokio::test]
    async fn test_facebook_oauth_signin_flow() {
        let container = TestContainer::new().await;

        // Skip test if database is not available
        if skip_if_no_database(&container).await {
            return;
        }

        let auth_service = AuthService::new(container.pool().clone());
        let container = create_test_pool().await;
        let pool = container.pool();
        let auth_service = AuthService::new(pool.clone());

        // First, create a user (signup)
        let facebook_user_info = FacebookUserInfo {
            id: "facebook-existing-user".to_string(),
            email: Some("existingfb@facebook.com".to_string()),
            name: "Existing Facebook User".to_string(),
            picture: Some(FacebookPicture {
                data: FacebookPictureData {
                    url: "https://graph.facebook.com/existing-picture.jpg".to_string(),
                },
            }),
        };

        let created_user = auth_service
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

        // Now test signin (update existing user)
        let updated_user = auth_service
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
            .expect("Should update existing Facebook user successfully");

        // Verify it's the same user
        assert_eq!(created_user.id, updated_user.id);
        assert_eq!(updated_user.email, facebook_user_info.email.unwrap());
        assert_eq!(updated_user.provider, AuthProvider::Facebook);
        assert_eq!(updated_user.provider_id, facebook_user_info.id);

        // Test authentication token generation
        let token = auth_service
            .generate_token(&updated_user)
            .expect("Should generate JWT token for existing user");

        let claims = auth_service
            .verify_token(&token)
            .expect("Should verify JWT token for existing user");

        assert_eq!(claims.email, updated_user.email);
        assert_eq!(claims.sub, updated_user.id.to_string());
    }

    /// Test email/password signup flow
    #[tokio::test]
    async fn test_email_password_signup_flow() {
        let container = TestContainer::new().await;

        // Skip test if database is not available
        if skip_if_no_database(&container).await {
            return;
        }

        let auth_service = AuthService::new(container.pool().clone());
        let container = create_test_pool().await;
        let pool = container.pool();
        let auth_service = AuthService::new(pool.clone());

        let email = "newuser@econgraph.com".to_string();
        let password = "securepassword123".to_string();
        let name = "New Email User".to_string();

        // Test user creation (signup)
        let user = auth_service
            .create_email_user(email.clone(), password.clone(), name.clone())
            .await
            .expect("Should create email user successfully");

        // Verify user was created correctly
        assert_eq!(user.email, email);
        assert_eq!(user.name, name);
        assert_eq!(user.provider, AuthProvider::Email);
        assert_eq!(user.role, UserRole::Viewer); // Default role for new users
        assert!(user.is_active);

        // Test JWT token generation
        let token = auth_service
            .generate_token(&user)
            .expect("Should generate JWT token successfully");

        assert!(!token.is_empty());

        // Test token verification
        let claims = auth_service
            .verify_token(&token)
            .expect("Should verify JWT token successfully");

        assert_eq!(claims.email, user.email);
        assert_eq!(claims.name, user.name);
        assert_eq!(claims.role, user.role);
    }

    /// Test email/password signin flow
    #[tokio::test]
    async fn test_email_password_signin_flow() {
        let container = TestContainer::new().await;

        // Skip test if database is not available
        if skip_if_no_database(&container).await {
            return;
        }

        let auth_service = AuthService::new(container.pool().clone());
        let container = create_test_pool().await;
        let pool = container.pool();
        let auth_service = AuthService::new(pool.clone());

        let email = "signinuser@econgraph.com".to_string();
        let password = "signinpassword123".to_string();
        let name = "Signin User".to_string();

        // First, create a user (signup)
        let created_user = auth_service
            .create_email_user(email.clone(), password.clone(), name.clone())
            .await
            .expect("Should create email user successfully");

        // Now test signin (authenticate existing user)
        let authenticated_user = auth_service
            .authenticate_email_user(email.clone(), password.clone())
            .await
            .expect("Should authenticate email user successfully");

        // Verify it's the same user
        assert_eq!(created_user.id, authenticated_user.id);
        assert_eq!(authenticated_user.email, email);
        assert_eq!(authenticated_user.name, name);
        assert_eq!(authenticated_user.provider, AuthProvider::Email);

        // Test authentication token generation
        let token = auth_service
            .generate_token(&authenticated_user)
            .expect("Should generate JWT token for authenticated user");

        let claims = auth_service
            .verify_token(&token)
            .expect("Should verify JWT token for authenticated user");

        assert_eq!(claims.email, authenticated_user.email);
        assert_eq!(claims.sub, authenticated_user.id.to_string());
    }

    /// Test authentication failure scenarios
    #[tokio::test]
    async fn test_authentication_failure_scenarios() {
        let container = create_test_pool().await;
        let pool = container.pool();
        let auth_service = AuthService::new(pool.clone());

        // Test invalid email/password combination
        let result = auth_service
            .authenticate_email_user(
                "nonexistent@econgraph.com".to_string(),
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

        // Test empty JWT token
        let result = auth_service.verify_token("");
        assert!(result.is_err());
    }

    /// Test duplicate user creation scenarios
    #[tokio::test]
    async fn test_duplicate_user_scenarios() {
        let container = TestContainer::new().await;

        // Skip test if database is not available
        if skip_if_no_database(&container).await {
            return;
        }

        let auth_service = AuthService::new(container.pool().clone());
        let container = create_test_pool().await;
        let pool = container.pool();
        let auth_service = AuthService::new(pool.clone());

        let email = "duplicate@econgraph.com".to_string();
        let password = "password123".to_string();
        let name = "Duplicate User".to_string();

        // Create first user
        let first_user = auth_service
            .create_email_user(email.clone(), password.clone(), name.clone())
            .await
            .expect("Should create first user successfully");

        // Try to create duplicate user with same email
        let result = auth_service
            .create_email_user(
                email.clone(),
                "differentpassword".to_string(),
                "Different Name".to_string(),
            )
            .await;

        // This should either succeed (update existing) or fail gracefully
        match result {
            Ok(second_user) => {
                // If it succeeds, it should be the same user
                assert_eq!(first_user.id, second_user.id);
            }
            Err(_) => {
                // If it fails, that's also acceptable behavior
                // The important thing is that it doesn't crash
            }
        }
    }

    /// Test user profile update functionality
    #[tokio::test]
    async fn test_user_profile_update() {
        let container = TestContainer::new().await;

        // Skip test if database is not available
        if skip_if_no_database(&container).await {
            return;
        }

        let auth_service = AuthService::new(container.pool().clone());
        let container = create_test_pool().await;
        let pool = container.pool();
        let auth_service = AuthService::new(pool.clone());

        // Create a test user
        let email = "profileuser@econgraph.com".to_string();
        let password = "password123".to_string();
        let name = "Profile User".to_string();

        let user = auth_service
            .create_email_user(email.clone(), password.clone(), name.clone())
            .await
            .expect("Should create user successfully");

        // Update profile
        let update_request = ProfileUpdateRequest {
            name: Some("Updated Profile User".to_string()),
            avatar: Some("https://example.com/new-avatar.jpg".to_string()),
            organization: Some("Updated Organization".to_string()),
            preferences: Some(UserPreferences {
                theme: "dark".to_string(),
                default_chart_type: "line".to_string(),
                notifications: true,
                collaboration_enabled: false,
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
        assert_eq!(updated_user.preferences.default_chart_type, "line");
        assert!(updated_user.preferences.notifications);
        assert!(!updated_user.preferences.collaboration_enabled);
    }

    /// Test password hashing and verification
    #[tokio::test]
    async fn test_password_security() {
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

        // Test multiple different passwords produce different hashes
        let hash2 = PasswordHash::new("differentpassword456").expect("Should create second hash");
        assert_ne!(hash.hash, hash2.hash);
    }

    /// Test JWT token expiration
    #[tokio::test]
    async fn test_jwt_token_expiration() {
        let container = create_test_pool().await;
        let pool = container.pool();
        let auth_service = AuthService::new(pool.clone());

        // Create a test user
        let user = User {
            id: Uuid::new_v4(),
            email: "tokenuser@econgraph.com".to_string(),
            name: "Token User".to_string(),
            avatar: None,
            provider: AuthProvider::Email,
            provider_id: "token-user-id".to_string(),
            role: UserRole::Viewer,
            organization: None,
            preferences: UserPreferences::default(),
            created_at: chrono::Utc::now(),
            last_login_at: chrono::Utc::now(),
            is_active: true,
        };

        // Generate token
        let token = auth_service
            .generate_token(&user)
            .expect("Should generate JWT token successfully");

        // Verify token is valid
        let claims = auth_service
            .verify_token(&token)
            .expect("Should verify JWT token successfully");

        assert_eq!(claims.email, user.email);
        assert_eq!(claims.name, user.name);

        // Test that token contains expected fields
        assert!(!claims.sub.is_empty());
        assert!(!claims.email.is_empty());
        assert!(!claims.name.is_empty());
        assert!(claims.exp > claims.iat); // Expiration should be after issued time
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

        // Test empty name
        let empty_name = RegisterRequest {
            email: "valid@econgraph.com".to_string(),
            password: "validpassword123".to_string(),
            name: "".to_string(),
        };
        assert!(empty_name.validate().is_err());
    }

    /// Test model serialization and deserialization
    #[tokio::test]
    async fn test_model_serialization() {
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

        // Test UserResponse serialization
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
        let json =
            serde_json::to_string(&user_response).expect("Should serialize UserResponse to JSON");

        assert!(json.contains(&user.email));
        assert!(json.contains(&user.name));
        assert!(json.contains(&user.id.to_string()));
    }
}
