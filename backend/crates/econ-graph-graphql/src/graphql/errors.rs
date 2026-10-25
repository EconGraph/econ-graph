//! # GraphQL error sanitizing
//!
//! Resolvers return service errors with `?`, which turns an [`AppError`] into a GraphQL error
//! carrying its `Display` text. For database and other server-side failures that text is the
//! raw driver message ("value too long for type character varying(255)", "numeric field
//! overflow"), which says nothing useful to the client and shows it our schema (ECO-410).
//! [`SanitizeErrors`] replaces those messages with a generic one at the schema boundary and
//! logs the detail.
//!
//! The check is deny-by-default: an error keeps its message only when it's known to be meant
//! for the client (see [`is_client_facing`]). Anything else carrying a typed source (a Diesel or
//! pool error, an I/O or HTTP failure, an [`AppError`] variant outside the allowlist) is hidden.
//! Hand-written messages (`Error::new("...")`, `Err("...".into())`) are client-facing, so a
//! resolver must not format a server-side error into one: pass the error itself (with `?` or
//! `Error::new_with_source`) and let this extension decide.

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

/// Whether `error`'s message is meant for the client. Everything else is a server-side failure
/// whose detail must not reach it.
fn is_client_facing(error: &ServerError) -> bool {
    // No source: a message the resolver (or async-graphql itself) wrote for the client.
    if error.source.is_none() {
        return true;
    }
    // `Err("...".into())` and `Err(format!(...).into())` keep the text as the source.
    if error.source::<&'static str>().is_some() || error.source::<String>().is_some() {
        return true;
    }
    // A malformed argument (`Uuid::parse_str(&id)?`, a bad date or number) is the client's to fix.
    if error.source::<uuid::Error>().is_some()
        || error.source::<chrono::ParseError>().is_some()
        || error.source::<std::num::ParseIntError>().is_some()
        || error.source::<std::num::ParseFloatError>().is_some()
    {
        return true;
    }
    matches!(
        error.source::<AppError>(),
        Some(
            AppError::Validation(_)
                | AppError::ValidationErrors(_)
                | AppError::ValidationError(_)
                | AppError::AuthenticationError(_)
                | AppError::SeriesNotFound(_)
                | AppError::DataSourceNotFound(_)
                | AppError::InvalidDateFormat(_)
                | AppError::InvalidTransformation(_)
                | AppError::RateLimitExceeded
                | AppError::PermissionDenied(_)
                | AppError::NotFound(_)
                | AppError::BadRequest(_)
                | AppError::Unauthorized(_)
                | AppError::Forbidden(_)
                | AppError::Conflict(_)
                | AppError::UnprocessableEntity(_)
        )
    )
}

/// Logs a server-side error with its detail and replaces its message with a generic one. Its
/// extensions are replaced too, with only the generic `code`, since a resolver may have put
/// server-side detail in them.
fn sanitize(error: &mut ServerError, operation_name: Option<&str>) {
    if is_client_facing(error) {
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
    let mut extensions = async_graphql::ErrorExtensionValues::default();
    extensions.set("code", "INTERNAL_SERVER_ERROR");
    error.extensions = Some(extensions);
}

/// Collects log output for tests: everything logged on this thread while the returned guard
/// is alive lands in the returned buffer.
#[cfg(test)]
pub(crate) fn capture_logs() -> (
    tracing::subscriber::DefaultGuard,
    Arc<std::sync::Mutex<Vec<u8>>>,
) {
    let buffer = Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = buffer.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || LogWriter(writer.clone()))
        .finish();
    (tracing::subscriber::set_default(subscriber), buffer)
}

#[cfg(test)]
struct LogWriter(Arc<std::sync::Mutex<Vec<u8>>>);

#[cfg(test)]
impl std::io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::{
        EmptyMutation, EmptySubscription, ErrorExtensions, Object, Result, Schema,
    };

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

        /// An `AppError` variant that isn't on the client-facing list.
        async fn unlisted_failure(&self, variant: String) -> Result<i32> {
            let detail = "relation \"secret_table\" does not exist".to_string();
            Err(match variant.as_str() {
                "ExternalApiError" => AppError::ExternalApiError(detail),
                "SearchError" => AppError::SearchError(detail),
                "CrawlerError" => AppError::CrawlerError(detail),
                "ConfigError" => AppError::ConfigError(detail),
                "Io" => AppError::Io(std::io::Error::other(detail)),
                "ServiceUnavailable" => AppError::ServiceUnavailable(detail),
                other => panic!("unknown variant {other}"),
            })?
        }

        /// A typed error that isn't an `AppError` at all.
        async fn io_failure(&self) -> Result<i32> {
            Err(std::io::Error::other("/srv/secret/path: permission denied"))?
        }

        /// A server-side error passed with `new_with_source` keeps its source, so it's hidden.
        async fn with_source(&self) -> Result<i32> {
            Err(async_graphql::Error::new_with_source(
                AppError::DatabaseError("pg detail".to_string()),
            ))
        }

        /// A server-side error carrying detail in an extension.
        async fn with_source_and_detail(&self) -> Result<i32> {
            Err(
                async_graphql::Error::new_with_source(AppError::DatabaseError(
                    "pg detail".to_string(),
                ))
                .extend_with(|_, extensions| extensions.set("detail", "relation secret_table")),
            )
        }

        /// A client error carrying an extension of its own.
        async fn hand_written_with_extension(&self) -> Result<i32> {
            Err(async_graphql::Error::new("Too many requests")
                .extend_with(|_, extensions| extensions.set("retryAfter", 30)))
        }

        async fn hand_written(&self) -> Result<i32> {
            Err(async_graphql::Error::new("Authentication required"))
        }

        async fn string_into(&self) -> Result<i32> {
            Err("seriesIds need a source".into())
        }

        async fn bad_uuid(&self) -> Result<i32> {
            let id = uuid::Uuid::parse_str("not-a-uuid")?;
            Ok(id.as_bytes()[0].into())
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

    /// Server-side `AppError` variants outside the allowlist are hidden too, not just the
    /// database ones: the sanitizer is deny-by-default.
    #[tokio::test]
    async fn unlisted_app_errors_are_generic() {
        for variant in [
            "ExternalApiError",
            "SearchError",
            "CrawlerError",
            "ConfigError",
            "Io",
            "ServiceUnavailable",
        ] {
            let query = format!(r#"{{ unlistedFailure(variant: "{variant}") }}"#);
            let error = first_error(&query).await;
            assert_eq!(error.message, INTERNAL_ERROR_MESSAGE, "{variant}");
        }
        let error = first_error("{ ioFailure }").await;
        assert_eq!(error.message, INTERNAL_ERROR_MESSAGE);
        let error = first_error("{ withSource }").await;
        assert_eq!(error.message, INTERNAL_ERROR_MESSAGE);
    }

    /// A hidden error's extensions are replaced with only the generic code, so server-side
    /// detail a resolver put there doesn't reach the client. Client errors keep theirs.
    #[tokio::test]
    async fn hidden_errors_drop_their_extensions() {
        let error = first_error("{ withSourceAndDetail }").await;
        assert_eq!(error.message, INTERNAL_ERROR_MESSAGE);
        let extensions = error.extensions.expect("code extension");
        assert_eq!(extensions.get("detail"), None);
        assert_eq!(
            extensions.get("code"),
            Some(&"INTERNAL_SERVER_ERROR".into())
        );
        let json = serde_json::to_value(&extensions).unwrap();
        assert_eq!(json, serde_json::json!({ "code": "INTERNAL_SERVER_ERROR" }));

        let error = first_error("{ handWrittenWithExtension }").await;
        assert_eq!(error.message, "Too many requests");
        assert_eq!(
            error.extensions.and_then(|e| e.get("retryAfter").cloned()),
            Some(30.into())
        );
    }

    /// Hand-written messages and malformed-argument errors still reach the client.
    #[tokio::test]
    async fn hand_written_and_argument_errors_keep_their_message() {
        assert_eq!(
            first_error("{ handWritten }").await.message,
            "Authentication required"
        );
        assert_eq!(
            first_error("{ stringInto }").await.message,
            "seriesIds need a source"
        );
        assert!(first_error("{ badUuid }")
            .await
            .message
            .contains("invalid character"));
    }

    /// The detail a client doesn't see is logged, with where it happened.
    #[tokio::test]
    async fn hidden_detail_is_logged() {
        let (_guard, logs) = capture_logs();
        let error = first_error(r#"{ unlistedFailure(variant: "SearchError") }"#).await;
        assert_eq!(error.message, INTERNAL_ERROR_MESSAGE);
        let logs = String::from_utf8(logs.lock().unwrap().clone()).unwrap();
        assert!(logs.contains("secret_table"), "{logs}");
        assert!(logs.contains("unlistedFailure"), "{logs}");
    }
}
