/**
 * REQUIREMENT: JWT authentication middleware for secure API access
 * PURPOSE: Provide JWT token validation for protected endpoints
 * This ensures only authenticated users can access protected resources
 */
use crate::auth::models::Claims;
use crate::auth::services::AuthService;
use std::convert::Infallible;
use warp::{
    filters::{header::headers_cloned, BoxedFilter},
    http::header::{HeaderMap, HeaderValue, AUTHORIZATION},
    reject, Filter, Rejection,
};

/// Custom rejection for authentication errors
#[derive(Debug)]
pub struct AuthError;

impl reject::Reject for AuthError {}

/// Extract JWT token from Authorization header
fn jwt_from_header(headers: &HeaderMap<HeaderValue>) -> Result<String, Rejection> {
    let header = headers
        .get(AUTHORIZATION)
        .ok_or_else(|| reject::custom(AuthError))?;

    let auth_header =
        std::str::from_utf8(header.as_bytes()).map_err(|_| reject::custom(AuthError))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(reject::custom(AuthError));
    }

    Ok(auth_header.trim_start_matches("Bearer ").to_owned())
}

/// Create authentication filter that extracts and validates JWT claims
pub fn with_auth(auth_service: AuthService) -> BoxedFilter<(Claims,)> {
    headers_cloned()
        .map(move |headers: HeaderMap<HeaderValue>| (headers, auth_service.clone()))
        .and_then(
            |(headers, auth_service): (HeaderMap<HeaderValue>, AuthService)| async move {
                match jwt_from_header(&headers) {
                    Ok(token) => match auth_service.verify_token(&token) {
                        Ok(claims) => Ok(claims),
                        Err(_) => Err(reject::custom(AuthError)),
                    },
                    Err(_) => Err(reject::custom(AuthError)),
                }
            },
        )
        .boxed()
}

/// Create optional authentication filter that extracts JWT claims if present
pub fn with_optional_auth(auth_service: AuthService) -> BoxedFilter<(Option<Claims>,)> {
    headers_cloned()
        .map(move |headers: HeaderMap<HeaderValue>| (headers, auth_service.clone()))
        .and_then(
            |(headers, auth_service): (HeaderMap<HeaderValue>, AuthService)| async move {
                match jwt_from_header(&headers) {
                    Ok(token) => match auth_service.verify_token(&token) {
                        Ok(claims) => Ok::<Option<Claims>, warp::Rejection>(Some(claims)),
                        Err(_) => Ok(None),
                    },
                    Err(_) => Ok(None),
                }
            },
        )
        .boxed()
}

/// Handle authentication rejection
pub async fn handle_auth_rejection(err: Rejection) -> Result<impl warp::Reply, Infallible> {
    if err.find::<AuthError>().is_some() {
        let json = warp::reply::json(&serde_json::json!({
            "error": "Authentication required",
            "message": "Valid JWT token required in Authorization header"
        }));
        Ok(warp::reply::with_status(
            json,
            warp::http::StatusCode::UNAUTHORIZED,
        ))
    } else {
        let json = warp::reply::json(&serde_json::json!({
            "error": "Internal server error"
        }));
        Ok(warp::reply::with_status(
            json,
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}
