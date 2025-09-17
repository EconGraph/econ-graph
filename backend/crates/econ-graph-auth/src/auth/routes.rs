/**
 * REQUIREMENT: OAuth authentication routes for multi-user collaboration
 * PURPOSE: Define REST API routes for authentication endpoints
 * This provides the HTTP routes that the frontend expects for authentication
 */
use crate::auth::handlers::*;
use crate::auth::middleware::{handle_auth_rejection, with_auth};
use crate::auth::services::AuthService;
use warp::{filters::BoxedFilter, Filter, Reply};

/// Create authentication routes
pub fn auth_routes(auth_service: AuthService) -> BoxedFilter<(impl Reply,)> {
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type", "authorization"])
        .allow_methods(vec!["GET", "POST", "PATCH", "DELETE", "OPTIONS"]);

    // Google OAuth route
    let google_auth = warp::path!("auth" / "google")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_auth_service(auth_service.clone()))
        .and_then(handle_google_auth);

    // Facebook OAuth route
    let facebook_auth = warp::path!("auth" / "facebook")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_auth_service(auth_service.clone()))
        .and_then(handle_facebook_auth);

    // Email login route
    let login = warp::path!("auth" / "login")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_auth_service(auth_service.clone()))
        .and_then(handle_login);

    // User registration route
    let register = warp::path!("auth" / "register")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_auth_service(auth_service.clone()))
        .and_then(handle_register);

    // Get user profile route (protected)
    let get_profile = warp::path!("auth" / "me")
        .and(warp::get())
        .and(with_auth(auth_service.clone()))
        .and(with_auth_service(auth_service.clone()))
        .and_then(handle_get_profile);

    // Update user profile route (protected)
    let update_profile = warp::path!("auth" / "profile")
        .and(warp::patch())
        .and(with_auth(auth_service.clone()))
        .and(warp::body::json())
        .and(with_auth_service(auth_service.clone()))
        .and_then(handle_update_profile);

    // Logout route
    let logout = warp::path!("auth" / "logout")
        .and(warp::post())
        .and_then(handle_logout);

    // Facebook data deletion callback route
    let facebook_data_deletion = warp::path!("auth" / "facebook" / "data-deletion")
        .and(warp::post())
        .and(warp::body::form())
        .and(with_auth_service(auth_service.clone()))
        .and_then(handle_facebook_data_deletion);

    google_auth
        .or(facebook_auth)
        .or(login)
        .or(register)
        .or(get_profile)
        .or(update_profile)
        .or(logout)
        .or(facebook_data_deletion)
        .with(cors)
        .recover(handle_auth_rejection)
        .boxed()
}

/// Helper filter to provide AuthService to handlers
fn with_auth_service(
    auth_service: AuthService,
) -> impl Filter<Extract = (AuthService,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || auth_service.clone())
}
