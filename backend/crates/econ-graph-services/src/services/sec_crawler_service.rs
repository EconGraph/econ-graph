use anyhow::Result;
use chrono::{DateTime, Utc};
use econ_graph_core::database::DatabasePool;
use econ_graph_sec_crawler::{CrawlConfig, SecEdgarCrawler};
use std::sync::Arc;
use uuid::Uuid;

/// **SEC Crawler Service**
///
/// Service for managing SEC EDGAR crawling operations.
/// Integrates with the existing SEC crawler crate.
pub struct SecCrawlerService {
    pool: Arc<DatabasePool>,
}

impl SecCrawlerService {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }

    /// Trigger SEC crawl for a specific company
    ///
    /// # Arguments
    /// * `cik` - Company CIK (Central Index Key)
    /// * `form_types` - Optional comma-separated list of form types to include
    /// * `start_date` - Optional start date for filing search (YYYY-MM-DD)
    /// * `end_date` - Optional end date for filing search (YYYY-MM-DD)
    /// * `exclude_amended` - Whether to exclude amended filings
    /// * `exclude_restated` - Whether to exclude restated filings
    /// * `max_file_size` - Maximum file size to download in bytes
    ///
    /// # Returns
    /// Result containing crawl operation details
    pub async fn crawl_company(
        &self,
        cik: &str,
        form_types: Option<String>,
        start_date: Option<String>,
        end_date: Option<String>,
        exclude_amended: Option<bool>,
        exclude_restated: Option<bool>,
        max_file_size: Option<i64>,
    ) -> Result<SecCrawlResult> {
        if cik.is_empty() || cik.len() > 10 || !cik.bytes().all(|b| b.is_ascii_digit()) {
            anyhow::bail!("CIK must contain 1 to 10 digits");
        }
        // Parse form types
        let form_types_vec: Vec<String> = form_types
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        // Parse dates
        let start_date_parsed = if let Some(date_str) = start_date {
            Some(chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")?)
        } else {
            None
        };

        let end_date_parsed = if let Some(date_str) = end_date {
            Some(chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")?)
        } else {
            None
        };

        if start_date_parsed
            .zip(end_date_parsed)
            .is_some_and(|(start, end)| start > end)
        {
            anyhow::bail!("startDate must not be after endDate");
        }
        // Create crawl configuration
        let config = CrawlConfig {
            max_requests_per_second: 10, // SEC rate limit
            max_retries: 3,
            retry_delay_seconds: 5,
            max_file_size_bytes: u64::try_from(max_file_size.unwrap_or(52_428_800))
                .ok()
                .filter(|size| *size > 0)
                .ok_or_else(|| anyhow::anyhow!("maxFileSize must be positive"))?, // 50MB default
            start_date: start_date_parsed,
            end_date: end_date_parsed,
            form_types: if form_types_vec.is_empty() {
                None
            } else {
                Some(form_types_vec)
            },
            exclude_amended: exclude_amended.unwrap_or(false),
            exclude_restated: exclude_restated.unwrap_or(false),
            user_agent: CrawlConfig::default().user_agent,
            max_concurrent_requests: Some(3),
        };

        // Create crawler with custom config
        let crawler = SecEdgarCrawler::with_config(self.pool.as_ref().clone(), config).await?;

        // Execute crawl
        let result = crawler.crawl_company_filings(cik).await?;

        // Convert to our result type
        Ok(SecCrawlResult {
            operation_id: result.operation_id,
            cik: cik.to_string(),
            filings_downloaded: result.filings_downloaded as i32,
            // The crawler downloads filings; parsing is a separate operation.
            filings_processed: 0,
            errors: result.errors.len() as i32,
            start_time: result.start_time,
            end_time: result.end_time,
            status: if result.success {
                "completed"
            } else {
                "failed"
            }
            .to_string(),
        })
    }

    /// Import SEC EDGAR RSS feed
    ///
    /// # Arguments
    /// * `rss_url` - Optional RSS feed URL (defaults to SEC EDGAR RSS)
    /// * `max_filings` - Maximum number of filings to import
    /// * `form_types` - Optional comma-separated list of form types to include
    ///
    /// # Returns
    /// Result containing import operation details
    pub async fn import_rss_feed(
        &self,
        _rss_url: Option<String>,
        _max_filings: Option<i32>,
        _form_types: Option<String>,
    ) -> Result<SecRssImportResult> {
        anyhow::bail!("SEC RSS import is not implemented yet")
    }
}

/// Result of SEC crawl operation
#[derive(Debug, Clone)]
pub struct SecCrawlResult {
    pub operation_id: Uuid,
    pub cik: String,
    pub filings_downloaded: i32,
    pub filings_processed: i32,
    pub errors: i32,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub status: String,
}

/// Result of SEC RSS import
#[derive(Debug, Clone)]
pub struct SecRssImportResult {
    pub operation_id: Uuid,
    pub filings_imported: i32,
    pub companies_added: i32,
    pub errors: i32,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub status: String,
}
