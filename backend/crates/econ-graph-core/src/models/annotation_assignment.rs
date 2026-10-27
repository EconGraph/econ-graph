use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::{AssignmentStatus, AssignmentType};
use crate::schema::annotation_assignments;

/// Assignment of a financial annotation to a team member.
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = annotation_assignments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AnnotationAssignment {
    pub id: Uuid,
    pub annotation_id: Uuid,
    pub assigned_to: Uuid,
    pub assigned_by: Uuid,
    pub assignment_type: AssignmentType,
    pub status: AssignmentStatus,
    pub due_date: Option<DateTime<Utc>>,
    pub instructions: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Values needed to assign an existing annotation.
#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = annotation_assignments)]
pub struct NewAnnotationAssignment {
    pub annotation_id: Uuid,
    pub assigned_to: Uuid,
    pub assigned_by: Uuid,
    pub assignment_type: AssignmentType,
    pub status: AssignmentStatus,
    pub due_date: Option<DateTime<Utc>>,
    pub instructions: Option<String>,
}

impl NewAnnotationAssignment {
    /// Create a pending assignment for an existing financial annotation.
    pub fn new(
        annotation_id: Uuid,
        assigned_to: Uuid,
        assigned_by: Uuid,
        assignment_type: AssignmentType,
    ) -> Self {
        Self {
            annotation_id,
            assigned_to,
            assigned_by,
            assignment_type,
            status: AssignmentStatus::Pending,
            due_date: None,
            instructions: None,
        }
    }
    /// Set the assignment deadline.
    pub fn with_due_date(mut self, due_date: DateTime<Utc>) -> Self {
        self.due_date = Some(due_date);
        self
    }
    /// Add instructions for the assignee.
    pub fn with_notes(mut self, notes: String) -> Self {
        self.instructions = Some(notes);
        self
    }
}

/// Filter for querying annotation assignments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationAssignmentFilter {
    pub annotation_id: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub assigned_by: Option<Uuid>,
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
            annotation_id: None,
            assigned_to: None,
            assigned_by: None,
            assignment_type: None,
            status: None,
            due_after: None,
            due_before: None,
            created_after: None,
            created_before: None,
        }
    }
}
