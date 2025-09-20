use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::annotation_assignments;

/// Assignment for team workflow management
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
#[diesel(table_name = annotation_assignments)]
pub struct AnnotationAssignment {
    pub id: Uuid,
    pub statement_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub assignee_id: Uuid,
    pub assigner_id: Uuid,
    pub assignment_type: String,
    pub due_date: Option<DateTime<Utc>>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New annotation assignment for insertion
#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = annotation_assignments)]
pub struct NewAnnotationAssignment {
    pub statement_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub assignee_id: Uuid,
    pub assigner_id: Uuid,
    pub assignment_type: String,
    pub due_date: Option<DateTime<Utc>>,
    pub status: String,
    pub notes: Option<String>,
}

impl NewAnnotationAssignment {
    /// Create a new annotation assignment
    pub fn new(
        statement_id: Uuid,
        assignee_id: Uuid,
        assigner_id: Uuid,
        assignment_type: AssignmentType,
    ) -> Self {
        Self {
            statement_id,
            line_item_id: None,
            assignee_id,
            assigner_id,
            assignment_type: assignment_type.to_string(),
            due_date: None,
            status: "pending".to_string(),
            notes: None,
        }
    }

    /// Create a line item assignment
    pub fn for_line_item(
        statement_id: Uuid,
        line_item_id: Uuid,
        assignee_id: Uuid,
        assigner_id: Uuid,
        assignment_type: AssignmentType,
    ) -> Self {
        Self {
            statement_id,
            line_item_id: Some(line_item_id),
            assignee_id,
            assigner_id,
            assignment_type: assignment_type.to_string(),
            due_date: None,
            status: "pending".to_string(),
            notes: None,
        }
    }

    /// Set due date for the assignment
    pub fn with_due_date(mut self, due_date: DateTime<Utc>) -> Self {
        self.due_date = Some(due_date);
        self
    }

    /// Add notes to the assignment
    pub fn with_notes(mut self, notes: String) -> Self {
        self.notes = Some(notes);
        self
    }
}

/// Assignment types for different workflow tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssignmentType {
    Review,
    Analyze,
    Verify,
    Comment,
    Approve,
    Investigate,
    Compare,
    Benchmark,
    RiskAssessment,
    TrendAnalysis,
    RatioAnalysis,
    PeerComparison,
}

impl AssignmentType {
    pub fn to_string(&self) -> String {
        match self {
            AssignmentType::Review => "review".to_string(),
            AssignmentType::Analyze => "analyze".to_string(),
            AssignmentType::Verify => "verify".to_string(),
            AssignmentType::Comment => "comment".to_string(),
            AssignmentType::Approve => "approve".to_string(),
            AssignmentType::Investigate => "investigate".to_string(),
            AssignmentType::Compare => "compare".to_string(),
            AssignmentType::Benchmark => "benchmark".to_string(),
            AssignmentType::RiskAssessment => "risk_assessment".to_string(),
            AssignmentType::TrendAnalysis => "trend_analysis".to_string(),
            AssignmentType::RatioAnalysis => "ratio_analysis".to_string(),
            AssignmentType::PeerComparison => "peer_comparison".to_string(),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "review" => Some(AssignmentType::Review),
            "analyze" => Some(AssignmentType::Analyze),
            "verify" => Some(AssignmentType::Verify),
            "comment" => Some(AssignmentType::Comment),
            "approve" => Some(AssignmentType::Approve),
            "investigate" => Some(AssignmentType::Investigate),
            "compare" => Some(AssignmentType::Compare),
            "benchmark" => Some(AssignmentType::Benchmark),
            "risk_assessment" => Some(AssignmentType::RiskAssessment),
            "trend_analysis" => Some(AssignmentType::TrendAnalysis),
            "ratio_analysis" => Some(AssignmentType::RatioAnalysis),
            "peer_comparison" => Some(AssignmentType::PeerComparison),
            _ => None,
        }
    }
}

/// Assignment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssignmentStatus {
    Pending,
    InProgress,
    Completed,
    Overdue,
    Cancelled,
}

impl AssignmentStatus {
    pub fn to_string(&self) -> String {
        match self {
            AssignmentStatus::Pending => "pending".to_string(),
            AssignmentStatus::InProgress => "in_progress".to_string(),
            AssignmentStatus::Completed => "completed".to_string(),
            AssignmentStatus::Overdue => "overdue".to_string(),
            AssignmentStatus::Cancelled => "cancelled".to_string(),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(AssignmentStatus::Pending),
            "in_progress" => Some(AssignmentStatus::InProgress),
            "completed" => Some(AssignmentStatus::Completed),
            "overdue" => Some(AssignmentStatus::Overdue),
            "cancelled" => Some(AssignmentStatus::Cancelled),
            _ => None,
        }
    }
}

/// Filter for querying annotation assignments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationAssignmentFilter {
    pub statement_id: Option<Uuid>,
    pub line_item_id: Option<Uuid>,
    pub assignee_id: Option<Uuid>,
    pub assigner_id: Option<Uuid>,
    pub assignment_type: Option<AssignmentType>,
    pub status: Option<AssignmentStatus>,
    pub due_after: Option<DateTime<Utc>>,
    pub due_before: Option<DateTime<Utc>>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

impl Default for AnnotationAssignmentFilter {
    fn default() -> Self {
        Self {
            statement_id: None,
            line_item_id: None,
            assignee_id: None,
            assigner_id: None,
            assignment_type: None,
            status: None,
            due_after: None,
            due_before: None,
            created_after: None,
            created_before: None,
        }
    }
}
