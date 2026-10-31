//! Operator reads of the persistent worker queue, with bounded pages and no process controls.
use crate::imports::*;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::schema::{crawl_queue, data_sources};
use std::sync::LazyLock;

/// One stored queue job, including its most recent execution outcome.
#[derive(SimpleObject)]
pub struct CrawlerJob {
    pub id: ID,
    pub source: String,
    pub series_id: String,
    pub kind: String,
    pub status: String,
    pub priority: i32,
    pub retry_count: i32,
    pub max_retries: i32,
    pub scheduled_for: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    /// Worker-recorded error with URLs and common credential assignments redacted.
    pub error_message: Option<String>,
}

/// Scheduler configuration for an adapter present in this server build.
#[derive(SimpleObject)]
pub struct CrawlerSource {
    pub source: String,
    pub name: String,
    /// Null until the crawler has created this source's stored configuration.
    pub enabled: Option<bool>,
    pub crawl_frequency_hours: Option<i32>,
    pub requires_admin_approval: Option<bool>,
}

pub(super) fn adapter(source: &str) -> Result<econ_graph_crawler::SourceId> {
    let parsed = source
        .trim()
        .parse::<econ_graph_crawler::SourceId>()
        .map_err(|_| super::errors::bad_user_input("Unknown crawler source"))?;
    if econ_graph_crawler::sources::default_registry()
        .get(parsed)
        .is_none()
    {
        return Err(super::errors::bad_user_input(
            "Source has no adapter in this build",
        ));
    }
    Ok(parsed)
}

fn redact_error(message: &str) -> String {
    // No request URL, query string, headers or API keys are needed to identify
    // a queue failure. CrawlError already requires redacted messages; this also covers old rows.
    static URLS: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r#"https?://[^\s<>"']+"#).unwrap());
    // Authorization credentials can contain a scheme plus token68 or a comma-separated
    // auth-param list. Redact the complete value while retaining later diagnostic text.
    static AUTHORIZATION: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r#"(?i)(["']?authorization["']?\s*[:=]\s*)(?:"(?:[^"\\]|\\.)*"|'[^']*'|[a-z][a-z0-9_-]*[ \t]+(?:[a-z][a-z0-9_-]*[ \t]*=[ \t]*(?:"(?:[^"\\]|\\.)*"|[^\s,;]+)(?:[ \t]*,[ \t]*[a-z][a-z0-9_-]*[ \t]*=[ \t]*(?:"(?:[^"\\]|\\.)*"|[^\s,;]+))*|[^\s,;]+)|[^\s,;]+)"#,
        )
        .unwrap()
    });
    static ASSIGNMENTS: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
        r#"(?i)(["']?(?:api[_-]?key|(?:access[_-]?)?token|password|secret)["']?\s*[:=]\s*)(?:"[^"]*"|'[^']*'|Bearer\s+[^\s,;]+|[^\s,;]+)"#,
    )
    .unwrap()
    });
    let text = URLS.replace_all(message, "[upstream URL redacted]");
    let text = AUTHORIZATION.replace_all(&text, "$1[redacted]");
    ASSIGNMENTS
        .replace_all(&text, "$1[redacted]")
        .chars()
        .take(2000)
        .collect()
}

pub(super) async fn jobs(
    ctx: &Context<'_>,
    limit: Option<i32>,
    offset: Option<i32>,
    source: Option<String>,
    status: Option<String>,
) -> Result<Vec<CrawlerJob>> {
    require_role(ctx, Role::AdminSystemRead)?;
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    if !(1..=200).contains(&limit) || !(0..=100_000).contains(&offset) {
        return Err(super::errors::bad_user_input(
            "limit must be 1..200 and offset 0..100000",
        ));
    }
    let mut query = crawl_queue::table.into_boxed();
    if let Some(source) = source {
        // History may contain adapters removed from this build; exact stored source labels
        // remain searchable without requiring an available adapter.
        if source.is_empty() || source.len() > 64 {
            return Err(super::errors::bad_user_input("Invalid source filter"));
        }
        query = query.filter(crawl_queue::source.eq(source));
    }
    if let Some(status) = status {
        if ![
            "pending",
            "processing",
            "retrying",
            "completed",
            "failed",
            "cancelled",
        ]
        .contains(&status.as_str())
        {
            return Err(super::errors::bad_user_input("Invalid queue status"));
        }
        query = query.filter(crawl_queue::status.eq(status));
    }
    let mut conn = data_pool(ctx)?.get().await?;
    let rows = query
        .order((crawl_queue::updated_at.desc(), crawl_queue::id.desc()))
        .limit(i64::from(limit))
        .offset(i64::from(offset))
        .load::<core_models::CrawlQueueItem>(&mut conn)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| CrawlerJob {
            id: row.id.into(),
            source: row.source,
            series_id: row.series_id,
            kind: row.kind,
            status: row.status,
            priority: row.priority,
            retry_count: row.retry_count,
            max_retries: row.max_retries,
            scheduled_for: row.scheduled_for,
            started_at: row.started_at,
            finished_at: row.finished_at,
            updated_at: row.updated_at,
            error_message: row.error_message.as_deref().map(redact_error),
        })
        .collect())
}

pub(super) async fn sources(ctx: &Context<'_>) -> Result<Vec<CrawlerSource>> {
    require_role(ctx, Role::AdminSystemRead)?;
    let mut conn = data_pool(ctx)?.get().await?;
    let rows = data_sources::table.load::<DataSource>(&mut conn).await?;
    Ok(econ_graph_crawler::sources::default_registry()
        .ids()
        .into_iter()
        .map(|source| {
            let name = econ_graph_crawler::persist::data_source_template(source).name;
            let stored = rows.iter().find(|row| row.name == name);
            CrawlerSource {
                source: source.as_str().into(),
                name,
                enabled: stored.map(|row| row.is_enabled),
                crawl_frequency_hours: stored.map(|row| row.crawl_frequency_hours),
                requires_admin_approval: stored.map(|row| row.requires_admin_approval),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authorization_redaction_removes_scheme_and_full_credentials() {
        for header in [
            "Basic dXNlcjpwYXNz",
            "Bearer opaque-token",
            "Negotiate Z29vZC1ieWU=",
            r#"Digest username="operator", realm="private", nonce="sensitive""#,
        ] {
            let original = format!("HTTP auth failed: Authorization: {header}; retry exhausted");
            assert_eq!(
                redact_error(&original),
                "HTTP auth failed: Authorization: [redacted]; retry exhausted"
            );
        }
        assert_eq!(
            redact_error("HTTP auth failed: authorization=Basic dXNlcjpwYXNz next retry deferred"),
            "HTTP auth failed: authorization=[redacted] next retry deferred"
        );
        assert_eq!(
            redact_error("HTTP auth failed; retry exhausted"),
            "HTTP auth failed; retry exhausted"
        );
    }

    #[test]
    fn stored_error_redaction_keeps_diagnostic_category_without_credentials() {
        for original in [
            "transient error: https://user:password@host/path?api_key=abc token=xyz",
            r#"parse error: {"password": "super secret", "api_key":"abc xyz"}"#,
            "authentication failed: Authorization: Bearer abc",
        ] {
            let text = redact_error(original);
            for secret in ["password@", "abc", "xyz", "super secret", "Bearer"] {
                assert!(!text.contains(secret), "{text}");
            }
        }
    }
}
