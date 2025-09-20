//! Financial data enums for type safety and data integrity
//! These enums correspond to the PostgreSQL enum types created in the database schema

use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::query_builder::AsQuery;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::is_nullable::IsNullable;
use diesel::sql_types::{SingleValue, SqlType};
use diesel::Expression;
use serde::{Deserialize, Serialize};
use std::io::Write;

/// Compression types for XBRL file storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionType {
    Zstd,
    Lz4,
    Gzip,
    None,
}

/// Processing status for XBRL files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessingStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

/// Financial statement types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatementType {
    IncomeStatement,
    BalanceSheet,
    CashFlow,
    Equity,
}

/// Financial statement sections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatementSection {
    Revenue,
    Expenses,
    Assets,
    Liabilities,
    Equity,
    Operating,
    Investing,
    Financing,
}

/// Financial ratio categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RatioCategory {
    Profitability,
    Liquidity,
    Leverage,
    Efficiency,
    Market,
    Growth,
}

/// Calculation methods for financial ratios
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalculationMethod {
    Simple,
    WeightedAverage,
    GeometricMean,
    Median,
}

/// Company comparison types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonType {
    Industry,
    Sector,
    Size,
    Geographic,
    Custom,
}

/// XBRL data types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XbrlDataType {
    MonetaryItemType,
    SharesItemType,
    StringItemType,
    DecimalItemType,
    IntegerItemType,
    BooleanItemType,
    DateItemType,
    TimeItemType,
}

/// XBRL period types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeriodType {
    Duration,
    Instant,
}

/// XBRL balance types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceType {
    Debit,
    Credit,
}

/// XBRL substitution groups
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubstitutionGroup {
    Item,
    Tuple,
}

/// XBRL processing steps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessingStep {
    Download,
    Parse,
    Validate,
    Store,
    Extract,
    Calculate,
}

/// Annotation types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnnotationType {
    Comment,
    Question,
    Concern,
    Insight,
    Risk,
    Opportunity,
    Highlight,
    RevenueGrowth,
    CostConcern,
    CashFlow,
    BalanceSheet,
    OneTimeItem,
    IndustryContext,
}

/// Annotation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnnotationStatus {
    Active,
    Resolved,
    Archived,
}

/// Assignment types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentType {
    Review,
    Analyze,
    Verify,
    Approve,
    Investigate,
}

/// Assignment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentStatus {
    Pending,
    InProgress,
    Completed,
    Overdue,
    Cancelled,
}

// Manual implementation of Diesel traits for all enums
// This is needed because diesel-derive-enum is not fully compatible with our setup

// CompressionType implementation
pub struct CompressionTypeSqlType;

impl ToSql<CompressionTypeSqlType, Pg> for CompressionType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            CompressionType::Zstd => "zstd",
            CompressionType::Lz4 => "lz4",
            CompressionType::Gzip => "gzip",
            CompressionType::None => "none",
        };
        <str as ToSql<diesel::sql_types::Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<CompressionTypeSqlType, Pg> for CompressionType {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<diesel::sql_types::Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "zstd" => Ok(CompressionType::Zstd),
            "lz4" => Ok(CompressionType::Lz4),
            "gzip" => Ok(CompressionType::Gzip),
            "none" => Ok(CompressionType::None),
            _ => Err(format!("Unknown compression_type value: {}", value).into()),
        }
    }
}

impl SqlType for CompressionTypeSqlType {
    type IsNull = IsNullable;
}

impl SingleValue for CompressionTypeSqlType {}

// Implement Expression and AsExpression traits for CompressionType
impl Expression for CompressionTypeSqlType {
    type SqlType = CompressionTypeSqlType;
}

impl Expression for CompressionType {
    type SqlType = CompressionTypeSqlType;
}

// ProcessingStatus implementation
pub struct ProcessingStatusSqlType;

impl ToSql<ProcessingStatusSqlType, Pg> for ProcessingStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            ProcessingStatus::Pending => "pending",
            ProcessingStatus::Processing => "processing",
            ProcessingStatus::Completed => "completed",
            ProcessingStatus::Failed => "failed",
        };
        <str as ToSql<diesel::sql_types::Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<ProcessingStatusSqlType, Pg> for ProcessingStatus {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<diesel::sql_types::Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "pending" => Ok(ProcessingStatus::Pending),
            "processing" => Ok(ProcessingStatus::Processing),
            "completed" => Ok(ProcessingStatus::Completed),
            "failed" => Ok(ProcessingStatus::Failed),
            _ => Err(format!("Unknown processing_status value: {}", value).into()),
        }
    }
}

impl SqlType for ProcessingStatusSqlType {
    type IsNull = IsNullable;
}

impl SingleValue for ProcessingStatusSqlType {}

impl Expression for ProcessingStatusSqlType {
    type SqlType = ProcessingStatusSqlType;
}

impl Expression for ProcessingStatus {
    type SqlType = ProcessingStatusSqlType;
}

impl FromSqlRow<ProcessingStatusSqlType, Pg> for ProcessingStatus {
    fn build_from_row<'a>(row: &impl diesel::row::Row<'a, Pg>) -> deserialize::Result<Self> {
        let field = row.get(0).ok_or("Missing field")?;
        <ProcessingStatus as FromSql<ProcessingStatusSqlType, Pg>>::from_sql(field)
    }
}

// StatementType implementation
pub struct StatementTypeSqlType;

impl ToSql<StatementTypeSqlType, Pg> for StatementType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            StatementType::IncomeStatement => "income_statement",
            StatementType::BalanceSheet => "balance_sheet",
            StatementType::CashFlow => "cash_flow",
            StatementType::Equity => "equity",
        };
        <str as ToSql<diesel::sql_types::Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<StatementTypeSqlType, Pg> for StatementType {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<diesel::sql_types::Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "income_statement" => Ok(StatementType::IncomeStatement),
            "balance_sheet" => Ok(StatementType::BalanceSheet),
            "cash_flow" => Ok(StatementType::CashFlow),
            "equity" => Ok(StatementType::Equity),
            _ => Err(format!("Unknown statement_type value: {}", value).into()),
        }
    }
}

impl SqlType for StatementTypeSqlType {
    type IsNull = IsNullable;
}

impl SingleValue for StatementTypeSqlType {}

impl Expression for StatementTypeSqlType {
    type SqlType = StatementTypeSqlType;
}

impl Expression for StatementType {
    type SqlType = StatementTypeSqlType;
}

impl FromSqlRow<StatementTypeSqlType, Pg> for StatementType {
    fn build_from_row<'a>(row: &impl diesel::row::Row<'a, Pg>) -> deserialize::Result<Self> {
        let field = row.get(0).ok_or("Missing field")?;
        <StatementType as FromSql<StatementTypeSqlType, Pg>>::from_sql(field)
    }
}

// StatementSection implementation
pub struct StatementSectionSqlType;

impl ToSql<StatementSectionSqlType, Pg> for StatementSection {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            StatementSection::Revenue => "revenue",
            StatementSection::Expenses => "expenses",
            StatementSection::Assets => "assets",
            StatementSection::Liabilities => "liabilities",
            StatementSection::Equity => "equity",
            StatementSection::Operating => "operating",
            StatementSection::Investing => "investing",
            StatementSection::Financing => "financing",
        };
        <str as ToSql<diesel::sql_types::Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<StatementSectionSqlType, Pg> for StatementSection {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<diesel::sql_types::Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "revenue" => Ok(StatementSection::Revenue),
            "expenses" => Ok(StatementSection::Expenses),
            "assets" => Ok(StatementSection::Assets),
            "liabilities" => Ok(StatementSection::Liabilities),
            "equity" => Ok(StatementSection::Equity),
            "operating" => Ok(StatementSection::Operating),
            "investing" => Ok(StatementSection::Investing),
            "financing" => Ok(StatementSection::Financing),
            _ => Err(format!("Unknown statement_section value: {}", value).into()),
        }
    }
}

impl SqlType for StatementSectionSqlType {
    type IsNull = IsNullable;
}

impl SingleValue for StatementSectionSqlType {}

impl Expression for StatementSectionSqlType {
    type SqlType = StatementSectionSqlType;
}

impl Expression for StatementSection {
    type SqlType = StatementSectionSqlType;
}

impl FromSqlRow<StatementSectionSqlType, Pg> for StatementSection {
    fn build_from_row<'a>(row: &impl diesel::row::Row<'a, Pg>) -> deserialize::Result<Self> {
        let field = row.get(0).ok_or("Missing field")?;
        <StatementSection as FromSql<StatementSectionSqlType, Pg>>::from_sql(field)
    }
}

// AnnotationType implementation
pub struct AnnotationTypeSqlType;

impl ToSql<AnnotationTypeSqlType, Pg> for AnnotationType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            AnnotationType::Comment => "comment",
            AnnotationType::Question => "question",
            AnnotationType::Concern => "concern",
            AnnotationType::Insight => "insight",
            AnnotationType::Risk => "risk",
            AnnotationType::Opportunity => "opportunity",
            AnnotationType::Highlight => "highlight",
            AnnotationType::RevenueGrowth => "revenue_growth",
            AnnotationType::CostConcern => "cost_concern",
            AnnotationType::CashFlow => "cash_flow",
            AnnotationType::BalanceSheet => "balance_sheet",
            AnnotationType::OneTimeItem => "one_time_item",
            AnnotationType::IndustryContext => "industry_context",
        };
        <str as ToSql<diesel::sql_types::Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<AnnotationTypeSqlType, Pg> for AnnotationType {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<diesel::sql_types::Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "comment" => Ok(AnnotationType::Comment),
            "question" => Ok(AnnotationType::Question),
            "concern" => Ok(AnnotationType::Concern),
            "insight" => Ok(AnnotationType::Insight),
            "risk" => Ok(AnnotationType::Risk),
            "opportunity" => Ok(AnnotationType::Opportunity),
            "highlight" => Ok(AnnotationType::Highlight),
            "revenue_growth" => Ok(AnnotationType::RevenueGrowth),
            "cost_concern" => Ok(AnnotationType::CostConcern),
            "cash_flow" => Ok(AnnotationType::CashFlow),
            "balance_sheet" => Ok(AnnotationType::BalanceSheet),
            "one_time_item" => Ok(AnnotationType::OneTimeItem),
            "industry_context" => Ok(AnnotationType::IndustryContext),
            _ => Err(format!("Unknown annotation_type value: {}", value).into()),
        }
    }
}

impl SqlType for AnnotationTypeSqlType {
    type IsNull = IsNullable;
}

impl SingleValue for AnnotationTypeSqlType {}

impl Expression for AnnotationTypeSqlType {
    type SqlType = AnnotationTypeSqlType;
}

impl Expression for AnnotationType {
    type SqlType = AnnotationTypeSqlType;
}

// AnnotationStatus implementation
pub struct AnnotationStatusSqlType;

impl ToSql<AnnotationStatusSqlType, Pg> for AnnotationStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            AnnotationStatus::Active => "active",
            AnnotationStatus::Resolved => "resolved",
            AnnotationStatus::Archived => "archived",
        };
        <str as ToSql<diesel::sql_types::Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<AnnotationStatusSqlType, Pg> for AnnotationStatus {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<diesel::sql_types::Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "active" => Ok(AnnotationStatus::Active),
            "resolved" => Ok(AnnotationStatus::Resolved),
            "archived" => Ok(AnnotationStatus::Archived),
            _ => Err(format!("Unknown annotation_status value: {}", value).into()),
        }
    }
}

impl SqlType for AnnotationStatusSqlType {
    type IsNull = IsNullable;
}

impl SingleValue for AnnotationStatusSqlType {}

impl Expression for AnnotationStatusSqlType {
    type SqlType = AnnotationStatusSqlType;
}

impl Expression for AnnotationStatus {
    type SqlType = AnnotationStatusSqlType;
}

// AssignmentType implementation
pub struct AssignmentTypeSqlType;

impl ToSql<AssignmentTypeSqlType, Pg> for AssignmentType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            AssignmentType::Review => "review",
            AssignmentType::Analyze => "analyze",
            AssignmentType::Verify => "verify",
            AssignmentType::Approve => "approve",
            AssignmentType::Investigate => "investigate",
        };
        <str as ToSql<diesel::sql_types::Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<AssignmentTypeSqlType, Pg> for AssignmentType {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<diesel::sql_types::Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "review" => Ok(AssignmentType::Review),
            "analyze" => Ok(AssignmentType::Analyze),
            "verify" => Ok(AssignmentType::Verify),
            "approve" => Ok(AssignmentType::Approve),
            "investigate" => Ok(AssignmentType::Investigate),
            _ => Err(format!("Unknown assignment_type value: {}", value).into()),
        }
    }
}

impl SqlType for AssignmentTypeSqlType {
    type IsNull = IsNullable;
}

impl SingleValue for AssignmentTypeSqlType {}

impl Expression for AssignmentTypeSqlType {
    type SqlType = AssignmentTypeSqlType;
}

impl Expression for AssignmentType {
    type SqlType = AssignmentTypeSqlType;
}

// AssignmentStatus implementation
pub struct AssignmentStatusSqlType;

impl ToSql<AssignmentStatusSqlType, Pg> for AssignmentStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            AssignmentStatus::Pending => "pending",
            AssignmentStatus::InProgress => "in_progress",
            AssignmentStatus::Completed => "completed",
            AssignmentStatus::Overdue => "overdue",
            AssignmentStatus::Cancelled => "cancelled",
        };
        <str as ToSql<diesel::sql_types::Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<AssignmentStatusSqlType, Pg> for AssignmentStatus {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<diesel::sql_types::Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "pending" => Ok(AssignmentStatus::Pending),
            "in_progress" => Ok(AssignmentStatus::InProgress),
            "completed" => Ok(AssignmentStatus::Completed),
            "overdue" => Ok(AssignmentStatus::Overdue),
            "cancelled" => Ok(AssignmentStatus::Cancelled),
            _ => Err(format!("Unknown assignment_status value: {}", value).into()),
        }
    }
}

impl SqlType for AssignmentStatusSqlType {
    type IsNull = IsNullable;
}

impl SingleValue for AssignmentStatusSqlType {}

impl Expression for AssignmentStatusSqlType {
    type SqlType = AssignmentStatusSqlType;
}

impl Expression for AssignmentStatus {
    type SqlType = AssignmentStatusSqlType;
}

// ProcessingStep implementation
pub struct ProcessingStepSqlType;

impl ToSql<ProcessingStepSqlType, Pg> for ProcessingStep {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            ProcessingStep::Download => "download",
            ProcessingStep::Parse => "parse",
            ProcessingStep::Validate => "validate",
            ProcessingStep::Store => "store",
            ProcessingStep::Extract => "extract",
            ProcessingStep::Calculate => "calculate",
        };
        <str as ToSql<diesel::sql_types::Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<ProcessingStepSqlType, Pg> for ProcessingStep {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<diesel::sql_types::Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "download" => Ok(ProcessingStep::Download),
            "parse" => Ok(ProcessingStep::Parse),
            "validate" => Ok(ProcessingStep::Validate),
            "store" => Ok(ProcessingStep::Store),
            "extract" => Ok(ProcessingStep::Extract),
            "calculate" => Ok(ProcessingStep::Calculate),
            _ => Err(format!("Unknown processing_step value: {}", value).into()),
        }
    }
}

impl SqlType for ProcessingStepSqlType {
    type IsNull = IsNullable;
}

impl SingleValue for ProcessingStepSqlType {}

impl Expression for ProcessingStepSqlType {
    type SqlType = ProcessingStepSqlType;
}

impl Expression for ProcessingStep {
    type SqlType = ProcessingStepSqlType;
}
