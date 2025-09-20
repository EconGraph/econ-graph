use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// **SEC Filing Model**
///
/// Represents a SEC EDGAR filing with metadata and download information.
/// This model contains all the information needed to download and process XBRL filings.
///
/// # Use Cases
/// - Storing SEC filing metadata before download
/// - Tracking filing status and processing progress
/// - Supporting filing discovery and filtering
/// - Enabling batch processing of multiple filings
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_sec_crawler::models::SecFiling;
/// use uuid::Uuid;
/// use chrono::{NaiveDate, Utc};
///
/// // Apple's 10-K filing for fiscal year 2023
/// let filing = SecFiling {
///     id: Uuid::new_v4(),
///     company_cik: "0000320193".to_string(),
///     company_name: "Apple Inc.".to_string(),
///     ticker: Some("AAPL".to_string()),
///     form_type: "10-K".to_string(),
///     filing_date: NaiveDate::from_ymd_opt(2023, 11, 3).unwrap(),
///     period_end_date: NaiveDate::from_ymd_opt(2023, 9, 30).unwrap(),
///     accession_number: "0000320193-23-000006".to_string(),
///     document_url: "https://www.sec.gov/Archives/edgar/data/320193/000032019323000006/aapl-20230930.htm".to_string(),
///     xbrl_url: Some("https://www.sec.gov/Archives/edgar/data/320193/000032019323000006/aapl-20230930.xbrl".to_string()),
///     file_size_bytes: Some(2048576), // 2MB
///     is_amended: false,
///     amendment_type: None,
///     original_filing_date: None,
///     is_restated: false,
///     restatement_reason: None,
///     created_at: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecFiling {
    /// Unique identifier for this filing record
    pub id: Uuid,

    /// Company CIK (Central Index Key)
    pub company_cik: String,

    /// Company name
    pub company_name: String,

    /// Stock ticker symbol (if available)
    pub ticker: Option<String>,

    /// SEC form type (10-K, 10-Q, 8-K, etc.)
    pub form_type: String,

    /// Date when the filing was submitted to the SEC
    pub filing_date: NaiveDate,

    /// End date of the reporting period
    pub period_end_date: NaiveDate,

    /// SEC accession number
    pub accession_number: String,

    /// URL to the main filing document
    pub document_url: String,

    /// URL to the XBRL instance document
    pub xbrl_url: Option<String>,

    /// Size of the XBRL file in bytes
    pub file_size_bytes: Option<u64>,

    /// Whether this is an amended filing
    pub is_amended: bool,

    /// Type of amendment (if applicable)
    pub amendment_type: Option<String>,

    /// Original filing date (if amended)
    pub original_filing_date: Option<NaiveDate>,

    /// Whether this filing contains restatements
    pub is_restated: bool,

    /// Reason for restatement (if applicable)
    pub restatement_reason: Option<String>,

    /// When this record was created
    pub created_at: DateTime<Utc>,
}

/// **SEC Company Model**
///
/// Represents a company that files with the SEC.
/// Contains basic company information and filing history.
///
/// # Use Cases
/// - Storing company metadata from SEC EDGAR
/// - Supporting company search and discovery
/// - Tracking filing history and patterns
/// - Enabling company-specific crawling
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_sec_crawler::models::SecCompany;
/// use uuid::Uuid;
/// use chrono::Utc;
///
/// // Apple Inc. company record
/// let company = SecCompany {
///     id: Uuid::new_v4(),
///     cik: "0000320193".to_string(),
///     name: "Apple Inc.".to_string(),
///     ticker: Some("AAPL".to_string()),
///     sic_code: Some("3571".to_string()),
///     sic_description: Some("Electronic Computers".to_string()),
///     state_of_incorporation: Some("CA".to_string()),
///     fiscal_year_end: Some("09-30".to_string()),
///     entity_type: Some("Corporation".to_string()),
///     entity_size: Some("Large Accelerated Filer".to_string()),
///     business_address: None,
///     mailing_address: None,
///     phone: None,
///     website: Some("https://www.apple.com".to_string()),
///     created_at: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecCompany {
    /// Unique identifier for this company record
    pub id: Uuid,

    /// SEC Central Index Key (CIK)
    pub cik: String,

    /// Company name
    pub name: String,

    /// Stock ticker symbol
    pub ticker: Option<String>,

    /// SIC code
    pub sic_code: Option<String>,

    /// SIC description
    pub sic_description: Option<String>,

    /// State of incorporation
    pub state_of_incorporation: Option<String>,

    /// Fiscal year end (MM-DD format)
    pub fiscal_year_end: Option<String>,

    /// Entity type
    pub entity_type: Option<String>,

    /// Entity size classification
    pub entity_size: Option<String>,

    /// Business address
    pub business_address: Option<serde_json::Value>,

    /// Mailing address
    pub mailing_address: Option<serde_json::Value>,

    /// Phone number
    pub phone: Option<String>,

    /// Website URL
    pub website: Option<String>,

    /// When this record was created
    pub created_at: DateTime<Utc>,
}

/// **CrawlResult Model**
///
/// Result of a crawling operation with statistics and error information.
/// Used for monitoring and reporting on crawl operations.
///
/// # Use Cases
/// - Reporting crawl operation results
/// - Monitoring crawl performance and success rates
/// - Tracking errors and failures
/// - Providing progress updates to users
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_sec_crawler::models::CrawlResult;
/// use uuid::Uuid;
/// use chrono::Utc;
///
/// // Result of crawling Apple's filings
/// let crawl_result = CrawlResult {
///     operation_id: Uuid::new_v4(),
///     company_cik: Some("0000320193".to_string()),
///     operation_type: "company_filings".to_string(),
///     start_time: Utc::now(),
///     end_time: Some(Utc::now()),
///     total_filings_found: 25,
///     filings_downloaded: 23,
///     filings_failed: 2,
///     total_bytes_downloaded: 52428800, // 50MB
///     errors: vec![
///         "Failed to download filing 0000320193-23-000001: HTTP 404".to_string(),
///         "Failed to download filing 0000320193-23-000002: Network timeout".to_string(),
///     ],
///     success: true,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    /// Unique identifier for this crawl operation
    pub operation_id: Uuid,

    /// Company CIK (if crawling specific company)
    pub company_cik: Option<String>,

    /// Type of crawl operation
    pub operation_type: String,

    /// When the crawl operation started
    pub start_time: DateTime<Utc>,

    /// When the crawl operation ended
    pub end_time: Option<DateTime<Utc>>,

    /// Total number of filings found
    pub total_filings_found: u32,

    /// Number of filings successfully downloaded
    pub filings_downloaded: u32,

    /// Number of filings that failed to download
    pub filings_failed: u32,

    /// Total bytes downloaded
    pub total_bytes_downloaded: u64,

    /// List of errors encountered
    pub errors: Vec<String>,

    /// Whether the overall operation was successful
    pub success: bool,
}

/// **CrawlConfig Model**
///
/// Configuration for SEC EDGAR crawling operations.
/// Controls rate limiting, retry logic, and other crawl parameters.
///
/// # Configuration Options
/// - Rate limiting settings
/// - Retry logic and backoff
/// - File size limits
/// - Date range filtering
/// - Form type filtering
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_sec_crawler::models::CrawlConfig;
/// use chrono::NaiveDate;
///
/// // Configuration for crawling recent 10-K filings
/// let config = CrawlConfig {
///     max_requests_per_second: 10,
///     max_retries: 3,
///     retry_delay_seconds: 5,
///     max_file_size_bytes: 50 * 1024 * 1024, // 50MB
///     start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
///     end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
///     form_types: Some(vec!["10-K".to_string(), "10-Q".to_string()]),
///     exclude_amended: true,
///     exclude_restated: false,
///     user_agent: "EconGraph-SEC-Crawler/1.0".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlConfig {
    /// Maximum requests per second
    pub max_requests_per_second: u32,

    /// Maximum number of retries for failed requests
    pub max_retries: u32,

    /// Delay between retries in seconds
    pub retry_delay_seconds: u64,

    /// Maximum file size to download (in bytes)
    pub max_file_size_bytes: u64,

    /// Start date for filing search
    pub start_date: Option<NaiveDate>,

    /// End date for filing search
    pub end_date: Option<NaiveDate>,

    /// Form types to include (None = all types)
    pub form_types: Option<Vec<String>>,

    /// Whether to exclude amended filings
    pub exclude_amended: bool,

    /// Whether to exclude restated filings
    pub exclude_restated: bool,

    /// User agent string for HTTP requests
    pub user_agent: String,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            max_requests_per_second: 10,
            max_retries: 3,
            retry_delay_seconds: 5,
            max_file_size_bytes: 50 * 1024 * 1024, // 50MB
            start_date: None,
            end_date: None,
            form_types: None,
            exclude_amended: false,
            exclude_restated: false,
            user_agent: "EconGraph Research Tool AdminContact@jmalicki+econgraph@gmail.com"
                .to_string(),
        }
    }
}

/// **XBRLDocument Model**
///
/// Represents a downloaded XBRL document with metadata.
/// Contains the document content and processing information.
///
/// # Use Cases
/// - Storing downloaded XBRL documents
/// - Tracking document processing status
/// - Supporting document validation and parsing
/// - Enabling document retrieval and analysis
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_sec_crawler::models::XbrlDocument;
/// use uuid::Uuid;
/// use chrono::Utc;
///
/// // Downloaded XBRL document
/// let xbrl_doc = XbrlDocument {
///     id: Uuid::new_v4(),
///     filing_id: Uuid::new_v4(),
///     accession_number: "0000320193-23-000006".to_string(),
///     file_path: "/data/xbrl/0000320193-23-000006.xbrl".to_string(),
///     file_size_bytes: 2048576,
///     download_url: "https://www.sec.gov/Archives/edgar/data/320193/000032019323000006/aapl-20230930.xbrl".to_string(),
///     downloaded_at: Utc::now(),
///     content_hash: "sha256:abc123...".to_string(),
///     is_valid_xml: true,
///     validation_errors: vec![],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XbrlDocument {
    /// Unique identifier for this document
    pub id: Uuid,

    /// ID of the associated filing
    pub filing_id: Uuid,

    /// SEC accession number
    pub accession_number: String,

    /// Local file path where document is stored
    pub file_path: String,

    /// Size of the file in bytes
    pub file_size_bytes: u64,

    /// URL where the document was downloaded from
    pub download_url: String,

    /// When the document was downloaded
    pub downloaded_at: DateTime<Utc>,

    /// SHA-256 hash of the document content
    pub content_hash: String,

    /// Whether the document is valid XML
    pub is_valid_xml: bool,

    /// List of XML validation errors
    pub validation_errors: Vec<String>,
}

/// **CrawlProgress Model**
///
/// Progress information for long-running crawl operations.
/// Used for monitoring and reporting progress to users.
///
/// # Progress Information
/// - Current operation status
/// - Progress percentages
/// - Estimated time remaining
/// - Current item being processed
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_sec_crawler::models::CrawlProgress;
/// use uuid::Uuid;
/// use chrono::Utc;
///
/// // Progress update for company crawling
/// let progress = CrawlProgress {
///     operation_id: Uuid::new_v4(),
///     operation_type: "company_filings".to_string(),
///     current_item: "0000320193-23-000006".to_string(),
///     items_processed: 15,
///     total_items: 25,
///     progress_percentage: 60.0,
///     estimated_remaining_seconds: 300,
///     current_phase: "downloading".to_string(),
///     start_time: Utc::now(),
///     last_updated: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlProgress {
    /// ID of the crawl operation
    pub operation_id: Uuid,

    /// Type of operation being performed
    pub operation_type: String,

    /// Current item being processed
    pub current_item: String,

    /// Number of items processed so far
    pub items_processed: u32,

    /// Total number of items to process
    pub total_items: u32,

    /// Progress percentage (0.0 to 100.0)
    pub progress_percentage: f64,

    /// Estimated time remaining in seconds
    pub estimated_remaining_seconds: u64,

    /// Current phase of the operation
    pub current_phase: String,

    /// When the operation started
    pub start_time: DateTime<Utc>,

    /// When this progress update was generated
    pub last_updated: DateTime<Utc>,
}

/// **SEC EDGAR API Response Models**
///
/// Models for parsing SEC EDGAR API responses.

/// **CompanyTickersResponse Model**
///
/// Response from SEC EDGAR company tickers API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyTickersResponse {
    /// Array of company ticker information
    pub tickers: Vec<CompanyTicker>,
}

/// **CompanyTicker Model**
///
/// Individual company ticker information from SEC EDGAR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyTicker {
    /// Company CIK
    pub cik_str: String,

    /// Company name
    pub title: String,

    /// Stock ticker symbol
    pub ticker: String,

    /// Exchange where the stock is traded
    pub exchange: String,
}

/// **CompanyFactsResponse Model**
///
/// Response from SEC EDGAR company facts API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyFactsResponse {
    /// Company CIK
    pub cik: u32,

    /// Company name
    pub entity_name: String,

    /// Entity type
    pub entity_type: String,

    /// SIC code
    pub sic: String,

    /// SIC description
    pub sic_description: String,

    /// Insider trading flag
    pub insider_transaction_for_issuer_exists: bool,

    /// Insider trading for owner flag
    pub insider_transaction_for_owner_exists: bool,

    /// Name of the owner
    pub name: String,

    /// Trading symbol
    pub tickers: Vec<String>,

    /// Exchange information
    pub exchanges: Vec<String>,

    /// Available facts by taxonomy
    pub facts: HashMap<String, serde_json::Value>,
}

/// **CompanySubmissionsResponse Model**
///
/// Response from SEC EDGAR company submissions API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanySubmissionsResponse {
    /// Company CIK
    pub cik: u32,

    /// Company name
    pub entity_type: String,

    /// SIC code
    pub sic: String,

    /// SIC description
    pub sic_description: String,

    /// Insider trading flag
    pub insider_transaction_for_issuer_exists: bool,

    /// Insider trading for owner flag
    pub insider_transaction_for_owner_exists: bool,

    /// Name of the owner
    pub name: String,

    /// Trading symbol
    pub tickers: Vec<String>,

    /// Exchange information
    pub exchanges: Vec<String>,

    /// Recent filings
    pub recent: RecentFilings,

    /// All filings
    pub filings: HashMap<String, serde_json::Value>,
}

/// **RecentFilings Model**
///
/// Recent filings information from company submissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFilings {
    /// Array of recent filing information
    pub filings: Vec<FilingInfo>,

    /// Array of recent filing forms
    pub forms: Vec<String>,
}

/// **FilingInfo Model**
///
/// Individual filing information from company submissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilingInfo {
    /// Accession number
    pub accession_number: Vec<String>,

    /// Filing date
    pub filing_date: Vec<String>,

    /// Report date
    pub report_date: Vec<String>,

    /// Acceptance date time
    pub acceptance_date_time: Vec<String>,

    /// Act
    pub act: Vec<String>,

    /// Form
    pub form: Vec<String>,

    /// File number
    pub file_number: Vec<String>,

    /// Film number
    pub film_number: Vec<String>,

    /// Items
    pub items: Vec<String>,

    /// Size
    pub size: Vec<u64>,

    /// Is XBRL
    pub is_xbrl: Vec<u32>,

    /// Is inline XBRL
    pub is_inline_xbrl: Vec<u32>,

    /// Primary document
    pub primary_document: Vec<String>,

    /// Primary doc description
    pub primary_doc_description: Vec<String>,
}

/// **Error Types**
///
/// Custom error types for SEC crawler operations.

#[derive(Debug, thiserror::Error)]
pub enum SecCrawlerError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Rate limit exceeded: {message}")]
    RateLimitExceeded { message: String },

    #[error("File download failed: {url} - {reason}")]
    DownloadFailed { url: String, reason: String },

    #[error("File size exceeds limit: {size} bytes (limit: {limit} bytes)")]
    FileSizeExceeded { size: u64, limit: u64 },

    #[error("Invalid XBRL document: {reason}")]
    InvalidXbrlDocument { reason: String },

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Parsing error: {0}")]
    ParsingError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Timeout error: {operation} timed out after {seconds} seconds")]
    TimeoutError { operation: String, seconds: u64 },

    #[error("Retry limit exceeded: {operation} failed after {attempts} attempts")]
    RetryLimitExceeded { operation: String, attempts: u32 },
}

/// **Result Type Alias**
///
/// Convenient result type for SEC crawler operations.
pub type SecCrawlerResult<T> = Result<T, SecCrawlerError>;

/// **XBRL Document Storage Model**
///
/// Represents a stored XBRL document with database storage information.
/// This is different from the XbrlDocument model above which represents
/// a downloaded document before storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredXbrlDocument {
    /// Unique identifier for this document
    pub id: Uuid,

    /// SEC accession number
    pub accession_number: String,

    /// Company ID this document belongs to
    pub company_id: Uuid,

    /// Filing date
    pub filing_date: DateTime<Utc>,

    /// Period end date
    pub period_end_date: DateTime<Utc>,

    /// Fiscal year
    pub fiscal_year: i32,

    /// Fiscal quarter (if applicable)
    pub fiscal_quarter: Option<i32>,

    /// Original file size in bytes
    pub file_size: usize,

    /// Compressed file size in bytes
    pub compressed_size: usize,

    /// Compression type used ("zstd", "lz4", "none")
    pub compression_type: String,

    /// SHA-256 hash of the original file
    pub file_hash: String,

    /// Storage method used ("large_object" or "bytea")
    pub storage_method: String,

    /// When this record was created
    pub created_at: DateTime<Utc>,
}

/// **XBRL Storage Statistics Model**
///
/// Statistics about XBRL file storage in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XbrlStorageStats {
    /// Total number of XBRL files stored
    pub total_files: u64,

    /// Total size of all files in bytes (original size)
    pub total_size_bytes: u64,

    /// Number of files stored as Large Objects
    pub large_object_files: u64,

    /// Number of files stored as bytea
    pub bytea_files: u64,

    /// Number of compressed files
    pub compressed_files: u64,

    /// Number of uncompressed files
    pub uncompressed_files: u64,
}

/// **DTS Reference Model**
///
/// Represents a DTS (Discoverable Taxonomy Set) reference found in XBRL instance files.
/// These references point to taxonomy schemas and linkbases that are required for proper XBRL parsing.
///
/// # Use Cases
/// - Parsing XBRL instance files to discover taxonomy dependencies
/// - Downloading and storing taxonomy components
/// - Tracking DTS resolution status
/// - Supporting proper XBRL parsing with complete taxonomies
///
/// # Examples
/// ```rust
/// use econ_graph_sec_crawler::models::DtsReference;
///
/// // US-GAAP schema reference
/// let schema_ref = DtsReference {
///     reference_type: "schemaRef".to_string(),
///     reference_role: None,
///     reference_href: "us-gaap/2024/entire/us-gaap-2024.xsd".to_string(),
///     reference_arcrole: None,
/// };
///
/// // Label linkbase reference
/// let linkbase_ref = DtsReference {
///     reference_type: "linkbaseRef".to_string(),
///     reference_role: Some("http://www.xbrl.org/2003/role/labelLinkbaseRef".to_string()),
///     reference_href: "us-gaap/2024/entire/us-gaap-2024_lab.xml".to_string(),
///     reference_arcrole: Some("http://www.w3.org/1999/xlink/properties/linkbase".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtsReference {
    /// Type of reference: "schemaRef" or "linkbaseRef"
    pub reference_type: String,

    /// Optional role attribute (for linkbase references)
    pub reference_role: Option<String>,

    /// URL or relative path to the taxonomy component
    pub reference_href: String,

    /// Optional arcrole attribute (for linkbase references)
    pub reference_arcrole: Option<String>,
}
