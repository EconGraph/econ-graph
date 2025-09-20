use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::enums::{StatementSection, StatementType};
use crate::schema::financial_line_items;

/// **FinancialLineItem Model**
///
/// Represents an individual line item from a financial statement (income statement,
/// balance sheet, cash flow statement, or statement of equity). This model stores
/// the actual financial data extracted from XBRL filings with proper taxonomy mapping.
///
/// # Use Cases
/// - Storing individual financial statement line items from XBRL filings
/// - Supporting financial statement reconstruction and analysis
/// - Enabling cross-company financial comparisons
/// - Providing data for financial ratio calculations
///
/// # Database Schema
/// Maps to the `financial_line_items` table in PostgreSQL with comprehensive line item information.
/// Indexes are maintained on `statement_id`, `taxonomy_concept`, `statement_type`,
/// `statement_section`, `parent_concept`, and `level` for optimal query performance.
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::FinancialLineItem;
/// use uuid::Uuid;
/// use bigdecimal::BigDecimal;
/// use chrono::Utc;
///
/// // Revenue line item from income statement
/// let revenue_item = FinancialLineItem {
///     id: Uuid::new_v4(),
///     statement_id: Uuid::new_v4(),
///     taxonomy_concept: "Revenues".to_string(),
///     standard_label: Some("Revenues".to_string()),
///     custom_label: Some("Net sales".to_string()),
///     value: Some(BigDecimal::from(394328000000i64)), // $394.3B
///     unit: Some("USD".to_string()),
///     context_ref: Some("c1".to_string()),
///     segment_ref: None,
///     scenario_ref: None,
///     precision: Some(3),
///     decimals: Some(-6), // Values in millions
///     is_credit: Some(false),
///     is_debit: Some(true),
///     statement_type: "income_statement".to_string(),
///     statement_section: "revenue".to_string(),
///     parent_concept: Some("IncomeStatementAbstract".to_string()),
///     level: 1,
///     order_index: Some(1),
///     is_calculated: false,
///     calculation_formula: None,
///     created_at: Utc::now(),
///     updated_at: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize, Deserialize)]
#[diesel(table_name = financial_line_items)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FinancialLineItem {
    /// Unique identifier for this line item record
    /// Generated automatically using UUID v4 for global uniqueness
    pub id: Uuid,

    /// Foreign key reference to the financial statement this line item belongs to
    /// Links to the `financial_statements.id` field to maintain referential integrity
    pub statement_id: Uuid,

    /// XBRL taxonomy concept name
    /// Standard concept name from the taxonomy (e.g., "Revenues", "Assets")
    pub taxonomy_concept: String,

    /// Standard label from the XBRL taxonomy
    /// Official label for the concept as defined in the taxonomy
    pub standard_label: Option<String>,

    /// Custom label used by the company
    /// Company-specific label that may differ from the standard label
    pub custom_label: Option<String>,

    /// Financial value of the line item
    /// The actual numeric value from the financial statement
    pub value: Option<BigDecimal>,

    /// Unit of measurement
    /// Currency or unit (USD, shares, etc.)
    pub unit: String,

    /// XBRL context reference
    /// Reference to the context that defines the reporting period and entity
    pub context_ref: String,

    /// XBRL segment reference
    /// Reference to dimensional data (business segment, geographic region, etc.)
    pub segment_ref: Option<String>,

    /// XBRL scenario reference
    /// Reference to scenario information (actual, budget, forecast, etc.)
    pub scenario_ref: Option<String>,

    /// Decimal precision of the value
    /// Number of significant digits in the value
    pub precision: Option<i32>,

    /// Number of decimal places
    /// Negative values indicate values in thousands, millions, etc.
    pub decimals: Option<i32>,

    /// Whether this is a credit balance item
    /// True for items that increase with credits (revenues, liabilities, equity)
    pub is_credit: Option<bool>,

    /// Whether this is a debit balance item
    /// True for items that increase with debits (expenses, assets)
    pub is_debit: Option<bool>,

    /// Type of financial statement
    /// Values: income_statement, balance_sheet, cash_flow, equity
    pub statement_type: StatementType,

    /// Section within the financial statement
    /// Values: revenue, expenses, assets, liabilities, equity, operating, investing, financing
    pub statement_section: StatementSection,

    /// Parent concept in the hierarchy
    /// Reference to the parent line item for hierarchical organization
    pub parent_concept: Option<String>,

    /// Hierarchy level within the statement
    /// 0 = top level, 1 = first level of detail, etc.
    pub level: i32,

    /// Display order within the statement
    /// Used for maintaining proper statement presentation order
    pub order_index: Option<i32>,

    /// Whether this is a calculated value
    /// True for line items that are calculated from other line items
    pub is_calculated: bool,

    /// Formula used for calculation
    /// Mathematical formula if this is a calculated line item
    pub calculation_formula: Option<String>,

    /// Timestamp when this record was first inserted into the database
    /// Used for audit trails and data lineage tracking
    pub created_at: DateTime<Utc>,

    /// Timestamp when this record was last modified
    /// Updated automatically on any field changes for change tracking
    pub updated_at: DateTime<Utc>,
}

/// **NewFinancialLineItem Model**
///
/// Data transfer object for creating new financial line item records in the database.
/// Excludes auto-generated fields like `id`, `created_at`, and `updated_at`.
///
/// # Validation Rules
/// - `statement_id` must reference an existing financial statement
/// - `taxonomy_concept` is required and cannot be empty
/// - `statement_type` must be a valid statement type
/// - `statement_section` must be a valid section
/// - `level` must be non-negative
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::NewFinancialLineItem;
/// use uuid::Uuid;
/// use bigdecimal::BigDecimal;
///
/// // Create a new revenue line item
/// let new_line_item = NewFinancialLineItem {
///     statement_id: Uuid::new_v4(),
///     taxonomy_concept: "Revenues".to_string(),
///     standard_label: Some("Revenues".to_string()),
///     custom_label: Some("Net sales".to_string()),
///     value: Some(BigDecimal::from(394328000000i64)),
///     unit: Some("USD".to_string()),
///     context_ref: Some("c1".to_string()),
///     segment_ref: None,
///     scenario_ref: None,
///     precision: Some(3),
///     decimals: Some(-6),
///     is_credit: Some(false),
///     is_debit: Some(true),
///     statement_type: "income_statement".to_string(),
///     statement_section: "revenue".to_string(),
///     parent_concept: Some("IncomeStatementAbstract".to_string()),
///     level: 1,
///     order_index: Some(1),
///     is_calculated: false,
///     calculation_formula: None,
/// };
/// ```
#[derive(Debug, Clone, Insertable, Serialize, Deserialize, Validate)]
#[diesel(table_name = financial_line_items)]
pub struct NewFinancialLineItem {
    /// Statement ID this line item belongs to
    pub statement_id: Uuid,

    /// XBRL taxonomy concept name
    #[validate(length(min = 1, max = 255))]
    pub taxonomy_concept: String,

    /// Standard label
    pub standard_label: Option<String>,

    /// Custom label
    pub custom_label: Option<String>,

    /// Financial value
    pub value: Option<BigDecimal>,

    /// Unit of measurement
    pub unit: String,

    /// Context reference
    pub context_ref: String,

    /// Segment reference
    pub segment_ref: Option<String>,

    /// Scenario reference
    pub scenario_ref: Option<String>,

    /// Precision
    #[validate(range(min = 0, max = 10))]
    pub precision: Option<i32>,

    /// Decimals
    #[validate(range(min = -10, max = 10))]
    pub decimals: Option<i32>,

    /// Is credit
    pub is_credit: Option<bool>,

    /// Is debit
    pub is_debit: Option<bool>,

    /// Statement type
    pub statement_type: StatementType,

    /// Statement section
    pub statement_section: StatementSection,

    /// Parent concept
    pub parent_concept: Option<String>,

    /// Hierarchy level
    #[validate(range(min = 0, max = 10))]
    pub level: i32,

    /// Order index
    #[validate(range(min = 0))]
    pub order_index: Option<i32>,

    /// Is calculated
    pub is_calculated: bool,

    /// Calculation formula
    pub calculation_formula: Option<String>,
}

/// **FinancialLineItemWithStatement Model**
///
/// Extended financial line item model that includes statement and company information.
/// Used for queries that need complete context for line items.
///
/// # Use Cases
/// - Line item queries with full context
/// - Cross-statement analysis
/// - Company-specific line item analysis
/// - API responses with complete information
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::{FinancialLineItem, FinancialStatement, Company, FinancialLineItemWithStatement};
///
/// // Line item with complete context
/// let line_item_with_context = FinancialLineItemWithStatement {
///     line_item: FinancialLineItem { /* ... */ },
///     statement: FinancialStatement { /* ... */ },
///     company: Company { /* ... */ },
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialLineItemWithStatement {
    /// Financial line item data
    pub line_item: FinancialLineItem,

    /// Financial statement data
    pub statement: crate::models::financial_statement::FinancialStatement,

    /// Company information
    pub company: crate::models::company::Company,
}

/// **FinancialLineItemQueryParams Model**
///
/// Parameters for querying financial line items with filtering and pagination.
/// Provides flexible filtering capabilities for line item discovery and analysis.
///
/// # Query Capabilities
/// - Statement filtering by ID or company
/// - Concept filtering by taxonomy concept
/// - Statement type and section filtering
/// - Value range filtering
/// - Hierarchy level filtering
/// - Calculation status filtering
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::FinancialLineItemQueryParams;
/// use uuid::Uuid;
/// use bigdecimal::BigDecimal;
///
/// // Query for revenue line items across all companies
/// let query_params = FinancialLineItemQueryParams {
///     statement_id: None,
///     company_id: None,
///     cik: None,
///     ticker: None,
///     taxonomy_concepts: Some(vec!["Revenues".to_string()]),
///     statement_types: Some(vec!["income_statement".to_string()]),
///     statement_sections: Some(vec!["revenue".to_string()]),
///     parent_concepts: None,
///     levels: None,
///     value_min: Some(BigDecimal::from(1000000000i64)), // $1B minimum
///     value_max: None,
///     is_calculated: Some(false),
///     fiscal_year: Some(2023),
///     fiscal_quarter: None,
///     limit: Some(100),
///     offset: Some(0),
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct FinancialLineItemQueryParams {
    /// Statement ID filter
    pub statement_id: Option<Uuid>,

    /// Company ID filter
    pub company_id: Option<Uuid>,

    /// CIK filter
    pub cik: Option<String>,

    /// Ticker symbol filter
    pub ticker: Option<String>,

    /// Taxonomy concepts to include
    pub taxonomy_concepts: Option<Vec<String>>,

    /// Statement types to include
    pub statement_types: Option<Vec<String>>,

    /// Statement sections to include
    pub statement_sections: Option<Vec<String>>,

    /// Parent concepts to include
    pub parent_concepts: Option<Vec<String>>,

    /// Hierarchy levels to include
    pub levels: Option<Vec<i32>>,

    /// Minimum value filter
    pub value_min: Option<BigDecimal>,

    /// Maximum value filter
    pub value_max: Option<BigDecimal>,

    /// Calculation status filter
    pub is_calculated: bool,

    /// Fiscal year filter
    pub fiscal_year: Option<i32>,

    /// Fiscal quarter filter
    #[validate(range(min = 1, max = 4))]
    pub fiscal_quarter: Option<i32>,

    /// Maximum number of results to return
    #[validate(range(min = 1, max = 10000))]
    pub limit: Option<i64>,

    /// Number of results to skip for pagination
    #[validate(range(min = 0))]
    pub offset: Option<i64>,
}

/// **FinancialLineItemSummary Model**
///
/// Summary information about financial line items for analysis.
/// Used for aggregating and summarizing line item data.
///
/// # Summary Information
/// - Total count of line items
/// - Value aggregations (sum, average, min, max)
/// - Statement type distribution
/// - Concept coverage
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_core::models::FinancialLineItemSummary;
/// use bigdecimal::BigDecimal;
///
/// // Summary of revenue line items
/// let line_item_summary = FinancialLineItemSummary {
///     total_count: 150,
///     total_value: Some(BigDecimal::from(5000000000000i64)), // $5T
///     average_value: Some(BigDecimal::from(33333333333i64)), // $33.3B
///     min_value: Some(BigDecimal::from(1000000i64)), // $1M
///     max_value: Some(BigDecimal::from(500000000000i64)), // $500B
///     statement_type_distribution: vec![
///         ("income_statement".to_string(), 75),
///         ("balance_sheet".to_string(), 50),
///         ("cash_flow".to_string(), 25),
///     ],
///     concept_coverage: 45, // 45 unique concepts
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialLineItemSummary {
    /// Total number of line items
    pub total_count: i32,

    /// Sum of all values
    pub total_value: Option<BigDecimal>,

    /// Average value
    pub average_value: Option<BigDecimal>,

    /// Minimum value
    pub min_value: Option<BigDecimal>,

    /// Maximum value
    pub max_value: Option<BigDecimal>,

    /// Distribution by statement type
    pub statement_type_distribution: Vec<(String, i32)>,

    /// Number of unique concepts
    pub concept_coverage: i32,
}
