use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::enums::{CompressionType, ProcessingStatus};
use crate::schema::financial_statements;

/// **FinancialStatement Model**
///
/// Represents a financial statement filing from SEC EDGAR.
/// This model stores metadata about XBRL financial filings including
/// filing type, dates, and processing status.
///
/// # Use Cases
/// - Storing SEC EDGAR filing metadata
/// - Tracking XBRL processing status and errors
/// - Supporting financial statement queries and filtering
/// - Enabling time-series analysis of financial data
///
/// # Database Schema
/// Maps to the `financial_statements` table in PostgreSQL with comprehensive filing information.
/// Indexes are maintained on `company_id`, `filing_type`, `filing_date`, `period_end_date`,
/// `fiscal_year`, `fiscal_quarter`, and `accession_number` for optimal query performance.
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::FinancialStatement;
/// use uuid::Uuid;
/// use chrono::{NaiveDate, Utc};
///
/// // A 10-K annual report filing
/// let financial_statement = FinancialStatement {
///     id: Uuid::new_v4(),
///     company_id: Uuid::new_v4(),
///     filing_type: "10-K".to_string(),
///     form_type: "10-K".to_string(),
///     accession_number: "0000320193-23-000006".to_string(),
///     filing_date: NaiveDate::from_ymd_opt(2023, 11, 3).unwrap(),
///     period_end_date: NaiveDate::from_ymd_opt(2023, 9, 30).unwrap(),
///     fiscal_year: 2023,
///     fiscal_quarter: None, // Annual filing
///     document_type: "10-K".to_string(),
///     document_url: "https://www.sec.gov/Archives/edgar/data/320193/000032019323000006/aapl-20230930.htm".to_string(),
///     xbrl_file_oid: Some(12345), // PostgreSQL Large Object OID
///     xbrl_file_content: None, // Not using bytea storage
///     xbrl_file_size_bytes: Some(2048576), // 2MB
///     xbrl_file_compressed: Some(true),
///     xbrl_file_compression_type: Some("zstd".to_string()),
///     xbrl_file_hash: Some("sha256:abc123...".to_string()),
///     xbrl_processing_status: "completed".to_string(),
///     xbrl_processing_error: None,
///     xbrl_processing_started_at: Some(Utc::now()),
///     xbrl_processing_completed_at: Some(Utc::now()),
///     is_amended: false,
///     amendment_type: None,
///     original_filing_date: None,
///     is_restated: false,
///     restatement_reason: None,
///     created_at: Utc::now(),
///     updated_at: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize, Deserialize)]
#[diesel(table_name = financial_statements)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FinancialStatement {
    /// Unique identifier for this financial statement record
    /// Generated automatically using UUID v4 for global uniqueness
    pub id: Uuid,

    /// Foreign key reference to the company this statement belongs to
    /// Links to the `companies.id` field to maintain referential integrity
    pub company_id: Uuid,

    /// SEC filing type (10-K, 10-Q, 8-K, etc.)
    /// Standard SEC form types for different filing requirements
    pub filing_type: String,

    /// Form type as specified in the filing - Required, always available from SEC
    /// May differ from filing_type in some cases
    pub form_type: String,

    /// SEC accession number - unique identifier for the filing
    /// Format: CIK-YY-NNNNNN (e.g., "0000320193-23-000006")
    pub accession_number: String,

    /// Date when the filing was submitted to the SEC
    /// Used for tracking filing timelines and compliance
    pub filing_date: NaiveDate,

    /// End date of the reporting period
    /// For quarterly filings: last day of the quarter
    /// For annual filings: last day of the fiscal year
    pub period_end_date: NaiveDate,

    /// Fiscal year of the reporting period
    /// Used for organizing and filtering financial data
    pub fiscal_year: i32,

    /// Fiscal quarter of the reporting period (1, 2, 3, 4)
    /// None for annual filings (10-K)
    /// Some(1-4) for quarterly filings (10-Q)
    pub fiscal_quarter: Option<i32>,

    /// Document type as specified in the filing - Required, always available
    /// Additional classification beyond filing_type
    pub document_type: String,

    /// URL to the original filing document on SEC EDGAR - Required, always available from SEC
    /// Direct link to the HTML or PDF version of the filing
    pub document_url: String,

    /// PostgreSQL Large Object OID for XBRL file - Nullable, file may not be downloaded yet
    /// Used for storing large XBRL files as PostgreSQL Large Objects
    pub xbrl_file_oid: Option<u32>,

    /// XBRL file content stored as bytea - Nullable, file may not be downloaded yet
    /// Alternative storage method for smaller XBRL files
    pub xbrl_file_content: Option<Vec<u8>>,

    /// Size of the XBRL file in bytes - Nullable, file may not be downloaded yet
    /// Used for monitoring storage usage and processing time
    pub xbrl_file_size_bytes: Option<i64>,

    /// Whether the XBRL file is compressed - Always known, defaults to true
    /// True if file is compressed using zstd or other compression
    pub xbrl_file_compressed: bool,

    /// Compression type used - Always known, defaults to zstd
    /// Values: zstd, lz4, gzip, none
    pub xbrl_file_compression_type: CompressionType,

    /// SHA-256 hash of the XBRL file - Nullable, file may not be downloaded yet
    /// Used for integrity verification and duplicate detection
    pub xbrl_file_hash: Option<String>,

    /// Current status of XBRL processing
    /// Values: pending, processing, completed, failed
    pub xbrl_processing_status: ProcessingStatus,

    /// Error message if XBRL processing failed
    /// Detailed error information for debugging and monitoring
    pub xbrl_processing_error: Option<String>,

    /// Timestamp when XBRL processing started
    /// Used for monitoring processing performance
    pub xbrl_processing_started_at: Option<DateTime<Utc>>,

    /// Timestamp when XBRL processing completed
    /// Used for monitoring processing performance and success rates
    pub xbrl_processing_completed_at: Option<DateTime<Utc>>,

    /// Whether this is an amended filing
    /// True if this filing amends a previously filed document
    pub is_amended: bool,

    /// Type of amendment if applicable
    /// Classification of the amendment (e.g., "10-K/A", "10-Q/A")
    pub amendment_type: Option<String>,

    /// Original filing date if this is an amended filing
    /// Date of the original filing being amended
    pub original_filing_date: Option<NaiveDate>,

    /// Whether this filing contains restatements
    /// True if financial data has been restated from previous filings
    pub is_restated: bool,

    /// Reason for restatement if applicable
    /// Explanation of why financial data was restated
    pub restatement_reason: Option<String>,

    /// Timestamp when this record was first inserted into the database
    /// Used for audit trails and data lineage tracking
    pub created_at: DateTime<Utc>,

    /// Timestamp when this record was last modified
    /// Updated automatically on any field changes for change tracking
    pub updated_at: DateTime<Utc>,
}

/// **NewFinancialStatement Model**
///
/// Data transfer object for creating new financial statement records in the database.
/// Excludes auto-generated fields like `id`, `created_at`, and `updated_at`.
///
/// # Validation Rules
/// - `company_id` must reference an existing company
/// - `filing_type` must be a valid SEC form type
/// - `accession_number` must be unique and follow SEC format
/// - `filing_date` and `period_end_date` must be valid dates
/// - `fiscal_year` must be reasonable (1900-2100)
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::NewFinancialStatement;
/// use uuid::Uuid;
/// use chrono::NaiveDate;
///
/// // Create a new 10-K filing record
/// let new_statement = NewFinancialStatement {
///     company_id: Uuid::new_v4(),
///     filing_type: "10-K".to_string(),
///     form_type: Some("10-K".to_string()),
///     accession_number: "0000320193-23-000006".to_string(),
///     filing_date: NaiveDate::from_ymd_opt(2023, 11, 3).unwrap(),
///     period_end_date: NaiveDate::from_ymd_opt(2023, 9, 30).unwrap(),
///     fiscal_year: 2023,
///     fiscal_quarter: None,
///     document_type: Some("10-K".to_string()),
///     document_url: Some("https://www.sec.gov/Archives/edgar/data/320193/000032019323000006/aapl-20230930.htm".to_string()),
///     xbrl_file_path: None, // Will be set after download
///     xbrl_file_size_bytes: None,
///     xbrl_processing_status: "pending".to_string(),
///     xbrl_processing_error: None,
///     xbrl_processing_started_at: None,
///     xbrl_processing_completed_at: None,
///     is_amended: false,
///     amendment_type: None,
///     original_filing_date: None,
///     is_restated: false,
///     restatement_reason: None,
/// };
/// ```
#[derive(Debug, Clone, Insertable, Serialize, Deserialize, Validate)]
#[diesel(table_name = financial_statements)]
pub struct NewFinancialStatement {
    /// Company ID this statement belongs to
    pub company_id: Uuid,

    /// SEC filing type
    #[validate(length(min = 1, max = 10))]
    pub filing_type: String,

    /// Form type - Required, always available from SEC
    pub form_type: String,

    /// SEC accession number
    #[validate(length(min = 1, max = 20))]
    pub accession_number: String,

    /// Filing date
    pub filing_date: NaiveDate,

    /// Period end date
    pub period_end_date: NaiveDate,

    /// Fiscal year
    #[validate(range(min = 1900, max = 2100))]
    pub fiscal_year: i32,

    /// Fiscal quarter
    #[validate(range(min = 1, max = 4))]
    pub fiscal_quarter: Option<i32>,

    /// Document type - Required, always available
    pub document_type: String,

    /// Document URL - Required, always available from SEC
    #[validate(url)]
    pub document_url: String,

    /// PostgreSQL Large Object OID for XBRL file - Nullable, file may not be downloaded yet
    pub xbrl_file_oid: Option<u32>,

    /// XBRL file content stored as bytea - Nullable, file may not be downloaded yet
    pub xbrl_file_content: Option<Vec<u8>>,

    /// XBRL file size - Nullable, file may not be downloaded yet
    #[validate(range(min = 0))]
    pub xbrl_file_size_bytes: Option<i64>,

    /// Whether the XBRL file is compressed - Always known, defaults to true
    pub xbrl_file_compressed: bool,

    /// Compression type used - Always known, defaults to zstd
    pub xbrl_file_compression_type: CompressionType,

    /// SHA-256 hash of the XBRL file - Nullable, file may not be downloaded yet
    pub xbrl_file_hash: Option<String>,

    /// XBRL processing status
    pub xbrl_processing_status: ProcessingStatus,

    /// XBRL processing error
    pub xbrl_processing_error: Option<String>,

    /// XBRL processing started at
    pub xbrl_processing_started_at: Option<DateTime<Utc>>,

    /// XBRL processing completed at
    pub xbrl_processing_completed_at: Option<DateTime<Utc>>,

    /// Is amended
    pub is_amended: bool,

    /// Amendment type
    pub amendment_type: Option<String>,

    /// Original filing date
    pub original_filing_date: Option<NaiveDate>,

    /// Is restated
    pub is_restated: bool,

    /// Restatement reason
    pub restatement_reason: Option<String>,
}

/// **FinancialStatementWithCompany Model**
///
/// Extended financial statement model that includes company information.
/// Used for queries that need both statement and company data.
///
/// # Use Cases
/// - Financial statement listings with company context
/// - Search results showing statements with company names
/// - API responses for statement queries
/// - Dashboard displays with company information
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::{FinancialStatement, Company, FinancialStatementWithCompany};
///
/// // Financial statement with company information
/// let statement_with_company = FinancialStatementWithCompany {
///     statement: FinancialStatement { /* ... */ },
///     company: Company { /* ... */ },
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialStatementWithCompany {
    /// Financial statement data
    pub statement: FinancialStatement,

    /// Company information
    pub company: crate::models::company::Company,
}

/// **FinancialStatementQueryParams Model**
///
/// Parameters for querying financial statements with filtering and pagination.
/// Provides flexible filtering capabilities for statement discovery and analysis.
///
/// # Query Capabilities
/// - Company filtering by ID, CIK, or ticker
/// - Filing type filtering (10-K, 10-Q, etc.)
/// - Date range filtering for filing and period dates
/// - Fiscal year and quarter filtering
/// - Processing status filtering
/// - Amendment and restatement filtering
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::FinancialStatementQueryParams;
/// use uuid::Uuid;
/// use chrono::NaiveDate;
///
/// // Query for Apple's 10-K filings from 2023
/// let query_params = FinancialStatementQueryParams {
///     company_id: Some(Uuid::new_v4()),
///     cik: None,
///     ticker: Some("AAPL".to_string()),
///     filing_types: Some(vec!["10-K".to_string()]),
///     filing_date_start: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
///     filing_date_end: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
///     period_end_date_start: None,
///     period_end_date_end: None,
///     fiscal_year: Some(2023),
///     fiscal_quarter: None,
///     processing_status: Some("completed".to_string()),
///     is_amended: Some(false),
///     is_restated: Some(false),
///     limit: Some(10),
///     offset: Some(0),
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct FinancialStatementQueryParams {
    /// Company ID filter
    pub company_id: Option<Uuid>,

    /// CIK filter
    pub cik: Option<String>,

    /// Ticker symbol filter
    pub ticker: Option<String>,

    /// Filing types to include
    pub filing_types: Option<Vec<String>>,

    /// Filing date range start
    pub filing_date_start: Option<NaiveDate>,

    /// Filing date range end
    pub filing_date_end: Option<NaiveDate>,

    /// Period end date range start
    pub period_end_date_start: Option<NaiveDate>,

    /// Period end date range end
    pub period_end_date_end: Option<NaiveDate>,

    /// Fiscal year filter
    pub fiscal_year: Option<i32>,

    /// Fiscal quarter filter
    #[validate(range(min = 1, max = 4))]
    pub fiscal_quarter: Option<i32>,

    /// XBRL processing status filter
    pub processing_status: Option<String>,

    /// Amendment status filter
    pub is_amended: Option<bool>,

    /// Restatement status filter
    pub is_restated: Option<bool>,

    /// Maximum number of results to return
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<i64>,

    /// Number of results to skip for pagination
    #[validate(range(min = 0))]
    pub offset: Option<i64>,
}

/// **FinancialStatementSummary Model**
///
/// Summary information about financial statements for a company.
/// Used for company overviews and quick financial data access.
///
/// # Summary Information
/// - Latest filing information
/// - Available fiscal years and quarters
/// - Filing type coverage
/// - Processing status overview
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::FinancialStatementSummary;
/// use chrono::NaiveDate;
///
/// // Summary of Apple's financial statements
/// let statement_summary = FinancialStatementSummary {
///     company_id: Uuid::new_v4(),
///     latest_filing_date: Some(NaiveDate::from_ymd_opt(2023, 11, 3).unwrap()),
///     latest_fiscal_year: Some(2023),
///     latest_fiscal_quarter: Some(4),
///     available_fiscal_years: vec![2021, 2022, 2023],
///     available_filing_types: vec!["10-K".to_string(), "10-Q".to_string()],
///     total_statements: 12,
///     completed_statements: 12,
///     pending_statements: 0,
///     failed_statements: 0,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialStatementSummary {
    /// Company ID
    pub company_id: Uuid,

    /// Date of most recent filing
    pub latest_filing_date: Option<NaiveDate>,

    /// Most recent fiscal year with data
    pub latest_fiscal_year: Option<i32>,

    /// Most recent fiscal quarter with data
    pub latest_fiscal_quarter: Option<i32>,

    /// All available fiscal years
    pub available_fiscal_years: Vec<i32>,

    /// All available filing types
    pub available_filing_types: Vec<String>,

    /// Total number of statements
    pub total_statements: i32,

    /// Number of completed statements
    pub completed_statements: i32,

    /// Number of pending statements
    pub pending_statements: i32,

    /// Number of failed statements
    pub failed_statements: i32,
}

/// **XBRLProcessingStatus Enum**
///
/// Enumeration of possible XBRL processing statuses.
/// Used for type safety and status validation.
///
/// # Status Values
/// - `Pending`: Filing downloaded but not yet processed
/// - `Processing`: Currently being processed
/// - `Completed`: Successfully processed
/// - `Failed`: Processing failed with error
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::XBRLProcessingStatus;
///
/// // Check processing status
/// let status = XBRLProcessingStatus::Completed;
/// match status {
///     XBRLProcessingStatus::Completed => println!("Processing successful"),
///     XBRLProcessingStatus::Failed => println!("Processing failed"),
///     _ => println!("Still processing"),
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum XBRLProcessingStatus {
    /// Filing is pending processing
    Pending,
    /// Filing is currently being processed
    Processing,
    /// Filing has been successfully processed
    Completed,
    /// Processing failed with an error
    Failed,
}

impl std::fmt::Display for XBRLProcessingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XBRLProcessingStatus::Pending => write!(f, "pending"),
            XBRLProcessingStatus::Processing => write!(f, "processing"),
            XBRLProcessingStatus::Completed => write!(f, "completed"),
            XBRLProcessingStatus::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for XBRLProcessingStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(XBRLProcessingStatus::Pending),
            "processing" => Ok(XBRLProcessingStatus::Processing),
            "completed" => Ok(XBRLProcessingStatus::Completed),
            "failed" => Ok(XBRLProcessingStatus::Failed),
            _ => Err(format!("Invalid XBRL processing status: {}", s)),
        }
    }
}
