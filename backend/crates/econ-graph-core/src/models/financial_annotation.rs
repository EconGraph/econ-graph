use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::{AnnotationStatus, AnnotationType};
use crate::schema::financial_annotations;

/// Financial annotation stored in the app database.
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = financial_annotations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FinancialAnnotation {
    pub id: Uuid,
    pub statement_id: Option<Uuid>,
    pub line_item_id: Option<Uuid>,
    pub annotation_type: AnnotationType,
    pub title: String,
    pub content: String,
    pub status: AnnotationStatus,
    pub created_by: Uuid,
    pub assigned_to: Option<Uuid>,
    pub priority: Option<i32>,
    pub tags: Option<Vec<Option<String>>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub research_anchor: Option<serde_json::Value>,
    pub research_anchor_key: Option<String>,
    pub research_cik: Option<String>,
}

/// Values needed to create a financial annotation.
#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = financial_annotations)]
pub struct NewFinancialAnnotation {
    pub statement_id: Option<Uuid>,
    pub line_item_id: Option<Uuid>,
    pub annotation_type: AnnotationType,
    pub title: String,
    pub content: String,
    pub status: AnnotationStatus,
    pub created_by: Uuid,
    pub assigned_to: Option<Uuid>,
    pub priority: Option<i32>,
    pub tags: Option<Vec<Option<String>>>,
    pub research_anchor: Option<serde_json::Value>,
    pub research_anchor_key: Option<String>,
    pub research_cik: Option<String>,
}

impl NewFinancialAnnotation {
    /// Create an active annotation, deriving a default title from its content.
    pub fn new(
        statement_id: Uuid,
        created_by: Uuid,
        content: String,
        annotation_type: AnnotationType,
    ) -> Self {
        let title = content
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(255)
            .collect();
        Self {
            statement_id: Some(statement_id),
            created_by,
            content,
            annotation_type,
            title,
            line_item_id: None,
            status: AnnotationStatus::Active,
            assigned_to: None,
            priority: None,
            tags: None,
            research_anchor: None,
            research_anchor_key: None,
            research_cik: None,
        }
    }
    /// Attach the annotation to a specific line item in its statement.
    pub fn for_line_item(
        statement_id: Uuid,
        line_item_id: Uuid,
        created_by: Uuid,
        content: String,
        annotation_type: AnnotationType,
    ) -> Self {
        Self {
            line_item_id: Some(line_item_id),
            ..Self::new(statement_id, created_by, content, annotation_type)
        }
    }
    /// Set searchable tags without introducing NULL array elements.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags.into_iter().map(Some).collect());
        self
    }
    /// Set a title distinct from the annotation content.
    pub fn with_title(mut self, title: String) -> Self {
        self.title = title;
        self
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
    pub created_by: Option<Uuid>,
    pub annotation_type: Option<AnnotationType>,
    pub status: Option<AnnotationStatus>,
    pub tags: Option<Vec<String>>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

impl Default for AnnotationFilter {
    fn default() -> Self {
        Self {
            statement_id: None,
            line_item_id: None,
            created_by: None,
            annotation_type: None,
            status: Some(AnnotationStatus::Active),
            tags: None,
            created_after: None,
            created_before: None,
        }
    }
}
