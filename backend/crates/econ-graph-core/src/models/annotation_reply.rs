use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::annotation_replies;

/// Reply to a financial annotation for threaded discussions
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
#[diesel(table_name = annotation_replies)]
pub struct AnnotationReply {
    pub id: Uuid,
    pub annotation_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub mentions: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New annotation reply for insertion
#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = annotation_replies)]
pub struct NewAnnotationReply {
    pub annotation_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub mentions: Vec<Uuid>,
}

impl NewAnnotationReply {
    /// Create a new annotation reply
    pub fn new(annotation_id: Uuid, author_id: Uuid, content: String) -> Self {
        Self {
            annotation_id,
            author_id,
            content,
            mentions: Vec::new(),
        }
    }

    /// Add mentions to the reply
    pub fn with_mentions(mut self, mentions: Vec<Uuid>) -> Self {
        self.mentions = mentions;
        self
    }
}

/// Filter for querying annotation replies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationReplyFilter {
    pub annotation_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

impl Default for AnnotationReplyFilter {
    fn default() -> Self {
        Self {
            annotation_id: None,
            author_id: None,
            created_after: None,
            created_before: None,
        }
    }
}
