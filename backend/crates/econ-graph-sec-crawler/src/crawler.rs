use anyhow::Result;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use econ_graph_crawler::{CrawlError, HttpConfig, HttpFetcher, SourceId, SourcePolicy};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::models::{
    CrawlConfig, CrawlProgress, CrawlResult, DtsReference, FilingInfo, SecCompany,
};
use crate::storage::{XbrlStorage, XbrlStorageConfig};
use crate::submissions::EdgarSubmissions;
use crate::utils::{
    get_fiscal_quarter, pad_cik, parse_accession_number, parse_sec_date, unpad_cik,
};
use econ_graph_core::database::DatabasePool;
use econ_graph_metrics::crawler::CRAWLER_METRICS;

/// Real EDGAR JSON API host (submissions, company facts).
pub const DEFAULT_DATA_BASE_URL: &str = "https://data.sec.gov";
/// Real EDGAR archives host (filing documents).
pub const DEFAULT_ARCHIVES_BASE_URL: &str = "https://www.sec.gov";

/// Where the crawler sends requests. Tests point both at a mock server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecEndpoints {
    /// API root for `/submissions/CIK##########.json` (no trailing slash).
    pub data_base_url: String,
    /// Root for `/Archives/edgar/data/...` (no trailing slash).
    pub archives_base_url: String,
}

impl SecEndpoints {
    /// Both hosts replaced by `data_base_url` / `archives_base_url` (trailing slashes trimmed).
    pub fn new(data_base_url: impl Into<String>, archives_base_url: impl Into<String>) -> Self {
        Self {
            data_base_url: data_base_url.into().trim_end_matches('/').to_string(),
            archives_base_url: archives_base_url.into().trim_end_matches('/').to_string(),
        }
    }

    /// `GET` URL of a company's submissions document.
    pub fn submissions_url(&self, cik: &str) -> String {
        format!(
            "{}/submissions/CIK{}.json",
            self.data_base_url,
            pad_cik(cik)
        )
    }

    /// URL of the XBRL instance document for a filing of company `cik`.
    ///
    /// Filing folders live under the *company's* CIK (the accession number's prefix is the filer
    /// agent's CIK, which often differs). For inline-XBRL filings EDGAR publishes the extracted
    /// instance as `<primary document stem>_htm.xml`; for other filings the legacy
    /// `<accession>.xbrl` name is kept.
    pub fn xbrl_instance_url(&self, cik: &str, filing: &FilingInfo) -> Result<String, CrawlError> {
        let accession = first(&filing.accession_number);
        parse_accession_number(accession).map_err(|e| CrawlError::Parse(format!("SEC: {e}")))?;
        let folder = format!(
            "{}/Archives/edgar/data/{}/{}",
            self.archives_base_url,
            unpad_cik(&pad_cik(cik)),
            accession.replace('-', "")
        );
        let primary = first(&filing.primary_document);
        let inline = filing.is_inline_xbrl.first().copied().unwrap_or(0) != 0;
        match primary.strip_suffix(".htm") {
            Some(stem) if inline && !stem.is_empty() => Ok(format!("{folder}/{stem}_htm.xml")),
            _ => Ok(format!("{folder}/{accession}.xbrl")),
        }
    }
}

impl Default for SecEndpoints {
    fn default() -> Self {
        Self::new(DEFAULT_DATA_BASE_URL, DEFAULT_ARCHIVES_BASE_URL)
    }
}

fn first(v: &[String]) -> &str {
    v.first().map(String::as_str).unwrap_or("")
}

/// Normalizes a CIK given with or without zero padding (and an optional `CIK` prefix) to the
/// 10-digit form EDGAR and the `companies` table use.
///
/// Fails with [`CrawlError::Permanent`] for anything that is not 1-10 digits or is all zeros.
pub fn normalize_cik(raw: &str) -> Result<String, CrawlError> {
    let s = raw.trim();
    let s = s
        .strip_prefix("CIK")
        .or_else(|| s.strip_prefix("cik"))
        .unwrap_or(s);
    if s.is_empty() || s.len() > 10 || !s.chars().all(|c| c.is_ascii_digit()) {
        return Err(CrawlError::Permanent(format!("SEC: invalid CIK {raw:?}")));
    }
    if s.chars().all(|c| c == '0') {
        return Err(CrawlError::Permanent(format!("SEC: invalid CIK {raw:?}")));
    }
    Ok(pad_cik(s))
}

/// Builds the shared-layer HTTP client for SEC with the given `User-Agent` (SEC requires a
/// contact address in it; one without `@` is replaced by the default EconGraph agent) and at most
/// `requests_per_second` (capped at the shared SEC policy's 8 req/s).
pub fn sec_http_fetcher(
    user_agent: &str,
    requests_per_second: u32,
) -> Result<HttpFetcher, CrawlError> {
    let default_policy = SourcePolicy::default_for(SourceId::Sec);
    let rps = f64::from(requests_per_second.max(1)).min(default_policy.requests_per_second);
    let policy = SourcePolicy {
        requests_per_second: rps,
        burst: default_policy.burst.min(rps.ceil() as u32).max(1),
        ..default_policy
    };
    let mut config = HttpConfig::default();
    if user_agent.contains('@') {
        config.user_agent = user_agent.to_string();
    }
    HttpFetcher::new(config, HashMap::from([(SourceId::Sec, policy)]))
}

/// A filing-level failure and whether the queue should retry the company.
fn db_error(context: &str, e: impl std::fmt::Display) -> CrawlError {
    CrawlError::Transient(format!("SEC: {context}: {e}"))
}

/// **SEC EDGAR Crawler**
///
/// Main crawler implementation for SEC EDGAR XBRL filings.
/// Handles filing enumeration and XBRL file downloads.
///
/// All HTTP goes through a shared [`HttpFetcher`] under [`SourceId::Sec`], which applies the
/// SEC rate limit / concurrency policy, retries and metrics.
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_sec_crawler::SecEdgarCrawler;
///
/// # async fn example() -> anyhow::Result<()> {
/// let pool = econ_graph_core::database::create_pool("postgres://...").await?;
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
    http: HttpFetcher,
    endpoints: SecEndpoints,
    storage: XbrlStorage,
    config: CrawlConfig,
    pool: DatabasePool,
}

/// Outcome of [`SecEdgarCrawler::crawl_company`].
#[derive(Debug, Clone)]
pub struct CompanyCrawl {
    /// Summary (same as [`SecEdgarCrawler::crawl_company_filings`] returns).
    pub result: CrawlResult,
    /// `companies.id` of the crawled company.
    pub company_id: Uuid,
    /// Filings already stored before this run (skipped without downloading).
    pub filings_skipped: u32,
    /// Latest filing date among newly stored filings.
    pub latest_filing_date: Option<NaiveDate>,
    /// First retryable error (rate limit, 5xx, database) hit while downloading a filing. Filings
    /// stored before it are kept, so re-running the company resumes where it stopped.
    pub retryable_error: Option<CrawlError>,
}

impl SecEdgarCrawler {
    /// Create a new SEC EDGAR crawler instance
    pub async fn new(pool: DatabasePool) -> Result<Self> {
        Self::with_config(pool, CrawlConfig::default()).await
    }

    /// Create a new SEC EDGAR crawler with custom configuration; builds its own [`HttpFetcher`]
    /// from `config.user_agent` and `config.max_requests_per_second` (see [`sec_http_fetcher`]).
    pub async fn with_config(pool: DatabasePool, config: CrawlConfig) -> Result<Self> {
        let http = sec_http_fetcher(&config.user_agent, config.max_requests_per_second)?;
        Ok(Self::with_fetcher(
            pool,
            config,
            http,
            SecEndpoints::default(),
        ))
    }

    /// Crawler that sends every request through `http` (shared with other sources, e.g. the
    /// worker's `CrawlCtx::http`) to `endpoints`.
    pub fn with_fetcher(
        pool: DatabasePool,
        config: CrawlConfig,
        http: HttpFetcher,
        endpoints: SecEndpoints,
    ) -> Self {
        let storage = XbrlStorage::new(pool.clone(), XbrlStorageConfig::default());
        Self {
            http,
            endpoints,
            storage,
            config,
            pool,
        }
    }

    /// Crawl all filings for a specific company
    pub async fn crawl_company_filings(&self, cik: &str) -> Result<CrawlResult> {
        Ok(self.crawl_company(cik).await?.result)
    }

    /// Crawls one company: submissions -> company upsert -> filter filings -> download and store
    /// each XBRL instance not stored yet (with its DTS components).
    ///
    /// Errors before any filing is attempted (bad CIK, submissions HTTP/parse failure, database
    /// unavailable) are returned as `Err`. Per-filing failures are collected in
    /// `result.errors`; the first retryable one is also returned in
    /// [`CompanyCrawl::retryable_error`].
    pub async fn crawl_company(&self, cik: &str) -> Result<CompanyCrawl, CrawlError> {
        let cik = normalize_cik(cik)?;
        let operation_id = Uuid::new_v4();
        let start_time = Utc::now();

        info!("Starting crawl for company CIK: {}", cik);

        let submissions = self.fetch_submissions(&cik).await?;
        let company_id = self.upsert_company(&cik, &submissions).await?;
        let company = sec_company(company_id, &cik, &submissions);

        let rows = submissions.filings.recent.to_filing_infos();
        let filings = self.filter_filings(&rows)?;

        let mut crawl = CompanyCrawl {
            result: CrawlResult {
                operation_id,
                company_cik: Some(cik.clone()),
                operation_type: "company_filings".to_string(),
                start_time,
                end_time: None,
                total_filings_found: filings.len() as u32,
                filings_downloaded: 0,
                filings_failed: 0,
                total_bytes_downloaded: 0,
                errors: Vec::new(),
                success: false,
            },
            company_id,
            filings_skipped: 0,
            latest_filing_date: None,
            retryable_error: None,
        };

        let stored = self.stored_accessions(company_id).await?;

        for filing_info in filings {
            let accession = first(&filing_info.accession_number);
            if stored.contains(accession) {
                debug!("Filing {} already stored; skipping", accession);
                crawl.filings_skipped += 1;
                continue;
            }
            match self.download_filing_xbrl(&company, filing_info).await {
                Ok((bytes_downloaded, filing_date)) => {
                    crawl.result.filings_downloaded += 1;
                    crawl.result.total_bytes_downloaded += bytes_downloaded;
                    crawl.latest_filing_date = crawl.latest_filing_date.max(Some(filing_date));
                    debug!("Successfully downloaded filing: {}", accession);
                }
                Err(e) => {
                    crawl.result.filings_failed += 1;
                    let error_msg = format!("Failed to download filing {}: {}", accession, e);
                    error!("{}", error_msg);
                    crawl.result.errors.push(error_msg);
                    if e.is_retryable() {
                        // The rest would most likely fail the same way (rate limit, outage):
                        // stop and let the queue retry the company later.
                        crawl.retryable_error = Some(e);
                        break;
                    }
                }
            }
        }

        crawl.result.end_time = Some(Utc::now());
        crawl.result.success = crawl.result.filings_failed == 0;

        info!(
            "Crawl completed for CIK {}: {} downloaded, {} already stored, {} failed",
            cik,
            crawl.result.filings_downloaded,
            crawl.filings_skipped,
            crawl.result.filings_failed
        );

        Ok(crawl)
    }

    /// Fetches and decodes a company's submissions document (CIK padded or not).
    pub async fn fetch_submissions(&self, cik: &str) -> Result<EdgarSubmissions, CrawlError> {
        let url = self.endpoints.submissions_url(&normalize_cik(cik)?);
        self.http.get_json(SourceId::Sec, &url, &[]).await
    }

    /// Inserts or updates the `companies` row for `cik` and returns its id (stable across runs).
    async fn upsert_company(&self, cik: &str, subs: &EdgarSubmissions) -> Result<Uuid, CrawlError> {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::companies::dsl as c;

        fn clip(s: Option<&str>, max: usize) -> Option<String> {
            s.map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.chars().take(max).collect())
        }
        let name: String = subs.name.trim().chars().take(255).collect();
        let name = if name.is_empty() {
            format!("CIK {cik}")
        } else {
            name
        };
        let ticker = clip(<[String]>::first(&subs.tickers).map(String::as_str), 10);
        let sic = clip(subs.sic.as_deref(), 4);
        let sic_description = clip(subs.sic_description.as_deref(), 255);
        let entity_type = clip(subs.entity_type.as_deref(), 50);

        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| db_error("database connection", e))?;
        diesel::insert_into(c::companies)
            .values((
                c::cik.eq(cik),
                c::name.eq(&name),
                c::ticker.eq(&ticker),
                c::sic_code.eq(&sic),
                c::sic_description.eq(&sic_description),
                c::entity_type.eq(&entity_type),
            ))
            .on_conflict(c::cik)
            .do_update()
            .set((
                c::name.eq(&name),
                c::ticker.eq(&ticker),
                c::sic_code.eq(&sic),
                c::sic_description.eq(&sic_description),
                c::entity_type.eq(&entity_type),
                c::updated_at.eq(Utc::now()),
            ))
            .returning(c::id)
            .get_result::<Uuid>(&mut conn)
            .await
            .map_err(|e| db_error("upserting company", e))
    }

    /// Accession numbers already stored for `company_id`.
    async fn stored_accessions(
        &self,
        company_id: Uuid,
    ) -> Result<std::collections::HashSet<String>, CrawlError> {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::financial_statements::dsl as fs;
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| db_error("database connection", e))?;
        let rows: Vec<String> = fs::financial_statements
            .filter(fs::company_id.eq(company_id))
            .select(fs::accession_number)
            .load(&mut conn)
            .await
            .map_err(|e| db_error("loading stored filings", e))?;
        Ok(rows.into_iter().collect())
    }

    /// Filter filings based on configuration
    fn filter_filings<'a>(
        &self,
        filings: &'a [FilingInfo],
    ) -> Result<Vec<&'a FilingInfo>, CrawlError> {
        let mut filtered = Vec::new();

        for filing in filings {
            // Check form type filter
            if let Some(ref form_types) = self.config.form_types {
                if !form_types.iter().any(|f| f == first(&filing.form)) {
                    continue;
                }
            }

            // Check date range filter
            if self.config.start_date.is_some() || self.config.end_date.is_some() {
                let filing_date = parse_sec_date(first(&filing.filing_date))
                    .map_err(|e| CrawlError::Parse(format!("SEC: {e}")))?;
                if self.config.start_date.is_some_and(|d| filing_date < d)
                    || self.config.end_date.is_some_and(|d| filing_date > d)
                {
                    continue;
                }
            }

            // Check if XBRL is available
            if filing.is_xbrl.first().copied().unwrap_or(0) == 0 {
                continue;
            }

            // Check file size limit
            if let Some(&size) = filing.size.first() {
                if size > self.config.max_file_size_bytes {
                    warn!(
                        "Skipping filing {}: size {} exceeds limit {}",
                        first(&filing.accession_number),
                        size,
                        self.config.max_file_size_bytes
                    );
                    continue;
                }
            }

            filtered.push(filing);
        }

        Ok(filtered)
    }

    /// Download and store the XBRL instance of one filing. Returns (bytes, filing date).
    async fn download_filing_xbrl(
        &self,
        company: &SecCompany,
        filing_info: &FilingInfo,
    ) -> Result<(u64, NaiveDate), CrawlError> {
        let accession_number = first(&filing_info.accession_number);
        let parse = |s: &str| parse_sec_date(s).map_err(|e| CrawlError::Parse(format!("SEC: {e}")));
        let filing_date = parse(first(&filing_info.filing_date))?;
        // Some forms carry no report date; fall back to the filing date.
        let report_date = match first(&filing_info.report_date) {
            "" => filing_date,
            s => parse(s)?,
        };

        let xbrl_url = self
            .endpoints
            .xbrl_instance_url(&company.cik, filing_info)?;

        debug!("Downloading XBRL from: {}", xbrl_url);

        let content = self.http.get_text(SourceId::Sec, &xbrl_url, &[]).await?;
        let file_size = content.len() as u64;

        // Record bytes downloaded metric
        CRAWLER_METRICS.record_bytes_downloaded("sec", "edgar", file_size);

        let midnight = |d: NaiveDate| {
            DateTime::from_naive_utc_and_offset(d.and_hms_opt(0, 0, 0).expect("valid time"), Utc)
        };

        // Store the XBRL file in the database
        let stored_doc = self
            .storage
            .store_xbrl_file(
                accession_number,
                content.as_bytes(),
                company.id,
                midnight(filing_date),
                midnight(report_date),
                report_date.year(),
                Some(get_fiscal_quarter(&report_date)),
                Some(first(&filing_info.form)),
                Some(&xbrl_url),
            )
            .await
            .map_err(|e| db_error("storing XBRL file", format!("{e:#}")))?;

        info!(
            "Stored XBRL file: {} ({} bytes, compressed: {})",
            accession_number, file_size, stored_doc.compressed_size
        );

        // Discover and download DTS components
        if let Err(e) = self
            .download_dts_components(content.as_bytes(), &xbrl_url, &stored_doc.id)
            .await
        {
            warn!(
                "Failed to download DTS components for {}: {}",
                accession_number, e
            );
            // Don't fail the entire process if DTS download fails
        }

        Ok((file_size, filing_date))
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
                    if e.name().as_ref() == "schemaRef" || e.name().as_ref() == "linkbaseRef" {
                        let mut href = None;
                        let mut role = None;
                        let mut arcrole = None;

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                "href" => {
                                    href = Some(attr.value.to_string());
                                }
                                "role" => {
                                    role = Some(attr.value.to_string());
                                }
                                "arcrole" => {
                                    arcrole = Some(attr.value.to_string());
                                }
                                _ => {}
                            }
                        }

                        if let Some(href) = href {
                            let reference_type = if e.name().as_ref() == "schemaRef" {
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
        // Resolve the (possibly relative) href against the instance document's URL
        let taxonomy_url = url::Url::parse(base_url)?
            .join(&reference.reference_href)?
            .to_string();

        debug!("Downloading taxonomy component from: {}", taxonomy_url);

        let content = self
            .http
            .get_text(SourceId::Sec, &taxonomy_url, &[])
            .await?;

        // Store the taxonomy component
        self.storage
            .store_taxonomy_component(reference, content.as_bytes(), &taxonomy_url, statement_id)
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

/// The [`SecCompany`] view of a submissions document, with the stored `companies.id`.
fn sec_company(id: Uuid, cik: &str, subs: &EdgarSubmissions) -> SecCompany {
    SecCompany {
        id,
        cik: cik.to_string(),
        name: subs.name.clone(),
        ticker: subs.tickers.first().cloned(),
        sic_code: subs.sic.clone(),
        sic_description: subs.sic_description.clone(),
        state_of_incorporation: None,
        fiscal_year_end: None,
        entity_type: subs.entity_type.clone(),
        entity_size: None,
        business_address: None,
        mailing_address: None,
        phone: None,
        website: None,
        created_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_cik_pads_and_rejects_garbage() {
        assert_eq!(normalize_cik("320193").unwrap(), "0000320193");
        assert_eq!(normalize_cik("0000320193").unwrap(), "0000320193");
        assert_eq!(normalize_cik(" CIK320193 ").unwrap(), "0000320193");
        for bad in ["", "0", "0000000000", "abc", "12345678901", "32-0193"] {
            assert!(
                matches!(normalize_cik(bad), Err(CrawlError::Permanent(_))),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn xbrl_instance_url_uses_company_cik_and_inline_instance() {
        let raw = include_str!("../test_data/sec_mock/submissions_CIK0009999901.json");
        let subs: EdgarSubmissions = serde_json::from_str(raw).unwrap();
        let rows = subs.filings.recent.to_filing_infos();
        let ep = SecEndpoints::new("http://data/", "http://archives");
        assert_eq!(
            ep.xbrl_instance_url("0009999901", &rows[0]).unwrap(),
            "http://archives/Archives/edgar/data/9999901/000095017024000001/tstc-20231231_htm.xml"
        );
        // Not inline XBRL: legacy name.
        assert_eq!(
            ep.xbrl_instance_url("9999901", &rows[2]).unwrap(),
            "http://archives/Archives/edgar/data/9999901/000999990124000003/0009999901-24-000003.xbrl"
        );
        assert_eq!(
            ep.submissions_url("9999901"),
            "http://data/submissions/CIK0009999901.json"
        );
    }
}
