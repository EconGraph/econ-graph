// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Crawl error taxonomy. The variant decides what the worker does with a failed job.

use std::time::Duration;

use econ_graph_core::error::AppError;
use reqwest::StatusCode;

/// Why a crawl operation failed.
///
/// Messages must never contain API keys; build them from redacted URLs only.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CrawlError {
    /// The source throttled us (HTTP 429, or 503 with `Retry-After`).
    /// Retry after `retry_after` if given; does not count as a failed attempt.
    #[error("rate limited{}", .retry_after.map(|d| format!(" (retry after {}s)", d.as_secs())).unwrap_or_default())]
    RateLimited {
        /// Server-provided `Retry-After`, if any.
        retry_after: Option<Duration>,
    },
    /// Timeout, connection failure or 5xx. Retry with backoff.
    #[error("transient error: {0}")]
    Transient(String),
    /// HTTP 404, or the series does not exist at the source. Not retried.
    #[error("not found: {0}")]
    NotFound(String),
    /// HTTP 401/403, or a missing/invalid API key. Not retried.
    #[error("authentication failed: {0}")]
    Auth(String),
    /// The response body did not have the expected shape. Not retried.
    #[error("parse error: {0}")]
    Parse(String),
    /// Any other 4xx, or a logic/configuration error. Not retried.
    #[error("permanent error: {0}")]
    Permanent(String),
    /// Local work contention: another crawl in this process is already doing the same shared
    /// work (for example, downloading a source's one shared file). Not an upstream rate limit,
    /// so it doesn't trip the per-source breaker; like `RateLimited`, it doesn't count as a
    /// failed attempt. Retry after `retry_after`, an adapter-computed backoff with jitter.
    #[error("busy: {message} (retry after {}s)", retry_after.as_secs())]
    Busy {
        /// Adapter-computed delay before retrying, already including jitter.
        retry_after: Duration,
        /// What is contended, e.g. the shared URL.
        message: String,
    },
}

impl CrawlError {
    /// Classifies a non-success HTTP status.
    ///
    /// 429 -> `RateLimited`; 503 with `retry_after` -> `RateLimited`; other 5xx and 408 -> `Transient`;
    /// 404/410 -> `NotFound`; 401/403 -> `Auth`; everything else -> `Permanent`.
    /// `context` should identify the request (source + redacted URL) and is used as the message.
    pub fn from_status(
        status: StatusCode,
        retry_after: Option<Duration>,
        context: impl Into<String>,
    ) -> Self {
        let msg = format!("HTTP {}: {}", status.as_u16(), context.into());
        match status {
            StatusCode::TOO_MANY_REQUESTS => CrawlError::RateLimited { retry_after },
            StatusCode::SERVICE_UNAVAILABLE if retry_after.is_some() => {
                CrawlError::RateLimited { retry_after }
            }
            StatusCode::REQUEST_TIMEOUT => CrawlError::Transient(msg),
            s if s.is_server_error() => CrawlError::Transient(msg),
            StatusCode::NOT_FOUND | StatusCode::GONE => CrawlError::NotFound(msg),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => CrawlError::Auth(msg),
            _ => CrawlError::Permanent(msg),
        }
    }

    /// True for `RateLimited`, `Transient` and `Busy`: the same request may succeed later.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CrawlError::RateLimited { .. } | CrawlError::Transient(_) | CrawlError::Busy { .. }
        )
    }

    /// The server-requested delay, if this is `RateLimited` with a `Retry-After`.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            CrawlError::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }

    /// Stable, low-cardinality label for metrics and logs.
    pub fn kind(&self) -> &'static str {
        match self {
            CrawlError::RateLimited { .. } => "rate_limited",
            CrawlError::Transient(_) => "transient",
            CrawlError::NotFound(_) => "not_found",
            CrawlError::Auth(_) => "auth",
            CrawlError::Parse(_) => "parse",
            CrawlError::Permanent(_) => "permanent",
            CrawlError::Busy { .. } => "busy",
        }
    }
}

impl From<CrawlError> for AppError {
    /// `RateLimited` -> `RateLimitExceeded`, `Transient` -> `ServiceUnavailable`,
    /// `NotFound` -> `NotFound`, `Parse` -> `ParserError`, `Auth`/`Permanent` -> `ExternalApiError`.
    ///
    /// `Auth` deliberately does not map to `Unauthorized`/`AuthenticationError`: those describe
    /// *our* caller's credentials, whereas this is the crawler's credential to an upstream API.
    fn from(err: CrawlError) -> Self {
        match err {
            CrawlError::RateLimited { .. } => AppError::RateLimitExceeded,
            CrawlError::Transient(m) => AppError::ServiceUnavailable(m),
            CrawlError::NotFound(m) => AppError::NotFound(m),
            CrawlError::Parse(m) => AppError::ParserError(m),
            CrawlError::Busy { message, .. } => AppError::ServiceUnavailable(message),
            e @ (CrawlError::Auth(_) | CrawlError::Permanent(_)) => {
                AppError::ExternalApiError(e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<CrawlError> {
        vec![
            CrawlError::RateLimited {
                retry_after: Some(Duration::from_secs(5)),
            },
            CrawlError::Transient("t".into()),
            CrawlError::NotFound("n".into()),
            CrawlError::Auth("a".into()),
            CrawlError::Parse("p".into()),
            CrawlError::Permanent("x".into()),
            CrawlError::Busy {
                retry_after: Duration::from_millis(500),
                message: "b".into(),
            },
        ]
    }

    #[test]
    fn retryability() {
        let retryable: Vec<bool> = all().iter().map(CrawlError::is_retryable).collect();
        assert_eq!(retryable, [true, true, false, false, false, false, true]);
        assert!(CrawlError::RateLimited { retry_after: None }.is_retryable());
    }

    #[test]
    fn retry_after_only_for_rate_limited() {
        let e = all();
        assert_eq!(e[0].retry_after(), Some(Duration::from_secs(5)));
        // `Busy` carries its own delay outside `retry_after`, which is specifically the
        // server-requested `Retry-After`.
        assert!(e[1..].iter().all(|e| e.retry_after().is_none()));
        assert_eq!(
            CrawlError::RateLimited { retry_after: None }.retry_after(),
            None
        );
    }

    #[test]
    fn kinds_are_distinct_labels() {
        let kinds: Vec<_> = all().iter().map(CrawlError::kind).collect();
        assert_eq!(
            kinds,
            [
                "rate_limited",
                "transient",
                "not_found",
                "auth",
                "parse",
                "permanent",
                "busy",
            ]
        );
    }

    #[test]
    fn status_classification() {
        let secs = Some(Duration::from_secs(7));
        let c =
            |code: u16, ra| CrawlError::from_status(StatusCode::from_u16(code).unwrap(), ra, "u");
        assert_eq!(c(429, None), CrawlError::RateLimited { retry_after: None });
        assert_eq!(c(429, secs), CrawlError::RateLimited { retry_after: secs });
        assert_eq!(c(503, secs), CrawlError::RateLimited { retry_after: secs });
        assert_eq!(c(503, None).kind(), "transient");
        assert_eq!(c(500, None).kind(), "transient");
        assert_eq!(c(502, None).kind(), "transient");
        assert_eq!(c(408, None).kind(), "transient");
        assert_eq!(c(404, None).kind(), "not_found");
        assert_eq!(c(410, None).kind(), "not_found");
        assert_eq!(c(401, None).kind(), "auth");
        assert_eq!(c(403, None).kind(), "auth");
        assert_eq!(c(400, None).kind(), "permanent");
        assert_eq!(c(422, None).kind(), "permanent");
        assert_eq!(c(404, None).to_string(), "not found: HTTP 404: u");
    }

    #[test]
    fn app_error_mapping() {
        let m: Vec<AppError> = all().into_iter().map(AppError::from).collect();
        assert!(matches!(m[0], AppError::RateLimitExceeded));
        assert!(matches!(&m[1], AppError::ServiceUnavailable(s) if s == "t"));
        assert!(matches!(&m[2], AppError::NotFound(s) if s == "n"));
        assert!(matches!(&m[3], AppError::ExternalApiError(s) if s.contains("authentication")));
        assert!(matches!(&m[4], AppError::ParserError(s) if s == "p"));
        assert!(matches!(&m[5], AppError::ExternalApiError(_)));
        assert!(matches!(&m[6], AppError::ServiceUnavailable(s) if s == "b"));
    }
}
