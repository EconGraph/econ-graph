use chrono::{DateTime, Utc};
use diesel::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::{CompressionType, ProcessingStatus, TaxonomyFileType, TaxonomySourceType};
use crate::schema::xbrl_taxonomy_schemas;

/// **XBRL Taxonomy Schema Model**
///
/// Represents a stored XBRL taxonomy schema file (.xsd) with metadata.
/// This model supports the DTS (Discoverable Taxonomy Set) architecture
/// for proper XBRL parsing and fact extraction.
///
/// # Use Cases
/// - Storing downloaded XBRL taxonomy schema files
/// - Tracking schema processing status and metadata
/// - Supporting DTS dependency resolution
/// - Enabling Arelle integration with local taxonomy files
///
/// # Example
/// ```rust
/// use chrono::Utc;
/// use uuid::Uuid;
///
/// let schema = XbrlTaxonomySchema {
///     id: Uuid::new_v4(),
///     schema_namespace: "http://www.apple.com/20250628".to_string(),
///     schema_filename: "aapl-20250628.xsd".to_string(),
///     schema_version: Some("2025-06-28".to_string()),
///     schema_date: Some(chrono::NaiveDate::from_ymd_opt(2025, 6, 28).unwrap()),
///     file_type: TaxonomyFileType::Schema,
///     source_type: TaxonomySourceType::CompanySpecific,
///     file_content: None,
///     file_oid: Some(12345),
///     file_size_bytes: 2048576,
///     file_hash: "sha256:abc123...".to_string(),
///     is_compressed: true,
///     compression_type: CompressionType::Zstd,
///     source_url: Some("https://www.sec.gov/...".to_string()),
///     download_url: Some("https://www.sec.gov/...".to_string()),
///     original_filename: Some("aapl-20250628.xsd".to_string()),
///     processing_status: ProcessingStatus::Downloaded,
///     processing_error: None,
///     processing_started_at: None,
///     processing_completed_at: None,
///     concepts_extracted: 0,
///     relationships_extracted: 0,
///     created_at: Utc::now(),
///     updated_at: Utc::now(),
/// };
/// ```

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize, Deserialize)]
#[diesel(table_name = xbrl_taxonomy_schemas)]
pub struct XbrlTaxonomySchema {
    pub id: Uuid,
    pub schema_namespace: String,
    pub schema_filename: String,
    pub schema_version: Option<String>,
    pub schema_date: Option<chrono::NaiveDate>,
    pub file_type: TaxonomyFileType,
    pub source_type: TaxonomySourceType,
    pub file_content: Option<Vec<u8>>,
    pub file_oid: Option<i32>,
    pub file_size_bytes: i64,
    pub file_hash: String,
    pub is_compressed: bool,
    pub compression_type: CompressionType,
    pub source_url: Option<String>,
    pub download_url: Option<String>,
    pub original_filename: Option<String>,
    pub processing_status: ProcessingStatus,
    pub processing_error: Option<String>,
    pub processing_started_at: Option<DateTime<Utc>>,
    pub processing_completed_at: Option<DateTime<Utc>>,
    pub concepts_extracted: i32,
    pub relationships_extracted: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl XbrlTaxonomySchema {
    /// Create a new taxonomy schema record for insertion
    pub fn new(
        schema_namespace: String,
        schema_filename: String,
        source_type: TaxonomySourceType,
        file_size_bytes: i64,
        file_hash: String,
        source_url: Option<String>,
        download_url: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            schema_namespace,
            schema_filename,
            schema_version: None,
            schema_date: None,
            file_type: TaxonomyFileType::Schema,
            source_type,
            file_content: None,
            file_oid: None,
            file_size_bytes,
            file_hash,
            is_compressed: true,
            compression_type: CompressionType::Zstd,
            source_url,
            download_url,
            original_filename: None,
            processing_status: ProcessingStatus::Downloaded,
            processing_error: None,
            processing_started_at: None,
            processing_completed_at: None,
            concepts_extracted: 0,
            relationships_extracted: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Check if the schema file is stored as a large object
    pub fn is_large_object(&self) -> bool {
        self.file_oid.is_some() && self.file_content.is_none()
    }

    /// Check if the schema file is stored as bytea
    pub fn is_bytea(&self) -> bool {
        self.file_content.is_some() && self.file_oid.is_none()
    }

    /// Get the effective filename for display
    pub fn display_filename(&self) -> String {
        self.original_filename
            .clone()
            .unwrap_or_else(|| self.schema_filename.clone())
    }
}
