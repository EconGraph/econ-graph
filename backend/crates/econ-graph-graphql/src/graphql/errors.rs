//! # GraphQL error sanitizing
//!
//! Resolvers return service errors with `?`, which turns an [`AppError`] into a GraphQL error
//! carrying its `Display` text. For database and other server-side failures that text is the
//! raw driver message ("value too long for type character varying(255)", "numeric field
//! overflow"), which says nothing useful to the client and shows it our schema (ECO-410).
//! [`SanitizeErrors`] replaces those messages with a generic one at the schema boundary and
//! logs the detail; client errors (validation, not found, unauthorized, ...) keep their message.

use std::sync::Arc;

use async_graphql::extensions::{Extension, ExtensionContext, ExtensionFactory, NextExecute};
use async_graphql::{PathSegment, Response, ServerError};
use econ_graph_core::error::AppError;

/// What the client sees in place of a server-side error's detail.
pub const INTERNAL_ERROR_MESSAGE: &str = "Internal server error";

/// Schema extension that hides server-side error detail from clients (see the module docs).
pub struct SanitizeErrors;

impl ExtensionFactory for SanitizeErrors {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(SanitizeErrorsExtension)
    }
}

struct SanitizeErrorsExtension;

#[async_trait::async_trait]
impl Extension for SanitizeErrorsExtension {
    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        let mut response = next.run(ctx, operation_name).await;
        for error in &mut response.errors {
            sanitize(error, operation_name);
        }
        response
    }
}

/// Whether `error` is a server-side failure whose detail must not reach the client.
fn is_internal(error: &ServerError) -> bool {
    if error.source::<diesel::result::Error>().is_some() {
        return true;
    }
    matches!(
        error.source::<AppError>(),
        Some(
            AppError::Database(_)
                | AppError::DatabasePool(_)
                | AppError::DatabaseError(_)
                | AppError::ConnectionPool(_)
                | AppError::MigrationError(_)
                | AppError::InternalError(_)
        )
    )
}

/// Logs a server-side error with its detail and replaces its message with a generic one.
fn sanitize(error: &mut ServerError, operation_name: Option<&str>) {
    if !is_internal(error) {
        return;
    }
    let path = error
        .path
        .iter()
        .map(|segment| match segment {
            PathSegment::Field(name) => name.clone(),
            PathSegment::Index(index) => index.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".");
    tracing::error!(
        operation = operation_name.unwrap_or(""),
        path = %path,
        "GraphQL request failed: {}",
        error.message
    );
    error.message = INTERNAL_ERROR_MESSAGE.to_string();
    error
        .extensions
        .get_or_insert_with(Default::default)
        .set("code", "INTERNAL_SERVER_ERROR");
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::{EmptyMutation, EmptySubscription, Object, Result, Schema};

    struct Query;

    #[Object]
    impl Query {
        async fn database_failure(&self) -> Result<i32> {
            Err(AppError::DatabaseError(
                "value too long for type character varying(255)".to_string(),
            ))?
        }

        async fn diesel_failure(&self) -> Result<i32> {
            Err(diesel::result::Error::NotFound)?
        }

        async fn validation_failure(&self) -> Result<i32> {
            Err(AppError::ValidationError(
                "Title can't be empty".to_string(),
            ))?
        }

        async fn not_found(&self) -> Result<i32> {
            Err(AppError::NotFound("Series not found".to_string()))?
        }
    }

    async fn first_error(query: &str) -> ServerError {
        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(SanitizeErrors)
            .finish();
        let mut errors = schema.execute(query).await.errors;
        assert_eq!(errors.len(), 1, "{errors:?}");
        errors.remove(0)
    }

    /// A database error reaches the client as a generic message with a code, not the
    /// driver's text.
    #[tokio::test]
    async fn database_errors_are_generic() {
        let error = first_error("{ databaseFailure }").await;
        assert_eq!(error.message, INTERNAL_ERROR_MESSAGE);
        assert!(!error.message.contains("varying"));
        let code = error
            .extensions
            .as_ref()
            .and_then(|e| e.get("code"))
            .cloned();
        assert_eq!(code, Some("INTERNAL_SERVER_ERROR".into()));
        // The rest of the error (where it happened) is kept.
        assert_eq!(error.path.len(), 1);
    }

    /// A Diesel error returned straight from a resolver is hidden too.
    #[tokio::test]
    async fn raw_diesel_errors_are_generic() {
        let error = first_error("{ dieselFailure }").await;
        assert_eq!(error.message, INTERNAL_ERROR_MESSAGE);
    }

    /// Client errors keep their message, which tells the user what to fix.
    #[tokio::test]
    async fn client_errors_keep_their_message() {
        let error = first_error("{ validationFailure }").await;
        assert_eq!(error.message, "Validation error: Title can't be empty");
        let error = first_error("{ notFound }").await;
        assert_eq!(error.message, "Not found: Series not found");
    }
}
