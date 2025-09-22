use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::schema::companies;

/// **Company Model**
///
/// Represents a company that files financial statements with the SEC.
/// This model stores basic company information including CIK (Central Index Key),
/// ticker symbol, and business details needed for financial data analysis.
///
/// # Use Cases
/// - Storing company metadata for SEC EDGAR filings
/// - Supporting company search and filtering functionality
/// - Enabling peer group analysis and benchmarking
/// - Providing context for financial statement analysis
///
/// # Database Schema
/// Maps to the `companies` table in PostgreSQL with comprehensive company information.
/// Indexes are maintained on `cik`, `ticker`, `name`, `industry`, and `sector` for optimal query performance.
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::Company;
/// use uuid::Uuid;
/// use chrono::Utc;
///
/// // A public company with SEC filings
/// let company = Company {
///     id: Uuid::new_v4(),
///     cik: "0000320193".to_string(), // Apple Inc.
///     ticker: Some("AAPL".to_string()),
///     name: "Apple Inc.".to_string(),
///     legal_name: Some("Apple Inc.".to_string()),
///     sic_code: Some("3571".to_string()),
///     sic_description: Some("Electronic Computers".to_string()),
///     industry: Some("Technology Hardware & Equipment".to_string()),
///     sector: Some("Technology".to_string()),
///     business_address: None,
///     mailing_address: None,
///     phone: None,
///     website: Some("https://www.apple.com".to_string()),
///     state_of_incorporation: Some("CA".to_string()),
///     state_of_incorporation_description: Some("California".to_string()),
///     fiscal_year_end: Some("09-30".to_string()),
///     entity_type: Some("Corporation".to_string()),
///     entity_size: Some("Large Accelerated Filer".to_string()),
///     is_active: true,
///     created_at: Utc::now(),
///     updated_at: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize, Deserialize)]
#[diesel(table_name = companies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Company {
    /// Unique identifier for this company record
    /// Generated automatically using UUID v4 for global uniqueness
    pub id: Uuid,

    /// SEC Central Index Key (CIK) - unique identifier for SEC filings
    /// Format: 10-digit zero-padded number (e.g., "0000320193" for Apple)
    pub cik: String,

    /// Stock ticker symbol for publicly traded companies
    /// None for private companies or companies without ticker symbols
    pub ticker: Option<String>,

    /// Company name as commonly known
    /// Used for display and search purposes
    pub name: String,

    /// Full legal company name as registered
    /// May differ from common name (e.g., "Apple Inc." vs "Apple Computer, Inc.")
    pub legal_name: Option<String>,

    /// Standard Industrial Classification (SIC) code
    /// 4-digit code identifying the company's primary business activity
    pub sic_code: Option<String>,

    /// Description of the SIC code
    /// Human-readable description of the company's business activity
    pub sic_description: Option<String>,

    /// Industry classification
    /// More specific than sector, e.g., "Technology Hardware & Equipment"
    pub industry: Option<String>,

    /// Sector classification
    /// Broad business sector, e.g., "Technology", "Healthcare", "Financial"
    pub sector: Option<String>,

    /// Company business address
    /// JSON object containing address components
    pub business_address: Option<serde_json::Value>,

    /// Company mailing address
    /// JSON object containing mailing address components
    pub mailing_address: Option<serde_json::Value>,

    /// Company phone number
    /// Primary business phone number
    pub phone: Option<String>,

    /// Company website URL
    /// Primary corporate website
    pub website: Option<String>,

    /// State of incorporation (US state code)
    /// 2-letter state code where the company is incorporated
    pub state_of_incorporation: Option<String>,

    /// Description of state of incorporation
    /// Full state name for display purposes
    pub state_of_incorporation_description: Option<String>,

    /// Fiscal year end date
    /// MM-DD format indicating when the company's fiscal year ends
    pub fiscal_year_end: Option<String>,

    /// Entity type
    /// Legal entity type (Corporation, LLC, Partnership, etc.)
    pub entity_type: Option<String>,

    /// Entity size classification
    /// SEC filing status (Large Accelerated Filer, Accelerated Filer, etc.)
    pub entity_size: Option<String>,

    /// Whether the company is currently active
    /// Used to filter out inactive or delisted companies
    pub is_active: bool,

    /// Timestamp when this record was first inserted into the database
    /// Used for audit trails and data lineage tracking
    pub created_at: DateTime<Utc>,

    /// Timestamp when this record was last modified
    /// Updated automatically on any field changes for change tracking
    pub updated_at: DateTime<Utc>,
}

/// **NewCompany Model**
///
/// Data transfer object for creating new company records in the database.
/// Excludes auto-generated fields like `id`, `created_at`, and `updated_at`.
///
/// # Validation Rules
/// - `cik` must be exactly 10 digits
/// - `name` is required and cannot be empty
/// - `ticker` must be valid format if provided
/// - `website` must be valid URL format if provided
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::NewCompany;
///
/// // Create a new company record
/// let new_company = NewCompany {
///     cik: "0000320193".to_string(),
///     ticker: Some("AAPL".to_string()),
///     name: "Apple Inc.".to_string(),
///     legal_name: Some("Apple Inc.".to_string()),
///     sic_code: Some("3571".to_string()),
///     sic_description: Some("Electronic Computers".to_string()),
///     industry: Some("Technology Hardware & Equipment".to_string()),
///     sector: Some("Technology".to_string()),
///     business_address: None,
///     mailing_address: None,
///     phone: None,
///     website: Some("https://www.apple.com".to_string()),
///     state_of_incorporation: Some("CA".to_string()),
///     state_of_incorporation_description: Some("California".to_string()),
///     fiscal_year_end: Some("09-30".to_string()),
///     entity_type: Some("Corporation".to_string()),
///     entity_size: Some("Large Accelerated Filer".to_string()),
///     is_active: true,
/// };
/// ```
#[derive(Debug, Clone, Insertable, Serialize, Deserialize, Validate)]
#[diesel(table_name = companies)]
pub struct NewCompany {
    /// SEC Central Index Key (CIK)
    #[validate(length(min = 10, max = 10))]
    pub cik: String,

    /// Stock ticker symbol
    pub ticker: Option<String>,

    /// Company name
    #[validate(length(min = 1, max = 255))]
    pub name: String,

    /// Legal company name
    pub legal_name: Option<String>,

    /// SIC code
    pub sic_code: Option<String>,

    /// SIC description
    pub sic_description: Option<String>,

    /// Industry classification
    pub industry: Option<String>,

    /// Sector classification
    pub sector: Option<String>,

    /// Business address
    pub business_address: Option<serde_json::Value>,

    /// Mailing address
    pub mailing_address: Option<serde_json::Value>,

    /// Phone number
    pub phone: Option<String>,

    /// Website URL
    #[validate(url)]
    pub website: Option<String>,

    /// State of incorporation
    pub state_of_incorporation: Option<String>,

    /// State of incorporation description
    pub state_of_incorporation_description: Option<String>,

    /// Fiscal year end
    pub fiscal_year_end: Option<String>,

    /// Entity type
    pub entity_type: Option<String>,

    /// Entity size
    pub entity_size: Option<String>,

    /// Active status
    pub is_active: bool,
}

/// **CompanyWithFinancials Model**
///
/// Extended company model that includes financial statement summary information.
/// Used for company listings and search results that need basic financial context.
///
/// # Use Cases
/// - Company search results with financial summary
/// - Peer group analysis with financial metrics
/// - Dashboard displays showing company overview
/// - API responses for company listings
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::{Company, CompanyWithFinancials};
/// use uuid::Uuid;
/// use chrono::Utc;
///
/// // Company with financial summary
/// let company_with_financials = CompanyWithFinancials {
///     company: Company { /* ... */ },
///     latest_filing_date: Some(Utc::now().date_naive()),
///     latest_fiscal_year: Some(2023),
///     latest_fiscal_quarter: Some(4),
///     total_revenue: Some(BigDecimal::from(394328000000i64)), // $394.3B
///     total_assets: Some(BigDecimal::from(352755000000i64)), // $352.8B
///     market_cap: Some(BigDecimal::from(3000000000000i64)), // $3T
///     employee_count: Some(164000),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyWithFinancials {
    /// Base company information
    pub company: Company,

    /// Date of most recent financial filing
    pub latest_filing_date: Option<NaiveDate>,

    /// Most recent fiscal year with available data
    pub latest_fiscal_year: Option<i32>,

    /// Most recent fiscal quarter with available data
    pub latest_fiscal_quarter: Option<i32>,

    /// Total revenue from most recent income statement
    pub total_revenue: Option<BigDecimal>,

    /// Total assets from most recent balance sheet
    pub total_assets: Option<BigDecimal>,

    /// Market capitalization (if available)
    pub market_cap: Option<BigDecimal>,

    /// Number of employees (if available)
    pub employee_count: Option<i32>,
}

/// **CompanySearchParams Model**
///
/// Parameters for searching and filtering companies in the database.
/// Provides flexible filtering capabilities for company discovery and analysis.
///
/// # Search Capabilities
/// - Text search across company name and ticker
/// - Industry and sector filtering
/// - Geographic filtering by state of incorporation
/// - Entity type and size filtering
/// - Active status filtering
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::CompanySearchParams;
///
/// // Search for technology companies in California
/// let search_params = CompanySearchParams {
///     query: Some("Apple".to_string()),
///     industry: Some("Technology".to_string()),
///     sector: Some("Technology".to_string()),
///     state_of_incorporation: Some("CA".to_string()),
///     entity_type: Some("Corporation".to_string()),
///     is_active: Some(true),
///     limit: Some(50),
///     offset: Some(0),
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CompanySearchParams {
    /// Text search query for company name or ticker
    pub query: Option<String>,

    /// Industry filter
    pub industry: Option<String>,

    /// Sector filter
    pub sector: Option<String>,

    /// State of incorporation filter
    pub state_of_incorporation: Option<String>,

    /// Entity type filter
    pub entity_type: Option<String>,

    /// Entity size filter
    pub entity_size: Option<String>,

    /// Active status filter
    pub is_active: Option<bool>,

    /// Maximum number of results to return
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<i64>,

    /// Number of results to skip for pagination
    #[validate(range(min = 0))]
    pub offset: Option<i64>,
}

/// **CompanyComparisonParams Model**
///
/// Parameters for comparing companies across financial metrics.
/// Enables peer group analysis and benchmarking functionality.
///
/// # Comparison Capabilities
/// - Multi-company comparison
/// - Metric selection and customization
/// - Time period specification
/// - Peer group definition
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::CompanyComparisonParams;
/// use uuid::Uuid;
///
/// // Compare Apple, Microsoft, and Google on key metrics
/// let comparison_params = CompanyComparisonParams {
///     company_ids: vec![
///         Uuid::new_v4(), // Apple
///         Uuid::new_v4(), // Microsoft
///         Uuid::new_v4(), // Google
///     ],
///     metrics: vec![
///         "revenue".to_string(),
///         "net_income".to_string(),
///         "total_assets".to_string(),
///         "roe".to_string(),
///         "current_ratio".to_string(),
///     ],
///     fiscal_year: Some(2023),
///     fiscal_quarter: Some(4),
///     include_peer_averages: true,
///     include_industry_benchmarks: true,
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CompanyComparisonParams {
    /// Company IDs to compare
    #[validate(length(min = 2, max = 20))]
    pub company_ids: Vec<Uuid>,

    /// Financial metrics to compare
    #[validate(length(min = 1, max = 50))]
    pub metrics: Vec<String>,

    /// Fiscal year for comparison
    pub fiscal_year: Option<i32>,

    /// Fiscal quarter for comparison
    #[validate(range(min = 1, max = 4))]
    pub fiscal_quarter: Option<i32>,

    /// Include peer group averages
    pub include_peer_averages: bool,

    /// Include industry benchmark data
    pub include_industry_benchmarks: bool,
}

/// **CompanyComparisonResult Model**
///
/// Result of company comparison analysis.
/// Contains comparative financial data and analysis insights.
///
/// # Analysis Features
/// - Side-by-side metric comparison
/// - Peer group rankings and percentiles
/// - Industry benchmark comparisons
/// - Trend analysis and insights
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::{Company, CompanyComparisonResult, CompanyComparisonMetric};
/// use uuid::Uuid;
/// use bigdecimal::BigDecimal;
///
/// // Comparison result with multiple companies and metrics
/// let comparison_result = CompanyComparisonResult {
///     comparison_id: Uuid::new_v4(),
///     companies: vec![/* Company objects */],
///     metrics: vec![
///         CompanyComparisonMetric {
///             metric_name: "revenue".to_string(),
///             metric_label: "Total Revenue".to_string(),
///             values: vec![
///                 BigDecimal::from(394328000000i64), // Apple
///                 BigDecimal::from(211915000000i64), // Microsoft
///                 BigDecimal::from(282836000000i64), // Google
///             ],
///             units: "USD".to_string(),
///             peer_average: Some(BigDecimal::from(296360000000i64)),
///             industry_benchmark: Some(BigDecimal::from(250000000000i64)),
///         },
///     ],
///     fiscal_year: 2023,
///     fiscal_quarter: Some(4),
///     generated_at: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyComparisonResult {
    /// Unique identifier for this comparison
    pub comparison_id: Uuid,

    /// Companies included in the comparison
    pub companies: Vec<Company>,

    /// Financial metrics compared
    pub metrics: Vec<CompanyComparisonMetric>,

    /// Fiscal year of comparison
    pub fiscal_year: i32,

    /// Fiscal quarter of comparison
    pub fiscal_quarter: Option<i32>,

    /// When this comparison was generated
    pub generated_at: DateTime<Utc>,
}

/// **CompanyComparisonMetric Model**
///
/// Individual metric within a company comparison.
/// Contains values for all companies and benchmark data.
///
/// # Metric Information
/// - Metric name and display label
/// - Values for each company
/// - Units of measurement
/// - Peer group and industry benchmarks
/// - Rankings and percentiles
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::CompanyComparisonMetric;
/// use bigdecimal::BigDecimal;
///
/// // Revenue comparison metric
/// let revenue_metric = CompanyComparisonMetric {
///     metric_name: "revenue".to_string(),
///     metric_label: "Total Revenue".to_string(),
///     values: vec![
///         BigDecimal::from(394328000000i64), // Apple
///         BigDecimal::from(211915000000i64), // Microsoft
///         BigDecimal::from(282836000000i64), // Google
///     ],
///     units: "USD".to_string(),
///     peer_average: Some(BigDecimal::from(296360000000i64)),
///     industry_benchmark: Some(BigDecimal::from(250000000000i64)),
///     rankings: Some(vec![1, 3, 2]), // Apple 1st, Google 2nd, Microsoft 3rd
///     percentiles: Some(vec![95, 75, 85]), // Percentile rankings
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyComparisonMetric {
    /// Internal metric name
    pub metric_name: String,

    /// Display label for the metric
    pub metric_label: String,

    /// Values for each company in the comparison
    pub values: Vec<BigDecimal>,

    /// Units of measurement
    pub units: String,

    /// Peer group average value
    pub peer_average: Option<BigDecimal>,

    /// Industry benchmark value
    pub industry_benchmark: Option<BigDecimal>,

    /// Rankings (1 = highest value)
    pub rankings: Option<Vec<i32>>,

    /// Percentile rankings (0-100)
    pub percentiles: Option<Vec<i32>>,
}
