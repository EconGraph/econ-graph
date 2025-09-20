use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::financial_annotations;

/// Financial annotation for collaborative analysis
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
#[diesel(table_name = financial_annotations)]
pub struct FinancialAnnotation {
    pub id: Uuid,
    pub statement_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub author_id: Uuid,
    pub content: String,
    pub annotation_type: String,
    pub tags: Vec<String>,
    pub highlights: serde_json::Value,
    pub mentions: Vec<Uuid>,
    pub parent_annotation_id: Option<Uuid>,
    pub status: String,
    pub is_private: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New financial annotation for insertion
#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = financial_annotations)]
pub struct NewFinancialAnnotation {
    pub statement_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub author_id: Uuid,
    pub content: String,
    pub annotation_type: String,
    pub tags: Vec<String>,
    pub highlights: serde_json::Value,
    pub mentions: Vec<Uuid>,
    pub parent_annotation_id: Option<Uuid>,
    pub status: String,
    pub is_private: bool,
}

impl NewFinancialAnnotation {
    /// Create a new financial annotation
    pub fn new(
        statement_id: Uuid,
        author_id: Uuid,
        content: String,
        annotation_type: AnnotationType,
    ) -> Self {
        Self {
            statement_id,
            line_item_id: None,
            author_id,
            content,
            annotation_type: annotation_type.to_string(),
            tags: Vec::new(),
            highlights: serde_json::Value::Null,
            mentions: Vec::new(),
            parent_annotation_id: None,
            status: "active".to_string(),
            is_private: false,
        }
    }

    /// Create a line item annotation
    pub fn for_line_item(
        statement_id: Uuid,
        line_item_id: Uuid,
        author_id: Uuid,
        content: String,
        annotation_type: AnnotationType,
    ) -> Self {
        Self {
            statement_id,
            line_item_id: Some(line_item_id),
            author_id,
            content,
            annotation_type: annotation_type.to_string(),
            tags: Vec::new(),
            highlights: serde_json::Value::Null,
            mentions: Vec::new(),
            parent_annotation_id: None,
            status: "active".to_string(),
            is_private: false,
        }
    }

    /// Add tags to the annotation
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Add highlights to the annotation
    pub fn with_highlights(mut self, highlights: serde_json::Value) -> Self {
        self.highlights = highlights;
        self
    }

    /// Add mentions to the annotation
    pub fn with_mentions(mut self, mentions: Vec<Uuid>) -> Self {
        self.mentions = mentions;
        self
    }

    /// Set as a reply to another annotation
    pub fn as_reply(mut self, parent_annotation_id: Uuid) -> Self {
        self.parent_annotation_id = Some(parent_annotation_id);
        self
    }

    /// Set as private annotation
    pub fn as_private(mut self) -> Self {
        self.is_private = true;
        self
    }
}

/// Annotation types for categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnnotationType {
    Comment,
    Question,
    Concern,
    Insight,
    Risk,
    Opportunity,
    OneTimeItem,
    IndustryContext,
    RevenueGrowth,
    CostConcern,
    CashFlow,
    BalanceSheet,
    RatioAnalysis,
    PeerComparison,
    TrendAnalysis,
}

impl AnnotationType {
    pub fn to_string(&self) -> String {
        match self {
            AnnotationType::Comment => "comment".to_string(),
            AnnotationType::Question => "question".to_string(),
            AnnotationType::Concern => "concern".to_string(),
            AnnotationType::Insight => "insight".to_string(),
            AnnotationType::Risk => "risk".to_string(),
            AnnotationType::Opportunity => "opportunity".to_string(),
            AnnotationType::OneTimeItem => "one_time_item".to_string(),
            AnnotationType::IndustryContext => "industry_context".to_string(),
            AnnotationType::RevenueGrowth => "revenue_growth".to_string(),
            AnnotationType::CostConcern => "cost_concern".to_string(),
            AnnotationType::CashFlow => "cash_flow".to_string(),
            AnnotationType::BalanceSheet => "balance_sheet".to_string(),
            AnnotationType::RatioAnalysis => "ratio_analysis".to_string(),
            AnnotationType::PeerComparison => "peer_comparison".to_string(),
            AnnotationType::TrendAnalysis => "trend_analysis".to_string(),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "comment" => Some(AnnotationType::Comment),
            "question" => Some(AnnotationType::Question),
            "concern" => Some(AnnotationType::Concern),
            "insight" => Some(AnnotationType::Insight),
            "risk" => Some(AnnotationType::Risk),
            "opportunity" => Some(AnnotationType::Opportunity),
            "one_time_item" => Some(AnnotationType::OneTimeItem),
            "industry_context" => Some(AnnotationType::IndustryContext),
            "revenue_growth" => Some(AnnotationType::RevenueGrowth),
            "cost_concern" => Some(AnnotationType::CostConcern),
            "cash_flow" => Some(AnnotationType::CashFlow),
            "balance_sheet" => Some(AnnotationType::BalanceSheet),
            "ratio_analysis" => Some(AnnotationType::RatioAnalysis),
            "peer_comparison" => Some(AnnotationType::PeerComparison),
            "trend_analysis" => Some(AnnotationType::TrendAnalysis),
            _ => None,
        }
    }
}

/// Annotation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnnotationStatus {
    Active,
    Resolved,
    Archived,
}

impl AnnotationStatus {
    pub fn to_string(&self) -> String {
        match self {
            AnnotationStatus::Active => "active".to_string(),
            AnnotationStatus::Resolved => "resolved".to_string(),
            AnnotationStatus::Archived => "archived".to_string(),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "active" => Some(AnnotationStatus::Active),
            "resolved" => Some(AnnotationStatus::Resolved),
            "archived" => Some(AnnotationStatus::Archived),
            _ => None,
        }
    }
}

/// Highlight range for annotations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightRange {
    pub start: usize,
    pub end: usize,
    pub color: String,
    pub label: Option<String>,
}

/// Filter for querying annotations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationFilter {
    pub statement_id: Option<Uuid>,
    pub line_item_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub annotation_type: Option<AnnotationType>,
    pub status: Option<AnnotationStatus>,
    pub tags: Option<Vec<String>>,
    pub is_private: Option<bool>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

impl Default for AnnotationFilter {
    fn default() -> Self {
        Self {
            statement_id: None,
            line_item_id: None,
            author_id: None,
            annotation_type: None,
            status: Some(AnnotationStatus::Active),
            tags: None,
            is_private: Some(false), // Default to public annotations
            created_after: None,
            created_before: None,
        }
    }
}
