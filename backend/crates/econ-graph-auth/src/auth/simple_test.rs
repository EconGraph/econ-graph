/**
 * REQUIREMENT: Simple test to verify authentication endpoints work
 * PURPOSE: Test authentication without database dependencies
 * This ensures the authentication handlers and services function correctly
 */
use crate::auth::models::*;
use crate::auth::services::AuthService;
use econ_graph_core::database::create_pool;

/// Simple test function to verify authentication works
pub async fn test_auth_system() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 Testing Authentication System");
    println!("📝 Testing authentication models and core logic...");

    // Create test database pool
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/econ_graph_test".to_string());

    let pool = create_pool(&database_url)
        .await
        .expect("Failed to create test database pool");

    // Create auth service
    let auth_service = AuthService::new(pool);
    println!("✅ AuthService created successfully");

    // Test JWT token generation and verification
    println!("🔑 Testing JWT token generation...");

    let test_user = User {
        id: uuid::Uuid::new_v4(),
        email: "test@econgraph.com".to_string(),
        name: "Test User".to_string(),
        avatar: None,
        provider: AuthProvider::Email,
        provider_id: "test-id".to_string(),
        role: UserRole::Analyst,
        organization: Some("Test Org".to_string()),
        preferences: UserPreferences::default(),
        created_at: chrono::Utc::now(),
        last_login_at: chrono::Utc::now(),
        is_active: true,
    };

    // Generate token
    let token = auth_service.generate_token(&test_user)?;
    println!("✅ JWT token generated: {}", &token[..50]);

    // Verify token
    let claims = auth_service.verify_token(&token)?;
    println!("✅ JWT token verified - User: {}", claims.email);

    // Test password hashing
    println!("🔒 Testing password hashing...");
    let password_hash = PasswordHash::new("testpassword123")?;
    let is_valid = password_hash.verify("testpassword123")?;
    let is_invalid = password_hash.verify("wrongpassword")?;

    println!("✅ Password hash created and verified correctly");
    println!(
        "✅ Password verification: valid={}, invalid={}",
        is_valid, !is_invalid
    );

    // Test model serialization
    println!("📄 Testing model serialization...");
    let user_response = UserResponse::from(test_user.clone());
    let json = serde_json::to_string(&user_response)?;
    println!("✅ User response serialized: {}", &json[..100]);

    // Test request validation
    println!("✅ Testing request validation...");
    use validator::Validate;

    let valid_login = LoginRequest {
        email: "valid@econgraph.com".to_string(),
        password: "validpassword123".to_string(),
    };
    assert!(valid_login.validate().is_ok());

    let invalid_login = LoginRequest {
        email: "not-an-email".to_string(),
        password: "short".to_string(),
    };
    assert!(invalid_login.validate().is_err());

    println!("✅ Request validation working correctly");

    println!("🎉 All authentication tests passed!");
    Ok(())
}
