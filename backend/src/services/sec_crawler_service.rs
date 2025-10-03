use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
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

        // Create crawl configuration
        let config = CrawlConfig {
            max_requests_per_second: 10, // SEC rate limit
            max_retries: 3,
            retry_delay_seconds: 5,
            max_file_size_bytes: max_file_size.unwrap_or(52_428_800), // 50MB default
            start_date: start_date_parsed,
            end_date: end_date_parsed,
            form_types: if form_types_vec.is_empty() {
                None
            } else {
                Some(form_types_vec)
            },
            exclude_amended: exclude_amended.unwrap_or(false),
            exclude_restated: exclude_restated.unwrap_or(false),
            user_agent: "EconGraph-SEC-Crawler/1.0".to_string(),
            max_concurrent_requests: Some(3),
        };

        // Create crawler with custom config
        let crawler = SecEdgarCrawler::with_config(self.pool.clone(), config).await?;

        // Execute crawl
        let result = crawler.crawl_company_filings(cik).await?;

        // Convert to our result type
        Ok(SecCrawlResult {
            operation_id: result.operation_id,
            cik: cik.to_string(),
            filings_downloaded: result.filings_downloaded as i32,
            filings_processed: result.filings_processed as i32,
            errors: result.errors as i32,
            start_time: result.start_time,
            end_time: result.end_time,
            status: result.status,
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
        rss_url: Option<String>,
        max_filings: Option<i32>,
        form_types: Option<String>,
    ) -> Result<SecRssImportResult> {
        // TODO: Implement RSS feed import functionality
        // This would involve:
        // 1. Fetching the RSS feed from SEC EDGAR
        // 2. Parsing the RSS entries
        // 3. Extracting company information and filing metadata
        // 4. Storing companies and filing information in the database
        
        let operation_id = Uuid::new_v4();
        let start_time = Utc::now();

        // For now, return a mock result
        Ok(SecRssImportResult {
            operation_id: operation_id.into(),
            filings_imported: 0,
            companies_added: 0,
            errors: 0,
            start_time,
            end_time: None,
            status: "pending".to_string(),
        })
    }

    /// Get crawl operation status
    pub async fn get_crawl_status(&self, operation_id: Uuid) -> Result<Option<SecCrawlResult>> {
        // TODO: Implement status tracking for crawl operations
        // This would involve storing operation status in the database
        // and providing real-time updates
        
        Ok(None)
    }

    /// Get recent crawl operations
    pub async fn get_recent_crawls(&self, limit: Option<i32>) -> Result<Vec<SecCrawlResult>> {
        // TODO: Implement retrieval of recent crawl operations
        // This would query the database for recent crawl operations
        
        Ok(vec![])
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

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::database::create_pool;

    #[tokio::test]
    async fn test_sec_crawler_service_creation() {
        // Test service creation
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:password@localhost/econ_graph_test".to_string());
        
        let pool = create_pool(&database_url).await.unwrap();
        let service = SecCrawlerService::new(Arc::new(pool));
        
        // Test basic functionality
        let result = service.crawl_company("0000320193", None, None, None, None, None, None).await;
        // This will fail without proper SEC crawler setup, but tests the interface
        assert!(result.is_ok() || result.is_err());
    }
}
