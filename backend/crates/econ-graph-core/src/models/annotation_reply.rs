use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::AnnotationStatus;
use crate::schema::annotation_replies;

/// Reply to a financial annotation, optionally nested beneath another reply.
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = annotation_replies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AnnotationReply {
    pub id: Uuid,
    pub annotation_id: Uuid,
    pub parent_reply_id: Option<Uuid>,
    pub content: String,
    pub status: AnnotationStatus,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Values needed to post a reply.
#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = annotation_replies)]
pub struct NewAnnotationReply {
    pub annotation_id: Uuid,
    pub parent_reply_id: Option<Uuid>,
    pub content: String,
    pub status: AnnotationStatus,
    pub created_by: Uuid,
}

impl NewAnnotationReply {
    /// Create an active reply to an annotation.
    pub fn new(annotation_id: Uuid, created_by: Uuid, content: String) -> Self {
        Self {
            annotation_id,
            created_by,
            content,
            parent_reply_id: None,
            status: AnnotationStatus::Active,
        }
    }
    /// Nest this reply beneath an existing reply.
    pub fn with_parent(mut self, parent_reply_id: Uuid) -> Self {
        self.parent_reply_id = Some(parent_reply_id);
        self
    }
}

/// Filter for querying annotation replies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationReplyFilter {
    pub annotation_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

impl Default for AnnotationReplyFilter {
    fn default() -> Self {
        Self {
            annotation_id: None,
            created_by: None,
            created_after: None,
            created_before: None,
        }
    }
}
