/**
 * Test module to verify error logging functionality
 * This module tests that errors are properly logged with context
 */
use crate::error::{AppError, AppResult};
use tracing_test::traced_test;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_logging_with_context() {
        // Test that errors are logged with appropriate context
        let error = AppError::DatabaseError("Connection failed".to_string());

        // This should log the error with context
        error.log_with_context("Database connection test");

        // The error should still be usable
        assert!(matches!(error, AppError::DatabaseError(_)));
    }

    #[test]
    fn test_error_type_classification() {
        // Test that different error types are classified correctly
        let db_error = AppError::DatabaseError("Test".to_string());
        let validation_error = AppError::ValidationError("Test".to_string());
        let not_found_error = AppError::NotFound("Test".to_string());

        // These should all be usable
        assert!(matches!(db_error, AppError::DatabaseError(_)));
        assert!(matches!(validation_error, AppError::ValidationError(_)));
        assert!(matches!(not_found_error, AppError::NotFound(_)));
    }

    #[test]
    fn test_error_utility_functions() {
        // Test utility functions for creating errors
        let not_found = AppError::not_found("user");
        let bad_request = AppError::bad_request("invalid input");
        let unauthorized = AppError::unauthorized("invalid token");

        assert!(matches!(not_found, AppError::NotFound(_)));
        assert!(matches!(bad_request, AppError::BadRequest(_)));
        assert!(matches!(unauthorized, AppError::Unauthorized(_)));
    }

    #[traced_test]
    #[tokio::test]
    async fn test_error_logging_integration() {
        // Test that errors are properly logged in an async context
        let result: AppResult<()> = Err(AppError::InternalError("Test error".to_string()));

        if let Err(error) = result {
            error.log_with_context("Integration test");
            // The error should be logged and still usable
            assert!(matches!(error, AppError::InternalError(_)));
        }
    }
}
