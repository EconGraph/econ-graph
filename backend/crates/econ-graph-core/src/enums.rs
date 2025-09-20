//! Financial data enums for type safety and data integrity
//! These enums correspond to the PostgreSQL enum types created in the database schema

use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Text;
use diesel::Expression;
use serde::{Deserialize, Serialize};

/// Compression types for XBRL file storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel::AsExpression)]
#[diesel(sql_type = Text)]
pub enum CompressionType {
    Zstd,
    Lz4,
    Gzip,
    None,
}

impl ToSql<Text, Pg> for CompressionType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            CompressionType::Zstd => "zstd",
            CompressionType::Lz4 => "lz4",
            CompressionType::Gzip => "gzip",
            CompressionType::None => "none",
        };
        <str as ToSql<Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<Text, Pg> for CompressionType {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "zstd" => Ok(CompressionType::Zstd),
            "lz4" => Ok(CompressionType::Lz4),
            "gzip" => Ok(CompressionType::Gzip),
            "none" => Ok(CompressionType::None),
            _ => Err(format!("Unknown compression_type value: {}", value).into()),
        }
    }
}

// Let Diesel automatically derive Queryable for single-field types
impl diesel::Queryable<Text, Pg> for CompressionType {
    type Row = Self;
    fn build(row: Self::Row) -> deserialize::Result<Self> {
        Ok(row)
    }
}

impl CompressionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionType::Zstd => "zstd",
            CompressionType::Lz4 => "lz4",
            CompressionType::Gzip => "gzip",
            CompressionType::None => "none",
        }
    }
}

/// Processing status for XBRL files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel::AsExpression)]
#[diesel(sql_type = Text)]
pub enum ProcessingStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl ToSql<Text, Pg> for ProcessingStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            ProcessingStatus::Pending => "pending",
            ProcessingStatus::Processing => "processing",
            ProcessingStatus::Completed => "completed",
            ProcessingStatus::Failed => "failed",
        };
        <str as ToSql<Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<Text, Pg> for ProcessingStatus {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "pending" => Ok(ProcessingStatus::Pending),
            "processing" => Ok(ProcessingStatus::Processing),
            "completed" => Ok(ProcessingStatus::Completed),
            "failed" => Ok(ProcessingStatus::Failed),
            _ => Err(format!("Unknown processing_status value: {}", value).into()),
        }
    }
}

impl diesel::Queryable<Text, Pg> for ProcessingStatus {
    type Row = Self;
    fn build(row: Self::Row) -> deserialize::Result<Self> {
        Ok(row)
    }
}

/// Financial statement types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel::AsExpression)]
#[diesel(sql_type = Text)]
pub enum StatementType {
    IncomeStatement,
    BalanceSheet,
    CashFlow,
    Equity,
}

impl ToSql<Text, Pg> for StatementType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            StatementType::IncomeStatement => "income_statement",
            StatementType::BalanceSheet => "balance_sheet",
            StatementType::CashFlow => "cash_flow",
            StatementType::Equity => "equity",
        };
        <str as ToSql<Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<Text, Pg> for StatementType {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "income_statement" => Ok(StatementType::IncomeStatement),
            "balance_sheet" => Ok(StatementType::BalanceSheet),
            "cash_flow" => Ok(StatementType::CashFlow),
            "equity" => Ok(StatementType::Equity),
            _ => Err(format!("Unknown statement_type value: {}", value).into()),
        }
    }
}

impl diesel::Queryable<Text, Pg> for StatementType {
    type Row = Self;
    fn build(row: Self::Row) -> deserialize::Result<Self> {
        Ok(row)
    }
}

/// Financial statement sections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel::AsExpression)]
#[diesel(sql_type = Text)]
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

impl ToSql<Text, Pg> for StatementSection {
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
        <str as ToSql<Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<Text, Pg> for StatementSection {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
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

impl diesel::Queryable<Text, Pg> for StatementSection {
    type Row = Self;
    fn build(row: Self::Row) -> deserialize::Result<Self> {
        Ok(row)
    }
}

/// Annotation types for collaborative analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel::AsExpression)]
#[diesel(sql_type = Text)]
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

impl ToSql<Text, Pg> for AnnotationType {
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
        <str as ToSql<Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<Text, Pg> for AnnotationType {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
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

impl diesel::Queryable<Text, Pg> for AnnotationType {
    type Row = Self;
    fn build(row: Self::Row) -> deserialize::Result<Self> {
        Ok(row)
    }
}

/// Annotation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel::AsExpression)]
#[diesel(sql_type = Text)]
pub enum AnnotationStatus {
    Active,
    Resolved,
    Archived,
}

impl ToSql<Text, Pg> for AnnotationStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            AnnotationStatus::Active => "active",
            AnnotationStatus::Resolved => "resolved",
            AnnotationStatus::Archived => "archived",
        };
        <str as ToSql<Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<Text, Pg> for AnnotationStatus {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        match value.as_str() {
            "active" => Ok(AnnotationStatus::Active),
            "resolved" => Ok(AnnotationStatus::Resolved),
            "archived" => Ok(AnnotationStatus::Archived),
            _ => Err(format!("Unknown annotation_status value: {}", value).into()),
        }
    }
}

impl diesel::Queryable<Text, Pg> for AnnotationStatus {
    type Row = Self;
    fn build(row: Self::Row) -> deserialize::Result<Self> {
        Ok(row)
    }
}

/// Assignment types for team workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel::AsExpression)]
#[diesel(sql_type = Text)]
pub enum AssignmentType {
    Review,
    Analyze,
    Verify,
    Approve,
    Investigate,
}

impl ToSql<Text, Pg> for AssignmentType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            AssignmentType::Review => "review",
            AssignmentType::Analyze => "analyze",
            AssignmentType::Verify => "verify",
            AssignmentType::Approve => "approve",
            AssignmentType::Investigate => "investigate",
        };
        <str as ToSql<Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<Text, Pg> for AssignmentType {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
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

impl diesel::Queryable<Text, Pg> for AssignmentType {
    type Row = Self;
    fn build(row: Self::Row) -> deserialize::Result<Self> {
        Ok(row)
    }
}

/// Assignment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel::AsExpression)]
#[diesel(sql_type = Text)]
pub enum AssignmentStatus {
    Pending,
    InProgress,
    Completed,
    Overdue,
    Cancelled,
}

impl ToSql<Text, Pg> for AssignmentStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            AssignmentStatus::Pending => "pending",
            AssignmentStatus::InProgress => "in_progress",
            AssignmentStatus::Completed => "completed",
            AssignmentStatus::Overdue => "overdue",
            AssignmentStatus::Cancelled => "cancelled",
        };
        <str as ToSql<Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<Text, Pg> for AssignmentStatus {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
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

impl diesel::Queryable<Text, Pg> for AssignmentStatus {
    type Row = Self;
    fn build(row: Self::Row) -> deserialize::Result<Self> {
        Ok(row)
    }
}

/// Processing steps for XBRL files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel::AsExpression)]
#[diesel(sql_type = Text)]
pub enum ProcessingStep {
    Download,
    Parse,
    Validate,
    Store,
    Extract,
    Calculate,
}

impl ToSql<Text, Pg> for ProcessingStep {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = match self {
            ProcessingStep::Download => "download",
            ProcessingStep::Parse => "parse",
            ProcessingStep::Validate => "validate",
            ProcessingStep::Store => "store",
            ProcessingStep::Extract => "extract",
            ProcessingStep::Calculate => "calculate",
        };
        <str as ToSql<Text, Pg>>::to_sql(value, out)
    }
}

impl FromSql<Text, Pg> for ProcessingStep {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
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

impl diesel::Queryable<Text, Pg> for ProcessingStep {
    type Row = Self;
    fn build(row: Self::Row) -> deserialize::Result<Self> {
        Ok(row)
    }
}
