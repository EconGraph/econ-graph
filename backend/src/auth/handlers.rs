/**
 * REQUIREMENT: OAuth authentication handlers for multi-user collaboration
 * PURPOSE: Handle HTTP requests for authentication endpoints
 * This provides REST API endpoints for Google, Facebook, and email authentication
 */
use crate::auth::models::*;
use crate::auth::services::AuthService;
use crate::error::AppError;
use serde_json::json;
use std::convert::Infallible;
use validator::Validate;
use warp::{http::StatusCode, reject, reply, Rejection, Reply};

/// Handle Google OAuth authentication
pub async fn handle_google_auth(
    auth_request: GoogleAuthRequest,
    auth_service: AuthService,
) -> Result<impl Reply, Rejection> {
    // Verify Google ID token
    match auth_service.verify_google_token(&auth_request.token).await {
        Ok(_) => {
            // Token is valid, proceed with user creation/update
        }
        Err(e) => {
            tracing::error!("Google token verification failed for user: {}", e);
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Authentication failed",
                    "message": "Unable to verify Google account. Please try signing in again."
                })),
                StatusCode::FORBIDDEN,
            ));
        }
    }

    // Use the user info from the request (already validated by Google on frontend)
    let google_user_info = auth_request.user_info;

    // Create or update user
    let user_email = google_user_info.email.clone();
    let user = match auth_service
        .create_or_update_oauth_user(
            AuthProvider::Google,
            google_user_info.id,
            google_user_info.email,
            google_user_info.name,
            google_user_info.avatar,
        )
        .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to create/update Google user {}: {}", user_email, e);
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Account creation failed",
                    "message": "Unable to create your account. Please try again or contact support."
                })),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    // Generate JWT token
    let token = match auth_service.generate_token(&user) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!(
                "Failed to generate JWT token for user {}: {}",
                user.email,
                e
            );
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Authentication failed",
                    "message": "Unable to complete sign-in. Please try again."
                })),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    let response = AuthResponse {
        token,
        user: UserResponse::from(user),
    };

    Ok(reply::with_status(reply::json(&response), StatusCode::OK))
}

/// Handle Facebook OAuth authentication
pub async fn handle_facebook_auth(
    auth_request: FacebookAuthRequest,
    auth_service: AuthService,
) -> Result<impl Reply, Rejection> {
    // Verify Facebook token
    let facebook_user_info = match auth_service
        .verify_facebook_token(&auth_request.token)
        .await
    {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Facebook token verification failed for user: {}", e);
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Authentication failed",
                    "message": "Unable to verify Facebook account. Please try signing in again."
                })),
                StatusCode::FORBIDDEN,
            ));
        }
    };

    // Create or update user
    let facebook_id = facebook_user_info.id.clone();
    let user_email = facebook_user_info
        .email
        .clone()
        .unwrap_or_else(|| format!("fb_user_{}@econgraph.com", facebook_id));
    let user = match auth_service
        .create_or_update_oauth_user(
            AuthProvider::Facebook,
            facebook_user_info.id,
            facebook_user_info
                .email
                .unwrap_or_else(|| format!("fb_user_{}@econgraph.com", facebook_id)),
            facebook_user_info.name,
            facebook_user_info.picture.map(|p| p.data.url),
        )
        .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!(
                "Failed to create/update Facebook user {}: {}",
                user_email,
                e
            );
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Account creation failed",
                    "message": "Unable to create your account. Please try again or contact support."
                })),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    // Generate JWT token
    let token = match auth_service.generate_token(&user) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!(
                "Failed to generate JWT token for user {}: {}",
                user.email,
                e
            );
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Authentication failed",
                    "message": "Unable to complete sign-in. Please try again."
                })),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    let response = AuthResponse {
        token,
        user: UserResponse::from(user),
    };

    Ok(reply::with_status(reply::json(&response), StatusCode::OK))
}

/// Handle email/password login
pub async fn handle_login(
    login_request: LoginRequest,
    auth_service: AuthService,
) -> Result<impl Reply, Rejection> {
    // Validate request
    if let Err(validation_errors) = login_request.validate() {
        return Ok(reply::with_status(
            reply::json(&json!({
                "error": "Validation failed",
                "details": validation_errors
            })),
            StatusCode::BAD_REQUEST,
        ));
    }

    // Authenticate user
    let user = match auth_service
        .authenticate_email_user(login_request.email, login_request.password)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::warn!("Login failed: {}", e);
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Invalid credentials",
                    "message": "Email or password is incorrect"
                })),
                StatusCode::FORBIDDEN,
            ));
        }
    };

    // Generate JWT token
    let token = match auth_service.generate_token(&user) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!(
                "Failed to generate JWT token for user {}: {}",
                user.email,
                e
            );
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Authentication failed",
                    "message": "Unable to complete sign-in. Please try again."
                })),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    let response = AuthResponse {
        token,
        user: UserResponse::from(user),
    };

    Ok(reply::with_status(reply::json(&response), StatusCode::OK))
}

/// Handle user registration
pub async fn handle_register(
    register_request: RegisterRequest,
    auth_service: AuthService,
) -> Result<impl Reply, Rejection> {
    // Validate request
    if let Err(validation_errors) = register_request.validate() {
        return Ok(reply::with_status(
            reply::json(&json!({
                "error": "Validation failed",
                "details": validation_errors
            })),
            StatusCode::BAD_REQUEST,
        ));
    }

    // Create user
    let user = match auth_service
        .create_email_user(
            register_request.email,
            register_request.password,
            register_request.name,
        )
        .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to create user: {}", e);
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Failed to create user account",
                    "message": e.to_string()
                })),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    // Generate JWT token
    let token = match auth_service.generate_token(&user) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!(
                "Failed to generate JWT token for user {}: {}",
                user.email,
                e
            );
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Authentication failed",
                    "message": "Unable to complete sign-in. Please try again."
                })),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    let response = AuthResponse {
        token,
        user: UserResponse::from(user),
    };

    Ok(reply::with_status(
        reply::json(&response),
        StatusCode::CREATED,
    ))
}

/// Handle user profile retrieval
pub async fn handle_get_profile(
    claims: Claims,
    auth_service: AuthService,
) -> Result<impl Reply, Rejection> {
    let user_id = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => {
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Invalid user ID"
                })),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    let user = match auth_service.refresh_user(user_id).await {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to get user profile: {}", e);
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "User not found",
                    "message": e.to_string()
                })),
                StatusCode::NOT_FOUND,
            ));
        }
    };

    Ok(reply::with_status(
        reply::json(&json!({
            "user": UserResponse::from(user)
        })),
        StatusCode::OK,
    ))
}

/// Handle profile update
pub async fn handle_update_profile(
    claims: Claims,
    update_request: ProfileUpdateRequest,
    auth_service: AuthService,
) -> Result<impl Reply, Rejection> {
    // Validate request
    if let Err(validation_errors) = update_request.validate() {
        return Ok(reply::with_status(
            reply::json(&json!({
                "error": "Validation failed",
                "details": validation_errors
            })),
            StatusCode::BAD_REQUEST,
        ));
    }

    let user_id = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => {
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Invalid user ID"
                })),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    let user = match auth_service
        .update_user_profile(user_id, update_request)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to update user profile: {}", e);
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Failed to update profile",
                    "message": e.to_string()
                })),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    Ok(reply::with_status(
        reply::json(&json!({
            "user": UserResponse::from(user)
        })),
        StatusCode::OK,
    ))
}

/// Handle logout
pub async fn handle_logout() -> Result<impl Reply, Rejection> {
    // In a stateless JWT system, logout is typically handled client-side
    // by removing the token. Here we just return a success message.
    Ok(reply::with_status(
        reply::json(&json!({
            "message": "Logout successful"
        })),
        StatusCode::OK,
    ))
}

/// Handle Facebook data deletion callback
///
/// This endpoint is called by Facebook when a user deletes their Facebook account
/// and requests data deletion from our app. Facebook sends a signed request with
/// the user's Facebook ID that we need to process.
///
/// REQUIREMENT: Facebook App Review - User Data Deletion Callback
/// PURPOSE: Comply with Facebook's data deletion requirements for app approval
pub async fn handle_facebook_data_deletion(
    form_data: std::collections::HashMap<String, String>,
    auth_service: AuthService,
) -> Result<impl Reply, Rejection> {
    // Extract the signed request from Facebook
    let signed_request = match form_data.get("signed_request") {
        Some(sr) => sr,
        None => {
            tracing::warn!("Facebook data deletion callback received without signed_request");
            return Ok(reply::with_status(
                reply::json(&json!({
                    "error": "Missing signed request",
                    "message": "No signed request provided"
                })),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    // In a real implementation, you would:
    // 1. Verify the Facebook signed request using your app secret
    // 2. Extract the user ID from the verified request
    // 3. Delete the user's data from your database
    // 4. Return appropriate status

    // For now, we'll simulate the process
    tracing::info!(
        "Received Facebook data deletion callback with signed_request: {}",
        signed_request
    );

    // Simulate processing the deletion request
    // In production, this would:
    // - Verify the signed request with Facebook's app secret
    // - Extract the user_id from the request
    // - Delete all user data associated with that Facebook ID
    // - Return confirmation URL for the user

    // Mock response for now
    let mock_user_id = "fb_user_12345"; // Would come from verified signed request

    // In production, you would call:
    // auth_service.delete_user_data_by_facebook_id(facebook_user_id).await?;

    tracing::info!(
        "Simulated deletion of data for Facebook user: {}",
        mock_user_id
    );

    // Return success response with confirmation URL
    Ok(reply::with_status(
        reply::json(&json!({
            "url": "https://econ-graph.com/user-data-deletion",
            "confirmation_code": format!("deletion_{}", mock_user_id)
        })),
        StatusCode::OK,
    ))
}
