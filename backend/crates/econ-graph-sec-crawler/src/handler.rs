//! `crawl_queue` integration: the [`SecFilingHandler`] the worker runs for `(SEC, fetch_filing)`
//! jobs, and [`enqueue_filings`] to create those jobs.
//!
//! A job's `series_id` is a company CIK (zero-padded or not, optional `CIK` prefix). Processing
//! a job runs [`SecEdgarCrawler::crawl_company`] through the worker's shared
//! [`HttpFetcher`](econ_graph_crawler::HttpFetcher): submissions -> `companies` upsert -> XBRL
//! instance download + storage for every matching filing not stored yet.
//!
//! Re-running a company is idempotent: the company row is upserted by CIK and filings already in
//! `financial_statements` for that company are skipped without being downloaded.

use async_trait::async_trait;
use econ_graph_core::models::{CrawlQueueItem, JobKind, NewCrawlQueueItem};
use econ_graph_core::{AppResult, DatabasePool};
use econ_graph_crawler::{CrawlCtx, CrawlError, JobHandler, JobOutcome, JobStats, SourceId};

use crate::crawler::{normalize_cik, SecEdgarCrawler, SecEndpoints};
use crate::models::CrawlConfig;

/// Form types a queued company crawl stores by default.
pub const DEFAULT_FORM_TYPES: &[&str] = &["10-K", "10-Q"];

/// [`JobHandler`] for `(SourceId::Sec, JobKind::FetchFiling)`.
///
/// Result mapping:
/// - bad CIK -> `Permanent`; submissions 404 -> `NotFound`; 429 -> `RateLimited`; 5xx/timeouts
///   and database errors -> `Transient` (the worker's usual transition table applies);
/// - a retryable error while downloading one filing stops the company and is returned (filings
///   stored so far are kept; the retry skips them);
/// - non-retryable per-filing errors (e.g. an instance document that 404s) are logged and the
///   job still completes.
///
/// [`JobStats`] on success: `points_written` = `new_points` = filings newly stored,
/// `latest_date` = latest filing date among them, `series_id` = `None`.
#[derive(Debug, Clone)]
pub struct SecFilingHandler {
    config: CrawlConfig,
    endpoints: SecEndpoints,
}

impl SecFilingHandler {
    /// Handler for the real EDGAR hosts with [`default_config`](Self::default_config).
    pub fn new() -> Self {
        Self::with_endpoints(Self::default_config(), SecEndpoints::default())
    }

    /// Handler with explicit filters and hosts (tests point `endpoints` at a mock server).
    pub fn with_endpoints(config: CrawlConfig, endpoints: SecEndpoints) -> Self {
        Self { config, endpoints }
    }

    /// [`CrawlConfig::default`] restricted to [`DEFAULT_FORM_TYPES`]. The rate limit and
    /// user agent in it are unused: requests go through the worker's `HttpFetcher`.
    pub fn default_config() -> CrawlConfig {
        CrawlConfig {
            form_types: Some(DEFAULT_FORM_TYPES.iter().map(|s| s.to_string()).collect()),
            ..CrawlConfig::default()
        }
    }
}

impl Default for SecFilingHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl JobHandler for SecFilingHandler {
    async fn handle(
        &self,
        ctx: &CrawlCtx,
        item: &CrawlQueueItem,
    ) -> Result<JobOutcome, CrawlError> {
        let cik = normalize_cik(&item.series_id)?;
        let crawler = SecEdgarCrawler::with_fetcher(
            ctx.pool.clone(),
            self.config.clone(),
            ctx.http.clone(),
            self.endpoints.clone(),
        );
        let crawl = crawler.crawl_company(&cik).await?;
        if let Some(err) = crawl.retryable_error {
            return Err(err);
        }
        let stored = crawl.result.filings_downloaded as usize;
        Ok(JobOutcome::Completed(JobStats {
            series_id: None,
            points_written: stored,
            new_points: stored,
            latest_date: crawl.latest_filing_date,
            metadata_written: 0,
        }))
    }
}

/// Enqueues one `(SEC, fetch_filing)` job per CIK (normalized to 10 digits).
///
/// Returns `(enqueued, already_active)`: a CIK with a pending/processing/retrying job is not
/// enqueued again. Invalid CIKs, or a `priority` outside 1-10, fail the whole call before
/// anything is inserted.
pub async fn enqueue_filings(
    pool: &DatabasePool,
    ciks: &[String],
    priority: i32,
) -> AppResult<(usize, usize)> {
    if !(1..=10).contains(&priority) {
        return Err(econ_graph_core::AppError::ValidationError(format!(
            "SEC: queue priority must be 1-10, got {priority}"
        )));
    }
    let normalized = ciks
        .iter()
        .map(|c| normalize_cik(c))
        .collect::<Result<Vec<_>, _>>()?;
    let (mut enqueued, mut skipped) = (0, 0);
    for cik in normalized {
        let item = NewCrawlQueueItem {
            source: SourceId::Sec.as_str().to_string(),
            series_id: cik,
            priority,
            kind: JobKind::FetchFiling.as_str().to_string(),
            ..NewCrawlQueueItem::default()
        };
        match CrawlQueueItem::enqueue(pool, &item).await? {
            Some(_) => enqueued += 1,
            None => skipped += 1,
        }
    }
    Ok((enqueued, skipped))
}

#[cfg(test)]
mod tests;
