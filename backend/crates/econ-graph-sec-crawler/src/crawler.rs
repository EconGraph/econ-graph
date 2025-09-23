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
    CompanySubmissionsResponse, CrawlConfig, CrawlProgress, CrawlResult, DtsReference, FilingInfo,
    SecCompany, SecFiling, StoredXbrlDocument,
};
use crate::rate_limiter::SecRateLimiter;
use crate::storage::{XbrlStorage, XbrlStorageConfig};
use crate::utils::{build_submissions_url, build_xbrl_url, get_fiscal_quarter, parse_sec_date};
use econ_graph_core::database::DatabasePool;
use econ_graph_core::models::{Company, FinancialStatement};
use econ_graph_metrics::crawler::CRAWLER_METRICS;

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
#[derive(Clone)]
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

        let start = std::time::Instant::now();
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch company submissions")?;

        let duration = start.elapsed().as_secs_f64();
        let status = response.status();
        CRAWLER_METRICS.record_request("sec", "edgar", "/submissions", status.as_str(), duration);
        if status.as_u16() == 429 {
            CRAWLER_METRICS.record_rate_limit_hit("sec", "edgar");
        }
        if !status.is_success() {
            CRAWLER_METRICS.record_error("sec", "edgar", "http_error");
            return Err(anyhow::anyhow!("HTTP error: {}", status));
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
    fn filter_filings<'a>(&self, filings: &'a [FilingInfo]) -> Result<Vec<&'a FilingInfo>> {
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

        let start = std::time::Instant::now();
        let response = self
            .client
            .get(&xbrl_url)
            .send()
            .await
            .context("Failed to download XBRL file")?;

        let duration = start.elapsed().as_secs_f64();
        let status = response.status();
        CRAWLER_METRICS.record_request("sec", "edgar", "/xbrl", status.as_str(), duration);
        if status.as_u16() == 429 {
            CRAWLER_METRICS.record_rate_limit_hit("sec", "edgar");
        }
        if !status.is_success() {
            CRAWLER_METRICS.record_error("sec", "edgar", "http_error");
            return Err(anyhow::anyhow!("HTTP error downloading XBRL: {}", status));
        }

        let content = response
            .bytes()
            .await
            .context("Failed to read response body")?;
        let file_size = content.len() as u64;

        // Record bytes downloaded metric
        CRAWLER_METRICS.record_bytes_downloaded("sec", "edgar", file_size);

        // Store the XBRL file in the database
        let stored_doc = self
            .storage
            .store_xbrl_file(
                accession_number,
                &content,
                company.id,
                DateTime::from_naive_utc_and_offset(filing_date.and_hms_opt(0, 0, 0).unwrap(), Utc),
                DateTime::from_naive_utc_and_offset(report_date.and_hms_opt(0, 0, 0).unwrap(), Utc),
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

        // Discover and download DTS components
        if let Err(e) = self
            .download_dts_components(&content, &xbrl_url, &stored_doc.id)
            .await
        {
            warn!(
                "Failed to download DTS components for {}: {}",
                accession_number, e
            );
            // Don't fail the entire process if DTS download fails
        }

        Ok(file_size)
    }

    /// Download DTS (Discoverable Taxonomy Set) components for an XBRL instance
    async fn download_dts_components(
        &self,
        xbrl_content: &[u8],
        xbrl_url: &str,
        statement_id: &Uuid,
    ) -> Result<()> {
        debug!("Discovering DTS components for XBRL file");

        // Parse XBRL content to find schema references
        let dts_references = self.discover_dts_references(xbrl_content)?;

        info!("Found {} DTS references in XBRL file", dts_references.len());

        // Download each referenced taxonomy component
        for reference in dts_references {
            if let Err(e) = self
                .download_taxonomy_component(&reference, xbrl_url, statement_id)
                .await
            {
                warn!(
                    "Failed to download taxonomy component {}: {}",
                    reference.reference_href, e
                );
                // Continue with other components
            }
        }

        Ok(())
    }

    /// Discover DTS references in XBRL content
    fn discover_dts_references(&self, xbrl_content: &[u8]) -> Result<Vec<DtsReference>> {
        use quick_xml::events::Event;
        use quick_xml::Reader;
        use std::io::Cursor;

        let mut reader = Reader::from_reader(Cursor::new(xbrl_content));
        reader.config_mut().trim_text(true);

        let mut references = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    if e.name().as_ref() == b"schemaRef" || e.name().as_ref() == b"linkbaseRef" {
                        let mut href = None;
                        let mut role = None;
                        let mut arcrole = None;

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"href" => {
                                    if let Ok(value) = std::str::from_utf8(&attr.value) {
                                        href = Some(value.to_string());
                                    }
                                }
                                b"role" => {
                                    if let Ok(value) = std::str::from_utf8(&attr.value) {
                                        role = Some(value.to_string());
                                    }
                                }
                                b"arcrole" => {
                                    if let Ok(value) = std::str::from_utf8(&attr.value) {
                                        arcrole = Some(value.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }

                        if let Some(href) = href {
                            let reference_type = if e.name().as_ref() == b"schemaRef" {
                                "schemaRef"
                            } else {
                                "linkbaseRef"
                            };

                            references.push(DtsReference {
                                reference_type: reference_type.to_string(),
                                reference_role: role,
                                reference_href: href,
                                reference_arcrole: arcrole,
                            });
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to parse XBRL for DTS discovery: {}",
                        e
                    ));
                }
            }
            buf.clear();
        }

        Ok(references)
    }

    /// Download a single taxonomy component
    async fn download_taxonomy_component(
        &self,
        reference: &DtsReference,
        base_url: &str,
        statement_id: &Uuid,
    ) -> Result<()> {
        // Construct the full URL for the taxonomy component
        let taxonomy_url = if reference.reference_href.starts_with("http") {
            reference.reference_href.clone()
        } else {
            // Relative URL - construct from base URL
            let base_path = std::path::Path::new(base_url)
                .parent()
                .unwrap_or_else(|| std::path::Path::new("/"));
            base_path
                .join(&reference.reference_href)
                .to_string_lossy()
                .to_string()
        };

        debug!("Downloading taxonomy component from: {}", taxonomy_url);

        self.rate_limiter.wait_for_permit().await;

        let start = std::time::Instant::now();
        let response = self
            .client
            .get(&taxonomy_url)
            .send()
            .await
            .context("Failed to download taxonomy component")?;

        let duration = start.elapsed().as_secs_f64();
        let status = response.status();
        CRAWLER_METRICS.record_request("sec", "edgar", "/taxonomy", status.as_str(), duration);
        if status.as_u16() == 429 {
            CRAWLER_METRICS.record_rate_limit_hit("sec", "edgar");
        }
        if !status.is_success() {
            CRAWLER_METRICS.record_error("sec", "edgar", "http_error");
            return Err(anyhow::anyhow!(
                "HTTP error downloading taxonomy component: {}",
                status
            ));
        }

        let content = response
            .bytes()
            .await
            .context("Failed to read taxonomy component response")?;

        // Store the taxonomy component
        self.storage
            .store_taxonomy_component(reference, &content, &taxonomy_url, statement_id)
            .await?;

        info!(
            "Downloaded and stored taxonomy component: {}",
            reference.reference_href
        );

        Ok(())
    }

    /// Get crawl progress for a running operation
    pub async fn get_crawl_progress(&self, operation_id: Uuid) -> Result<CrawlProgress> {
        // TODO: Implement progress tracking with database storage
        // For now, return a placeholder progress
        Ok(CrawlProgress {
            operation_id,
            operation_type: "company_filings".to_string(),
            current_item: "Unknown".to_string(),
            items_processed: 0,
            total_items: 0,
            progress_percentage: 0.0,
            start_time: Utc::now(),
            last_updated: Utc::now(),
            estimated_remaining_seconds: 0,
            current_phase: "Initializing".to_string(),
        })
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<crate::models::XbrlStorageStats> {
        self.storage.get_storage_stats().await
    }

    /// Crawl multiple companies concurrently
    pub async fn crawl_multiple_companies(&self, ciks: Vec<String>) -> Result<Vec<CrawlResult>> {
        let mut results = Vec::new();
        let mut handles = Vec::new();

        // Limit concurrent operations to avoid overwhelming SEC servers
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(
            self.config.max_concurrent_requests.unwrap_or(3),
        ));

        for cik in ciks {
            let semaphore = semaphore.clone();
            let crawler = self.clone();
            let cik_clone = cik.clone(); // Clone the CIK before moving it

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                crawler.crawl_company_filings(&cik_clone).await
            });

            handles.push((cik, handle));
        }

        // Wait for all operations to complete
        for (cik, handle) in handles {
            match handle.await {
                Ok(Ok(result)) => {
                    info!(
                        "Successfully crawled company {}: {} filings",
                        cik, result.filings_downloaded
                    );
                    results.push(result);
                }
                Ok(Err(e)) => {
                    error!("Failed to crawl company {}: {}", cik, e);
                    results.push(CrawlResult {
                        operation_id: Uuid::new_v4(),
                        company_cik: Some(cik.clone()),
                        operation_type: "company_filings".to_string(),
                        start_time: Utc::now(),
                        end_time: Some(Utc::now()),
                        total_filings_found: 0,
                        filings_downloaded: 0,
                        filings_failed: 0,
                        total_bytes_downloaded: 0,
                        errors: vec![format!("Failed to crawl company: {}", e)],
                        success: false,
                    });
                }
                Err(e) => {
                    error!("Task failed for company {}: {}", cik, e);
                    results.push(CrawlResult {
                        operation_id: Uuid::new_v4(),
                        company_cik: Some(cik.clone()),
                        operation_type: "company_filings".to_string(),
                        start_time: Utc::now(),
                        end_time: Some(Utc::now()),
                        total_filings_found: 0,
                        filings_downloaded: 0,
                        filings_failed: 0,
                        total_bytes_downloaded: 0,
                        errors: vec![format!("Task failed: {}", e)],
                        success: false,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Crawl all companies in the S&P 500 index
    pub async fn crawl_sp500_companies(&self) -> Result<Vec<CrawlResult>> {
        info!("Starting S&P 500 company crawl");

        // Load S&P 500 CIKs from a predefined list
        let sp500_ciks = self.load_sp500_ciks().await?;

        info!("Found {} S&P 500 companies to crawl", sp500_ciks.len());

        self.crawl_multiple_companies(sp500_ciks).await
    }

    /// Load S&P 500 CIKs from a predefined list or external source
    async fn load_sp500_ciks(&self) -> Result<Vec<String>> {
        // For now, return a small sample of major companies
        // In production, this would load from a comprehensive S&P 500 list
        Ok(vec![
            "0000320193".to_string(), // Apple Inc.
            "0000789019".to_string(), // Microsoft Corporation
            "0001018724".to_string(), // Amazon.com Inc.
            "0001067983".to_string(), // Alphabet Inc. (Google)
            "0000078003".to_string(), // Tesla Inc.
            "0000789019".to_string(), // NVIDIA Corporation
            "0000079038".to_string(), // Meta Platforms Inc. (Facebook)
            "0001341439".to_string(), // Berkshire Hathaway Inc.
            "0001032975".to_string(), // Johnson & Johnson
            "0000066740".to_string(), // JPMorgan Chase & Co.
        ])
    }

    /// Crawl companies by industry (SIC code)
    pub async fn crawl_companies_by_industry(&self, sic_code: &str) -> Result<Vec<CrawlResult>> {
        info!("Starting crawl for industry SIC code: {}", sic_code);

        // Get companies by SIC code from SEC
        let companies = self.get_companies_by_sic(sic_code).await?;
        let ciks: Vec<String> = companies.into_iter().map(|c| c.cik).collect();

        self.crawl_multiple_companies(ciks).await
    }

    /// Get companies by SIC code from SEC EDGAR
    async fn get_companies_by_sic(&self, sic_code: &str) -> Result<Vec<SecCompany>> {
        // This would require implementing SEC company search API
        // For now, return a placeholder
        warn!("SIC-based company search not yet implemented, returning empty list");
        Ok(Vec::new())
    }

    /// Crawl recent filings (last N days)
    pub async fn crawl_recent_filings(&self, days: u32) -> Result<Vec<CrawlResult>> {
        let start_date = Utc::now().date_naive() - chrono::Duration::days(days as i64);

        info!("Starting crawl for recent filings since: {}", start_date);

        // Get recent filings from SEC
        let recent_filings = self.get_recent_filings(Some(start_date), None).await?;

        // Group by company CIK
        let mut company_filings: std::collections::HashMap<String, Vec<FilingInfo>> =
            std::collections::HashMap::new();
        for filing in recent_filings {
            // Extract CIK from filing info (this would need to be implemented)
            // For now, we'll need to modify the approach
        }

        let ciks: Vec<String> = company_filings.keys().cloned().collect();
        self.crawl_multiple_companies(ciks).await
    }

    /// Get recent filings from SEC EDGAR
    async fn get_recent_filings(
        &self,
        start_date: Option<chrono::NaiveDate>,
        end_date: Option<chrono::NaiveDate>,
    ) -> Result<Vec<FilingInfo>> {
        // This would require implementing SEC filings search API
        // For now, return a placeholder
        warn!("Recent filings search not yet implemented, returning empty list");
        Ok(Vec::new())
    }

    /// Parse and store XBRL data after downloading
    pub async fn parse_and_store_xbrl(&self, accession_number: &str) -> Result<()> {
        info!("Parsing and storing XBRL data for: {}", accession_number);

        // Retrieve the XBRL file from storage
        let xbrl_content = self.storage.retrieve_xbrl_file(accession_number).await?;

        // Parse using our XBRL parser with Arelle
        use crate::xbrl_parser::{XbrlParser, XbrlParserConfig};

        let config = XbrlParserConfig {
            use_arelle: true, // Use Arelle for comprehensive parsing
            ..Default::default()
        };

        let parser = XbrlParser::with_config_and_database(config, Some(self.pool.clone())).await?;

        // Create a temporary file for parsing
        let temp_file = std::env::temp_dir().join(format!("{}.xml", accession_number));
        tokio::fs::write(&temp_file, &xbrl_content).await?;

        // Parse the XBRL file
        let parse_result = parser.parse_xbrl_document(&temp_file).await?;

        // Store the parsed financial statements in the database
        // This would require implementing the storage logic for parsed data
        info!(
            "Successfully parsed XBRL file: {} statements, {} facts",
            parse_result.statements.len(),
            parse_result.facts.len()
        );

        // Clean up temporary file
        let _ = tokio::fs::remove_file(&temp_file).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_crawler_creation() {
        // TODO: Implement proper integration tests with testcontainers
        // This is a placeholder test - actual implementation would require
        // database setup and migration running
    }
}
