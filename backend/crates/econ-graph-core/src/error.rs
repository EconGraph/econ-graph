//! # Error Handling
//!
//! This module provides comprehensive error handling for the `EconGraph` system.
//! It defines custom error types, logging utilities, and HTTP error responses.
//!
//! ## Features
//!
//! - **Custom Error Types**: Application-specific error variants with context
//! - **Error Logging**: Structured logging with context and tracing
//! - **HTTP Integration**: Automatic conversion to HTTP responses
//! - **Error Macros**: Convenient macros for error handling and logging
//! - **Warp Integration**: Seamless integration with Warp web framework
//!
//! ## Usage
//!
//! ```rust,no_run
//! use econ_graph_core::error::{AppError, AppResult};
//! use econ_graph_core::log_and_return;
//!
//! async fn example_function() -> AppResult<String> {
//!     // Create an error with context
//!     let error = AppError::ValidationError("Invalid input".to_string());
//!     log_and_return!(error, "Failed to process user input");
//! }
//! ```

use serde_json::json;
use thiserror::Error;
use warp::{http::StatusCode, reject::Reject, Reply};

/// Macro to log an error with context before returning it
/// Usage: `log_and_return!(error, "context message")`
#[macro_export]
macro_rules! log_and_return {
    ($error:expr, $context:expr) => {{
        $error.log_with_context($context);
        return Err($error);
    }};
}

/// Macro to log an error with context and return it as a Result
/// Usage: `log_error!(error, "context message")`
#[macro_export]
macro_rules! log_error {
    ($error:expr, $context:expr) => {{
        $error.log_with_context($context);
        Err($error)
    }};
}

/// Application-specific error types
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("Database pool error: {0}")]
    DatabasePool(String),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Validation errors: {0}")]
    ValidationErrors(#[from] validator::ValidationErrors),

    #[error("Connection pool error: {0}")]
    ConnectionPool(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    #[error("Internal server error: {0}")]
    InternalError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Series not found: {0}")]
    SeriesNotFound(String),

    #[error("Data source not found: {0}")]
    DataSourceNotFound(String),

    #[error("Invalid date format: {0}")]
    InvalidDateFormat(String),

    #[error("Invalid transformation: {0}")]
    InvalidTransformation(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("External API error: {0}")]
    ExternalApiError(String),

    #[error("Parser error: {0}")]
    ParserError(String),

    #[error("Migration error: {0}")]
    MigrationError(String),

    #[error("Crawler error: {0}")]
    CrawlerError(String),

    #[error("Search error: {0}")]
    SearchError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl Reject for AppError {}

/// Convert `AppError` to HTTP response
///
/// # Errors
///
/// This function never returns an error as it converts all errors to HTTP responses
pub fn handle_rejection(err: warp::Rejection) -> Result<impl Reply, std::convert::Infallible> {
    let (code, message) = if err.is_not_found() {
        tracing::warn!("404 Not Found: {:?}", err);
        (StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if let Some(app_error) = err.find::<AppError>() {
        // Log detailed error information for all AppError variants
        match app_error {
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            }
            AppError::DatabasePool(msg) => {
                tracing::error!("Database pool error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database connection error".to_string(),
                )
            }
            AppError::HttpClient(e) => {
                tracing::error!("HTTP client error: {}", e);
                (
                    StatusCode::BAD_GATEWAY,
                    "External service error".to_string(),
                )
            }
            AppError::JsonSerialization(e) => {
                tracing::error!("JSON serialization error: {}", e);
                (StatusCode::BAD_REQUEST, "Invalid JSON".to_string())
            }
            AppError::Validation(msg) => {
                tracing::warn!("Validation error: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::ValidationErrors(e) => {
                tracing::warn!("Validation errors: {}", e);
                (StatusCode::BAD_REQUEST, "Validation failed".to_string())
            }
            AppError::ConnectionPool(msg) => {
                tracing::error!("Connection pool error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Connection pool error".to_string(),
                )
            }
            AppError::ConfigError(msg) => {
                tracing::error!("Configuration error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Configuration error".to_string(),
                )
            }
            AppError::Io(e) => {
                tracing::error!("I/O error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "I/O error".to_string())
            }
            AppError::AuthenticationError(msg) => {
                tracing::warn!("Authentication error: {}", msg);
                (StatusCode::UNAUTHORIZED, msg.clone())
            }
            AppError::InternalError(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::DatabaseError(msg) => {
                tracing::error!("Database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            }
            AppError::ValidationError(msg) => {
                tracing::warn!("Validation error: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::SeriesNotFound(msg) => {
                tracing::info!("Series not found: {}", msg);
                (StatusCode::NOT_FOUND, msg.clone())
            }
            AppError::DataSourceNotFound(msg) => {
                tracing::info!("Data source not found: {}", msg);
                (StatusCode::NOT_FOUND, msg.clone())
            }
            AppError::InvalidDateFormat(msg) => {
                tracing::warn!("Invalid date format: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::InvalidTransformation(msg) => {
                tracing::warn!("Invalid transformation: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::RateLimitExceeded => {
                tracing::warn!("Rate limit exceeded");
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    "Rate limit exceeded".to_string(),
                )
            }
            AppError::ExternalApiError(msg) => {
                tracing::error!("External API error: {}", msg);
                (StatusCode::BAD_GATEWAY, "External API error".to_string())
            }
            AppError::ParserError(msg) => {
                tracing::error!("Parser error: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::MigrationError(msg) => {
                tracing::error!("Migration error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Migration error".to_string(),
                )
            }
            AppError::CrawlerError(msg) => {
                tracing::error!("Crawler error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Crawler error".to_string(),
                )
            }
            AppError::SearchError(msg) => {
                tracing::error!("Search error: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::PermissionDenied(msg) => {
                tracing::warn!("Permission denied: {}", msg);
                (StatusCode::FORBIDDEN, msg.clone())
            }
            AppError::NotFound(msg) => {
                tracing::info!("Not found: {}", msg);
                (StatusCode::NOT_FOUND, msg.clone())
            }
            AppError::BadRequest(msg) => {
                tracing::warn!("Bad request: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::Unauthorized(msg) => {
                tracing::warn!("Unauthorized: {}", msg);
                (StatusCode::UNAUTHORIZED, msg.clone())
            }
            AppError::Forbidden(msg) => {
                tracing::warn!("Forbidden: {}", msg);
                (StatusCode::FORBIDDEN, msg.clone())
            }
            AppError::Conflict(msg) => {
                tracing::warn!("Conflict: {}", msg);
                (StatusCode::CONFLICT, msg.clone())
            }
            AppError::UnprocessableEntity(msg) => {
                tracing::warn!("Unprocessable entity: {}", msg);
                (StatusCode::UNPROCESSABLE_ENTITY, msg.clone())
            }
            AppError::ServiceUnavailable(msg) => {
                tracing::error!("Service unavailable: {}", msg);
                (StatusCode::SERVICE_UNAVAILABLE, msg.clone())
            }
        }
    } else if err
        .find::<warp::filters::body::BodyDeserializeError>()
        .is_some()
    {
        tracing::warn!("Invalid request body: {:?}", err);
        (StatusCode::BAD_REQUEST, "Invalid request body".to_string())
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        tracing::warn!("Method not allowed: {:?}", err);
        (
            StatusCode::METHOD_NOT_ALLOWED,
            "Method not allowed".to_string(),
        )
    } else {
        tracing::error!("Unhandled rejection: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        )
    };

    let json = warp::reply::json(&json!({
        "error": message,
        "code": code.as_u16()
    }));

    Ok(warp::reply::with_status(json, code))
}

/// Result type alias for convenience
pub type AppResult<T> = Result<T, AppError>;

/// Convert from bb8 pool error
impl From<bb8::RunError<diesel_async::pooled_connection::PoolError>> for AppError {
    fn from(err: bb8::RunError<diesel_async::pooled_connection::PoolError>) -> Self {
        AppError::ConnectionPool(err.to_string())
    }
}

/// Convert from diesel migration error
impl From<diesel_migrations::MigrationError> for AppError {
    fn from(err: diesel_migrations::MigrationError) -> Self {
        AppError::MigrationError(err.to_string())
    }
}

/// Convert from chrono parse error
impl From<chrono::ParseError> for AppError {
    fn from(err: chrono::ParseError) -> Self {
        AppError::InvalidDateFormat(err.to_string())
    }
}

/// Convert from uuid parse error
impl From<uuid::Error> for AppError {
    fn from(err: uuid::Error) -> Self {
        AppError::ValidationError(format!("Invalid UUID: {}", err))
    }
}

/// Convert from bigdecimal parse error
impl From<bigdecimal::ParseBigDecimalError> for AppError {
    fn from(err: bigdecimal::ParseBigDecimalError) -> Self {
        AppError::ValidationError(format!("Invalid decimal: {}", err))
    }
}

/// Convert from config error
impl From<config::ConfigError> for AppError {
    fn from(err: config::ConfigError) -> Self {
        AppError::ConfigError(err.to_string())
    }
}

/// Convert from async-graphql error
impl From<async_graphql::Error> for AppError {
    fn from(err: async_graphql::Error) -> Self {
        AppError::ValidationError(err.message)
    }
}

/// Convert from JWT errors
impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        AppError::AuthenticationError(format!("JWT error: {}", err))
    }
}

/// Convert from bcrypt errors
impl From<bcrypt::BcryptError> for AppError {
    fn from(err: bcrypt::BcryptError) -> Self {
        AppError::AuthenticationError(format!("Password hashing error: {}", err))
    }
}

/// Utility functions for common error scenarios
impl AppError {
    pub fn not_found<T: std::fmt::Display>(resource: T) -> Self {
        AppError::NotFound(format!("{} not found", resource))
    }

    pub fn bad_request<T: std::fmt::Display>(message: T) -> Self {
        AppError::BadRequest(message.to_string())
    }

    pub fn unauthorized<T: std::fmt::Display>(message: T) -> Self {
        AppError::Unauthorized(message.to_string())
    }

    pub fn forbidden<T: std::fmt::Display>(message: T) -> Self {
        AppError::Forbidden(message.to_string())
    }

    pub fn internal_error<T: std::fmt::Display>(message: T) -> Self {
        AppError::InternalError(message.to_string())
    }

    pub fn validation_error<T: std::fmt::Display>(message: T) -> Self {
        AppError::ValidationError(message.to_string())
    }

    pub fn database_error<T: std::fmt::Display>(message: T) -> Self {
        AppError::DatabaseError(message.to_string())
    }

    pub fn authentication_error<T: std::fmt::Display>(message: T) -> Self {
        AppError::AuthenticationError(message.to_string())
    }

    /// Log the error with appropriate level and context
    /// This should be called before returning the error to ensure it's logged
    pub fn log_with_context(&self, context: &str) {
        match self {
            // Critical errors that need immediate attention
            AppError::Database(_)
            | AppError::DatabasePool(_)
            | AppError::DatabaseError(_)
            | AppError::ConnectionPool(_)
            | AppError::MigrationError(_)
            | AppError::InternalError(_)
            | AppError::ServiceUnavailable(_) => {
                tracing::error!("{} - {}: {}", context, self.error_type(), self);
            }
            // External service errors
            AppError::HttpClient(_) | AppError::ExternalApiError(_) | AppError::CrawlerError(_) => {
                tracing::error!("{} - {}: {}", context, self.error_type(), self);
            }
            // Client errors (warnings)
            AppError::Validation(_)
            | AppError::ValidationErrors(_)
            | AppError::ValidationError(_)
            | AppError::BadRequest(_)
            | AppError::InvalidDateFormat(_)
            | AppError::InvalidTransformation(_)
            | AppError::ParserError(_)
            | AppError::SearchError(_) => {
                tracing::warn!("{} - {}: {}", context, self.error_type(), self);
            }
            // Authentication/authorization errors
            AppError::AuthenticationError(_)
            | AppError::Unauthorized(_)
            | AppError::Forbidden(_)
            | AppError::PermissionDenied(_) => {
                tracing::warn!("{} - {}: {}", context, self.error_type(), self);
            }
            // Not found errors (info level)
            AppError::NotFound(_)
            | AppError::SeriesNotFound(_)
            | AppError::DataSourceNotFound(_) => {
                tracing::info!("{} - {}: {}", context, self.error_type(), self);
            }
            // Other errors
            _ => {
                tracing::error!("{} - {}: {}", context, self.error_type(), self);
            }
        }
    }

    /// Get a human-readable error type for logging
    fn error_type(&self) -> &'static str {
        match self {
            AppError::Database(_) => "DatabaseError",
            AppError::DatabasePool(_) => "DatabasePoolError",
            AppError::HttpClient(_) => "HttpClientError",
            AppError::JsonSerialization(_) => "JsonSerializationError",
            AppError::Validation(_) => "ValidationError",
            AppError::ValidationErrors(_) => "ValidationErrors",
            AppError::ConnectionPool(_) => "ConnectionPoolError",
            AppError::ConfigError(_) => "ConfigError",
            AppError::Io(_) => "IoError",
            AppError::AuthenticationError(_) => "AuthenticationError",
            AppError::InternalError(_) => "InternalError",
            AppError::DatabaseError(_) => "DatabaseError",
            AppError::ValidationError(_) => "ValidationError",
            AppError::SeriesNotFound(_) => "SeriesNotFound",
            AppError::DataSourceNotFound(_) => "DataSourceNotFound",
            AppError::InvalidDateFormat(_) => "InvalidDateFormat",
            AppError::InvalidTransformation(_) => "InvalidTransformation",
            AppError::RateLimitExceeded => "RateLimitExceeded",
            AppError::ExternalApiError(_) => "ExternalApiError",
            AppError::ParserError(_) => "ParserError",
            AppError::MigrationError(_) => "MigrationError",
            AppError::CrawlerError(_) => "CrawlerError",
            AppError::SearchError(_) => "SearchError",
            AppError::PermissionDenied(_) => "PermissionDenied",
            AppError::NotFound(_) => "NotFound",
            AppError::BadRequest(_) => "BadRequest",
            AppError::Unauthorized(_) => "Unauthorized",
            AppError::Forbidden(_) => "Forbidden",
            AppError::Conflict(_) => "Conflict",
            AppError::UnprocessableEntity(_) => "UnprocessableEntity",
            AppError::ServiceUnavailable(_) => "ServiceUnavailable",
        }
    }
}
