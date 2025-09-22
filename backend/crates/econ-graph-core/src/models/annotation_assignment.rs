use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::{AssignmentStatus, AssignmentType};
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
    pub assignment_type: AssignmentType,
    pub due_date: Option<DateTime<Utc>>,
    pub status: AssignmentStatus,
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
    pub assignment_type: AssignmentType,
    pub due_date: Option<DateTime<Utc>>,
    pub status: AssignmentStatus,
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
            assignment_type,
            due_date: None,
            status: AssignmentStatus::Pending,
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
            assignment_type,
            due_date: None,
            status: AssignmentStatus::Pending,
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
