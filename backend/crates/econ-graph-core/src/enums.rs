//! Financial data enums for type safety and data integrity
//! These enums correspond to the PostgreSQL enum types created in the database schema

use serde::{Deserialize, Serialize};

/// Compression types for XBRL file storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "CompressionType"]
pub enum CompressionType {
    #[db_rename = "zstd"]
    Zstd,
    #[db_rename = "lz4"]
    Lz4,
    #[db_rename = "gzip"]
    Gzip,
    #[db_rename = "none"]
    None,
}

/// Processing status for XBRL files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "ProcessingStatus"]
pub enum ProcessingStatus {
    #[db_rename = "pending"]
    Pending,
    #[db_rename = "processing"]
    Processing,
    #[db_rename = "completed"]
    Completed,
    #[db_rename = "failed"]
    Failed,
}

/// Financial statement types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "StatementType"]
pub enum StatementType {
    #[db_rename = "income_statement"]
    IncomeStatement,
    #[db_rename = "balance_sheet"]
    BalanceSheet,
    #[db_rename = "cash_flow"]
    CashFlow,
    #[db_rename = "equity"]
    Equity,
}

/// Financial statement sections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "StatementSection"]
pub enum StatementSection {
    #[db_rename = "revenue"]
    Revenue,
    #[db_rename = "expenses"]
    Expenses,
    #[db_rename = "assets"]
    Assets,
    #[db_rename = "liabilities"]
    Liabilities,
    #[db_rename = "equity"]
    Equity,
    #[db_rename = "operating"]
    Operating,
    #[db_rename = "investing"]
    Investing,
    #[db_rename = "financing"]
    Financing,
}

/// Financial ratio categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "RatioCategory"]
pub enum RatioCategory {
    #[db_rename = "profitability"]
    Profitability,
    #[db_rename = "liquidity"]
    Liquidity,
    #[db_rename = "leverage"]
    Leverage,
    #[db_rename = "efficiency"]
    Efficiency,
    #[db_rename = "market"]
    Market,
    #[db_rename = "growth"]
    Growth,
}

/// Calculation methods for financial ratios
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "CalculationMethod"]
pub enum CalculationMethod {
    #[db_rename = "simple"]
    Simple,
    #[db_rename = "weighted_average"]
    WeightedAverage,
    #[db_rename = "geometric_mean"]
    GeometricMean,
    #[db_rename = "median"]
    Median,
}

/// Company comparison types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "ComparisonType"]
pub enum ComparisonType {
    #[db_rename = "industry"]
    Industry,
    #[db_rename = "sector"]
    Sector,
    #[db_rename = "size"]
    Size,
    #[db_rename = "geographic"]
    Geographic,
    #[db_rename = "custom"]
    Custom,
}

/// XBRL data types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "XbrlDataType"]
pub enum XbrlDataType {
    #[db_rename = "monetaryItemType"]
    MonetaryItemType,
    #[db_rename = "sharesItemType"]
    SharesItemType,
    #[db_rename = "stringItemType"]
    StringItemType,
    #[db_rename = "decimalItemType"]
    DecimalItemType,
    #[db_rename = "integerItemType"]
    IntegerItemType,
    #[db_rename = "booleanItemType"]
    BooleanItemType,
    #[db_rename = "dateItemType"]
    DateItemType,
    #[db_rename = "timeItemType"]
    TimeItemType,
}

/// XBRL period types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "PeriodType"]
pub enum PeriodType {
    #[db_rename = "duration"]
    Duration,
    #[db_rename = "instant"]
    Instant,
}

/// XBRL balance types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "BalanceType"]
pub enum BalanceType {
    #[db_rename = "debit"]
    Debit,
    #[db_rename = "credit"]
    Credit,
}

/// XBRL substitution groups
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "SubstitutionGroup"]
pub enum SubstitutionGroup {
    #[db_rename = "item"]
    Item,
    #[db_rename = "tuple"]
    Tuple,
}

/// XBRL processing steps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "ProcessingStep"]
pub enum ProcessingStep {
    #[db_rename = "download"]
    Download,
    #[db_rename = "parse"]
    Parse,
    #[db_rename = "validate"]
    Validate,
    #[db_rename = "store"]
    Store,
    #[db_rename = "extract"]
    Extract,
    #[db_rename = "calculate"]
    Calculate,
}

/// Annotation types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "AnnotationType"]
pub enum AnnotationType {
    #[db_rename = "comment"]
    Comment,
    #[db_rename = "question"]
    Question,
    #[db_rename = "concern"]
    Concern,
    #[db_rename = "insight"]
    Insight,
    #[db_rename = "risk"]
    Risk,
    #[db_rename = "opportunity"]
    Opportunity,
    #[db_rename = "highlight"]
    Highlight,
}

/// Annotation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "AnnotationStatus"]
pub enum AnnotationStatus {
    #[db_rename = "active"]
    Active,
    #[db_rename = "resolved"]
    Resolved,
    #[db_rename = "archived"]
    Archived,
}

/// Assignment types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "AssignmentType"]
pub enum AssignmentType {
    #[db_rename = "review"]
    Review,
    #[db_rename = "analyze"]
    Analyze,
    #[db_rename = "verify"]
    Verify,
    #[db_rename = "approve"]
    Approve,
    #[db_rename = "investigate"]
    Investigate,
}

/// Assignment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel_derive_enum::DbEnum)]
#[DieselType = "AssignmentStatus"]
pub enum AssignmentStatus {
    #[db_rename = "pending"]
    Pending,
    #[db_rename = "in_progress"]
    InProgress,
    #[db_rename = "completed"]
    Completed,
    #[db_rename = "overdue"]
    Overdue,
    #[db_rename = "cancelled"]
    Cancelled,
}