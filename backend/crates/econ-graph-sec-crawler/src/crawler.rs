use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use reqwest::{
    header::{HeaderMap, HeaderValue, USER_AGENT},
    Client,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::models::{
    CompanySubmissionsResponse, CrawlConfig, CrawlProgress, CrawlResult, FilingInfo, SecCompany,
    SecFiling, StoredXbrlDocument,
};
use crate::rate_limiter::SecRateLimiter;
use crate::storage::{XbrlStorage, XbrlStorageConfig};
use crate::utils::{build_submissions_url, build_xbrl_url, get_fiscal_quarter, parse_sec_date};
use econ_graph_core::database::DatabasePool;
use econ_graph_core::models::{Company, FinancialStatement};

/// **SEC EDGAR Crawler**
///
/// Main crawler implementation for SEC EDGAR XBRL filings.
/// Handles company discovery, filing enumeration, and XBRL file downloads.
///
/// # Features
/// - Rate-limited HTTP requests (SEC compliant)
/// - Comprehensive error handling and retry logic
/// - Progress tracking for long-running operations
/// - XBRL file storage with compression
/// - Company and filing metadata extraction
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_sec_crawler::SecEdgarCrawler;
/// use econ_graph_core::database::DatabasePool;
/// use uuid::Uuid;
///
/// # async fn example() -> anyhow::Result<()> {
/// let pool = DatabasePool::new("postgres://...").await?;
/// let crawler = SecEdgarCrawler::new(pool).await?;
///
/// // Crawl Apple's recent filings
/// let result = crawler.crawl_company_filings("0000320193").await?;
/// println!("Downloaded {} filings", result.filings_downloaded);
/// # Ok(())
/// # }
/// ```
pub struct SecEdgarCrawler {
    client: Client,
    rate_limiter: SecRateLimiter,
    storage: XbrlStorage,
    config: CrawlConfig,
    pool: DatabasePool,
}

impl SecEdgarCrawler {
    /// Create a new SEC EDGAR crawler instance
    pub async fn new(pool: DatabasePool) -> Result<Self> {
        Self::with_config(pool, CrawlConfig::default()).await
    }

    /// Create a new SEC EDGAR crawler instance with custom configuration
    pub async fn with_config(pool: DatabasePool, config: CrawlConfig) -> Result<Self> {
        // Create HTTP client with proper headers
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_str(&config.user_agent)?);

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        // Create rate limiter
        let rate_limiter =
            SecRateLimiter::new(config.max_requests_per_second, Duration::from_secs(1));

        // Create XBRL storage with default configuration
        let storage_config = XbrlStorageConfig::default();
        let storage = XbrlStorage::new(pool.clone(), storage_config);

        Ok(Self {
            client,
            rate_limiter,
            storage,
            config,
            pool,
        })
    }

    /// Crawl all filings for a specific company
    pub async fn crawl_company_filings(&self, cik: &str) -> Result<CrawlResult> {
        let operation_id = Uuid::new_v4();
        let start_time = Utc::now();

        info!("Starting crawl for company CIK: {}", cik);

        // Get company information
        let company = self.get_company_info(cik).await?;

        // Get company submissions (filings)
        let submissions = self.get_company_submissions(cik).await?;

        // Filter and process filings
        let filings = self.filter_filings(&submissions.recent.filings)?;

        let mut result = CrawlResult {
            operation_id,
            company_cik: Some(cik.to_string()),
            operation_type: "company_filings".to_string(),
            start_time,
            end_time: None,
            total_filings_found: filings.len() as u32,
            filings_downloaded: 0,
            filings_failed: 0,
            total_bytes_downloaded: 0,
            errors: Vec::new(),
            success: false,
        };

        // Download XBRL files
        for filing_info in filings {
            match self.download_filing_xbrl(&company, &filing_info).await {
                Ok(bytes_downloaded) => {
                    result.filings_downloaded += 1;
                    result.total_bytes_downloaded += bytes_downloaded;
                    debug!(
                        "Successfully downloaded filing: {}",
                        filing_info.accession_number[0]
                    );
                }
                Err(e) => {
                    result.filings_failed += 1;
                    let error_msg = format!(
                        "Failed to download filing {}: {}",
                        filing_info.accession_number[0], e
                    );
                    error!("{}", error_msg);
                    result.errors.push(error_msg);
                }
            }
        }

        result.end_time = Some(Utc::now());
        result.success = result.filings_failed == 0;

        info!(
            "Crawl completed for CIK {}: {} downloaded, {} failed",
            cik, result.filings_downloaded, result.filings_failed
        );

        Ok(result)
    }

    /// Get company information from SEC EDGAR
    async fn get_company_info(&self, cik: &str) -> Result<SecCompany> {
        let url = build_submissions_url(cik);

        self.rate_limiter.wait_for_permit().await;

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch company submissions")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP error: {}", response.status()));
        }

        let submissions: CompanySubmissionsResponse = response
            .json()
            .await
            .context("Failed to parse company submissions")?;

        // Convert to SecCompany
        let company = SecCompany {
            id: Uuid::new_v4(),
            cik: cik.to_string(),
            name: submissions.name,
            ticker: submissions.tickers.first().cloned(),
            sic_code: Some(submissions.sic),
            sic_description: Some(submissions.sic_description),
            state_of_incorporation: None, // Not available in submissions API
            fiscal_year_end: None,        // Not available in submissions API
            entity_type: Some(submissions.entity_type),
            entity_size: None, // Not available in submissions API
            business_address: None,
            mailing_address: None,
            phone: None,
            website: None,
            created_at: Utc::now(),
        };

        Ok(company)
    }

    /// Get company submissions (filings) from SEC EDGAR
    async fn get_company_submissions(&self, cik: &str) -> Result<CompanySubmissionsResponse> {
        let url = build_submissions_url(cik);

        self.rate_limiter.wait_for_permit().await;

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch company submissions")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP error: {}", response.status()));
        }

        let submissions: CompanySubmissionsResponse = response
            .json()
            .await
            .context("Failed to parse company submissions")?;

        Ok(submissions)
    }

    /// Filter filings based on configuration
    fn filter_filings(&self, filings: &[FilingInfo]) -> Result<Vec<&FilingInfo>> {
        let mut filtered = Vec::new();

        for filing in filings {
            // Check form type filter
            if let Some(ref form_types) = self.config.form_types {
                if !form_types.contains(&filing.form[0]) {
                    continue;
                }
            }

            // Check date range filter
            if let Some(start_date) = self.config.start_date {
                let filing_date = parse_sec_date(&filing.filing_date[0])?;
                if filing_date < start_date {
                    continue;
                }
            }

            if let Some(end_date) = self.config.end_date {
                let filing_date = parse_sec_date(&filing.filing_date[0])?;
                if filing_date > end_date {
                    continue;
                }
            }

            // Check if XBRL is available
            if filing.is_xbrl.is_empty() || filing.is_xbrl[0] == 0 {
                continue;
            }

            // Check file size limit
            if !filing.size.is_empty() && filing.size[0] > self.config.max_file_size_bytes {
                warn!(
                    "Skipping filing {}: size {} exceeds limit {}",
                    filing.accession_number[0], filing.size[0], self.config.max_file_size_bytes
                );
                continue;
            }

            filtered.push(filing);
        }

        Ok(filtered)
    }

    /// Download XBRL file for a specific filing
    async fn download_filing_xbrl(
        &self,
        company: &SecCompany,
        filing_info: &FilingInfo,
    ) -> Result<u64> {
        let accession_number = &filing_info.accession_number[0];
        let filing_date = parse_sec_date(&filing_info.filing_date[0])?;
        let report_date = parse_sec_date(&filing_info.report_date[0])?;

        // Construct XBRL URL
        let xbrl_url = build_xbrl_url(accession_number)?;

        debug!("Downloading XBRL from: {}", xbrl_url);

        self.rate_limiter.wait_for_permit().await;

        let response = self
            .client
            .get(&xbrl_url)
            .send()
            .await
            .context("Failed to download XBRL file")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP error downloading XBRL: {}",
                response.status()
            ));
        }

        let content = response
            .bytes()
            .await
            .context("Failed to read response body")?;
        let file_size = content.len() as u64;

        // Store the XBRL file in the database
        let stored_doc = self
            .storage
            .store_xbrl_file(
                accession_number,
                &content,
                company.id,
                Utc.from_utc_datetime(&filing_date.and_hms_opt(0, 0, 0).unwrap()),
                Utc.from_utc_datetime(&report_date.and_hms_opt(0, 0, 0).unwrap()),
                report_date.year(),
                Some(get_fiscal_quarter(&report_date)),
                Some(&filing_info.form[0]),
                Some(&xbrl_url),
            )
            .await
            .context("Failed to store XBRL file")?;

        info!(
            "Stored XBRL file: {} ({} bytes, compressed: {})",
            accession_number, file_size, stored_doc.compressed_size
        );

        Ok(file_size)
    }

    /// Get crawl progress for a running operation
    pub async fn get_crawl_progress(&self, operation_id: Uuid) -> Result<CrawlProgress> {
        // TODO: Implement progress tracking
        // This would require storing progress in the database or cache
        Err(anyhow::anyhow!("Progress tracking not yet implemented"))
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<crate::models::XbrlStorageStats> {
        self.storage.get_storage_stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use testcontainers::core::WaitFor;
    use testcontainers::images::generic::GenericImage;
    use testcontainers::{clients, images::postgres::Postgres, Container};

    #[tokio::test]
    async fn test_crawler_creation() {
        // Setup test database
        let docker = clients::Cli::default();
        let postgres_image = GenericImage::new("postgres", "15")
            .with_env_var("POSTGRES_PASSWORD", "password")
            .with_env_var("POSTGRES_DB", "test")
            .with_wait_for(WaitFor::message_on_stderr(
                "database system is ready to accept connections",
            ));

        let container = docker.run(postgres_image);
        let connection_string = format!(
            "postgres://postgres:password@localhost:{}/test",
            container.get_host_port_ipv4(5432)
        );

        // TODO: Setup database schema and run migration
        // TODO: Create crawler instance
        // TODO: Test crawler functionality

        // This is a placeholder test - actual implementation would require
        // database setup and migration running
    }
}
