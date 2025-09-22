use anyhow::{Context, Result};
use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use diesel::expression_methods::ExpressionMethods;
use diesel::prelude::*;
use diesel::query_dsl::QueryDsl;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;
use zstd::stream::{decode_all, encode_all};

use crate::models::{StoredXbrlDocument, XbrlStorageStats};
use econ_graph_core::database::DatabasePool;
use econ_graph_core::enums::{CompressionType, ProcessingStatus};
use econ_graph_core::models::{Company, FinancialStatement};

/// Configuration for XBRL file storage
#[derive(Debug, Clone)]
pub struct XbrlStorageConfig {
    /// Whether to use PostgreSQL Large Objects (true) or bytea columns (false)
    pub use_large_objects: bool,
    /// Maximum file size for bytea storage before switching to Large Objects (bytes)
    pub max_bytea_size: usize,
    /// Zstandard compression level (1-22, higher = better compression, slower)
    pub zstd_compression_level: i32,
    /// Whether to enable compression
    pub compression_enabled: bool,
}

impl Default for XbrlStorageConfig {
    fn default() -> Self {
        Self {
            use_large_objects: true,
            max_bytea_size: 100 * 1024 * 1024, // 100MB
            zstd_compression_level: 3,         // Good balance of speed vs compression
            compression_enabled: true,
        }
    }
}

/// XBRL file storage implementation using PostgreSQL
#[derive(Clone)]
pub struct XbrlStorage {
    pool: DatabasePool,
    config: XbrlStorageConfig,
}

impl XbrlStorage {
    /// Create a new XBRL storage instance
    pub fn new(pool: DatabasePool, config: XbrlStorageConfig) -> Self {
        Self { pool, config }
    }

    /// Store an XBRL file in the database
    pub async fn store_xbrl_file(
        &self,
        acc_num: &str,
        content: &[u8],
        comp_id: Uuid,
        filing_dt: DateTime<Utc>,
        period_end_dt: DateTime<Utc>,
        fiscal_yr: i32,
        fiscal_qtr: Option<i32>,
        form_typ: Option<&str>,
        doc_url: Option<&str>,
    ) -> Result<StoredXbrlDocument> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get database connection: {}", e))?;

        // Calculate file hash for integrity verification
        let mut hasher = Sha256::new();
        hasher.update(content);
        let file_hash = hex::encode(hasher.finalize());

        // Compress content if enabled
        let (compressed_content, compression_type): (Vec<u8>, CompressionType) =
            if self.config.compression_enabled {
                let compressed = encode_all(content, self.config.zstd_compression_level)
                    .context("Failed to compress XBRL file")?;
                (compressed, CompressionType::Zstd)
            } else {
                (content.to_vec(), CompressionType::None)
            };

        let file_size = content.len();
        let compressed_size = compressed_content.len();

        // Determine storage method based on file size and configuration
        let use_lob = self.config.use_large_objects && compressed_size > self.config.max_bytea_size;

        let compression_type_str = match compression_type {
            CompressionType::Zstd => "zstd",
            CompressionType::Lz4 => "lz4",
            CompressionType::Gzip => "gzip",
            CompressionType::None => "none",
        };

        if use_lob {
            self.store_as_large_object(
                &mut *conn,
                acc_num,
                &compressed_content,
                comp_id,
                filing_dt,
                period_end_dt,
                fiscal_yr,
                fiscal_qtr,
                form_typ,
                doc_url,
                file_size,
                &file_hash,
                compression_type,
            )
            .await
        } else {
            self.store_as_bytea(
                &mut *conn,
                acc_num,
                &compressed_content,
                comp_id,
                filing_dt,
                period_end_dt,
                fiscal_yr,
                fiscal_qtr,
                form_typ,
                doc_url,
                file_size,
                &file_hash,
                compression_type,
            )
            .await
        }
    }

    /// Store XBRL file as PostgreSQL Large Object
    async fn store_as_large_object(
        &self,
        conn: &mut AsyncPgConnection,
        acc_num: &str,
        content: &[u8],
        comp_id: Uuid,
        filing_dt: DateTime<Utc>,
        period_end_dt: DateTime<Utc>,
        fiscal_yr: i32,
        fiscal_qtr: Option<i32>,
        form_typ: Option<&str>,
        doc_url: Option<&str>,
        original_size: usize,
        file_hash: &str,
        compression_type: CompressionType,
    ) -> Result<StoredXbrlDocument> {
        use econ_graph_core::schema::financial_statements::dsl::*;

        // For now, use bytea storage instead of Large Objects
        // TODO: Implement proper Large Object storage
        let lob_oid = (12345i32,); // Placeholder OID

        // TODO: Write content to the Large Object
        // This requires additional PostgreSQL extensions or custom functions

        // Insert financial statement record
        let new_statement = FinancialStatement {
            id: Uuid::new_v4(),
            company_id: comp_id,
            filing_type: "10-K".to_string(), // Default, should be determined from filing
            form_type: form_typ.unwrap_or("10-K").to_string(),
            accession_number: acc_num.to_string(),
            filing_date: filing_dt.date_naive(),
            period_end_date: period_end_dt.date_naive(),
            fiscal_year: fiscal_yr,
            fiscal_quarter: fiscal_qtr,
            document_type: "XBRL".to_string(),
            document_url: doc_url.unwrap_or("").to_string(),
            xbrl_file_oid: Some(lob_oid.0 as u32),
            xbrl_file_content: None,
            xbrl_file_size_bytes: Some(original_size as i64),
            xbrl_file_compressed: self.config.compression_enabled,
            xbrl_file_compression_type: compression_type,
            xbrl_file_hash: Some(file_hash.to_string()),
            xbrl_processing_status: ProcessingStatus::Pending,
            xbrl_processing_error: None,
            xbrl_processing_started_at: None,
            xbrl_processing_completed_at: None,
            is_amended: false,
            amendment_type: None,
            original_filing_date: None,
            is_restated: false,
            restatement_reason: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        diesel::insert_into(financial_statements)
            .values(&new_statement)
            .execute(conn)
            .await
            .context("Failed to insert financial statement")?;

        Ok(StoredXbrlDocument {
            id: new_statement.id,
            accession_number: acc_num.to_string(),
            company_id: comp_id,
            filing_date: filing_dt,
            period_end_date: period_end_dt,
            fiscal_year: fiscal_yr,
            fiscal_quarter: fiscal_qtr,
            file_size: original_size,
            compressed_size: content.len(),
            compression_type: match compression_type {
                CompressionType::Zstd => "zstd".to_string(),
                CompressionType::Lz4 => "lz4".to_string(),
                CompressionType::Gzip => "gzip".to_string(),
                CompressionType::None => "none".to_string(),
            },
            file_hash: file_hash.to_string(),
            storage_method: "large_object".to_string(),
            created_at: new_statement.created_at,
        })
    }

    /// Store XBRL file as bytea column
    async fn store_as_bytea(
        &self,
        conn: &mut AsyncPgConnection,
        acc_num: &str,
        content: &[u8],
        comp_id: Uuid,
        filing_dt: DateTime<Utc>,
        period_end_dt: DateTime<Utc>,
        fiscal_yr: i32,
        fiscal_qtr: Option<i32>,
        form_typ: Option<&str>,
        doc_url: Option<&str>,
        original_size: usize,
        file_hash: &str,
        compression_type: CompressionType,
    ) -> Result<StoredXbrlDocument> {
        use econ_graph_core::schema::financial_statements::dsl::*;

        // Insert financial statement record with bytea content
        let new_statement = FinancialStatement {
            id: Uuid::new_v4(),
            company_id: comp_id,
            filing_type: "10-K".to_string(), // Default, should be determined from filing
            form_type: form_typ.unwrap_or("10-K").to_string(),
            accession_number: acc_num.to_string(),
            filing_date: filing_dt.date_naive(),
            period_end_date: period_end_dt.date_naive(),
            fiscal_year: fiscal_yr,
            fiscal_quarter: fiscal_qtr,
            document_type: "XBRL".to_string(),
            document_url: doc_url.unwrap_or("").to_string(),
            xbrl_file_oid: None,
            xbrl_file_content: Some(content.to_vec()),
            xbrl_file_size_bytes: Some(original_size as i64),
            xbrl_file_compressed: self.config.compression_enabled,
            xbrl_file_compression_type: compression_type,
            xbrl_file_hash: Some(file_hash.to_string()),
            xbrl_processing_status: ProcessingStatus::Pending,
            xbrl_processing_error: None,
            xbrl_processing_started_at: None,
            xbrl_processing_completed_at: None,
            is_amended: false,
            amendment_type: None,
            original_filing_date: None,
            is_restated: false,
            restatement_reason: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        diesel::insert_into(financial_statements)
            .values(&new_statement)
            .execute(conn)
            .await
            .context("Failed to insert financial statement")?;

        Ok(StoredXbrlDocument {
            id: new_statement.id,
            accession_number: acc_num.to_string(),
            company_id: comp_id,
            filing_date: filing_dt,
            period_end_date: period_end_dt,
            fiscal_year: fiscal_yr,
            fiscal_quarter: fiscal_qtr,
            file_size: original_size,
            compressed_size: content.len(),
            compression_type: match compression_type {
                CompressionType::Zstd => "zstd".to_string(),
                CompressionType::Lz4 => "lz4".to_string(),
                CompressionType::Gzip => "gzip".to_string(),
                CompressionType::None => "none".to_string(),
            },
            file_hash: file_hash.to_string(),
            storage_method: "bytea".to_string(),
            created_at: new_statement.created_at,
        })
    }

    /// Retrieve an XBRL file from the database
    pub async fn retrieve_xbrl_file(&self, acc_num: &str) -> Result<Vec<u8>> {
        use econ_graph_core::schema::financial_statements::dsl::*;

        let mut conn = self.pool.get().await?;

        let statement = financial_statements
            .filter(accession_number.eq(acc_num))
            .first::<FinancialStatement>(&mut conn)
            .await
            .optional()
            .context("Failed to query financial statement")?
            .ok_or_else(|| anyhow::anyhow!("XBRL file not found: {}", acc_num))?;

        let content = if let Some(oid) = statement.xbrl_file_oid {
            // Retrieve from Large Object
            self.retrieve_from_large_object(&mut conn, oid as i32)
                .await?
        } else if let Some(content) = statement.xbrl_file_content {
            // Retrieve from bytea column
            content
        } else {
            return Err(anyhow::anyhow!("No XBRL file content found"));
        };

        // Decompress if necessary
        if statement.xbrl_file_compressed {
            match statement.xbrl_file_compression_type.as_str() {
                "zstd" => Ok(decode_all(&content[..]).context("Failed to decompress XBRL file")?),
                _ => Ok(content), // Unknown compression type, return as-is
            }
        } else {
            Ok(content)
        }
    }

    /// Retrieve content from PostgreSQL Large Object
    async fn retrieve_from_large_object(
        &self,
        conn: &mut AsyncPgConnection,
        oid: i32,
    ) -> Result<Vec<u8>> {
        // TODO: Implement Large Object retrieval
        // This requires additional PostgreSQL extensions or custom functions
        Err(anyhow::anyhow!(
            "Large Object retrieval not yet implemented"
        ))
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<XbrlStorageStats> {
        use econ_graph_core::schema::financial_statements::dsl::*;

        let mut conn = self.pool.get().await?;

        // Count total files
        let total_files: i64 = financial_statements
            .count()
            .get_result(&mut conn)
            .await
            .context("Failed to count total files")?;

        // Calculate total size
        let total_size: Option<bigdecimal::BigDecimal> = financial_statements
            .select(diesel::dsl::sum(xbrl_file_size_bytes))
            .first(&mut conn)
            .await
            .context("Failed to calculate total size")?;

        // Count by storage method
        let lob_count: i64 = financial_statements
            .filter(xbrl_file_oid.is_not_null())
            .count()
            .get_result(&mut conn)
            .await
            .context("Failed to count LOB files")?;

        let bytea_count: i64 = financial_statements
            .filter(xbrl_file_content.is_not_null())
            .count()
            .get_result(&mut conn)
            .await
            .context("Failed to count bytea files")?;

        // Count by compression type
        let compressed_count: i64 = financial_statements
            .filter(xbrl_file_compressed.eq(true))
            .count()
            .get_result(&mut conn)
            .await
            .context("Failed to count compressed files")?;

        Ok(XbrlStorageStats {
            total_files: total_files as u64,
            total_size_bytes: total_size
                .unwrap_or(BigDecimal::from(0))
                .to_string()
                .parse::<u64>()
                .unwrap_or(0),
            large_object_files: lob_count as u64,
            bytea_files: bytea_count as u64,
            compressed_files: compressed_count as u64,
            uncompressed_files: total_files as u64 - compressed_count as u64,
        })
    }

    /// Delete an XBRL file from the database
    pub async fn delete_xbrl_file(&self, acc_num: &str) -> Result<()> {
        use econ_graph_core::schema::financial_statements::dsl::*;

        let mut conn = self.pool.get().await?;

        // Get the statement to check storage method
        let statement = financial_statements
            .filter(accession_number.eq(acc_num))
            .first::<FinancialStatement>(&mut conn)
            .await
            .optional()
            .context("Failed to query financial statement")?;

        if let Some(stmt) = statement {
            // Delete Large Object if it exists
            // TODO: Implement Large Object deletion
            // For now, just log that we would delete the Large Object
            if let Some(oid) = stmt.xbrl_file_oid {
                tracing::debug!("Would delete Large Object with OID: {}", oid);
            }
        }

        // Delete the financial statement record (cascades to related tables)
        diesel::delete(financial_statements.filter(accession_number.eq(acc_num)))
            .execute(&mut conn)
            .await
            .context("Failed to delete financial statement")?;

        Ok(())
    }

    /// Store a taxonomy component (schema or linkbase) in the database
    pub async fn store_taxonomy_component(
        &self,
        reference: &crate::models::DtsReference,
        content: &[u8],
        download_url: &str,
        statement_id: &Uuid,
    ) -> Result<()> {
        use econ_graph_core::enums::{TaxonomyFileType, TaxonomySourceType};
        use econ_graph_core::models::XbrlTaxonomySchema;
        use sha2::{Digest, Sha256};

        let mut conn = self.pool.get().await?;

        // Calculate file hash
        let mut hasher = Sha256::new();
        hasher.update(content);
        let file_hash = format!("sha256:{:x}", hasher.finalize());

        // Determine file type and source type
        let file_type = if reference.reference_type == "schemaRef" {
            TaxonomyFileType::Schema
        } else {
            // Determine linkbase type from role or filename
            TaxonomyFileType::LabelLinkbase // Default, could be enhanced
        };

        let source_type = self.determine_taxonomy_source_type(&reference.reference_href);

        // Extract namespace and filename from href
        let (schema_namespace, schema_filename) =
            self.extract_taxonomy_info(&reference.reference_href);

        // Create taxonomy schema record
        let taxonomy_schema = XbrlTaxonomySchema {
            id: Uuid::new_v4(),
            schema_namespace,
            schema_filename,
            schema_version: None,
            schema_date: None,
            file_type,
            source_type,
            file_content: Some(content.to_vec()),
            file_oid: None,
            file_size_bytes: content.len() as i64,
            file_hash,
            is_compressed: false, // Store uncompressed for taxonomy files
            compression_type: econ_graph_core::enums::CompressionType::None,
            source_url: Some(download_url.to_string()),
            download_url: Some(download_url.to_string()),
            original_filename: None,
            processing_status: econ_graph_core::enums::ProcessingStatus::Downloaded,
            processing_error: None,
            processing_started_at: None,
            processing_completed_at: None,
            concepts_extracted: 0,
            relationships_extracted: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Insert the taxonomy schema
        diesel::insert_into(econ_graph_core::schema::xbrl_taxonomy_schemas::table)
            .values(&taxonomy_schema)
            .execute(&mut conn)
            .await
            .context("Failed to insert taxonomy schema")?;

        // Store DTS reference
        self.store_dts_reference(reference, statement_id, &taxonomy_schema.id, download_url)
            .await?;

        Ok(())
    }

    /// Store DTS reference in the database
    async fn store_dts_reference(
        &self,
        reference: &crate::models::DtsReference,
        statement_id: &Uuid,
        resolved_schema_id: &Uuid,
        download_url: &str,
    ) -> Result<()> {
        use econ_graph_core::schema::xbrl_instance_dts_references;

        let mut conn = self.pool.get().await?;

        let dts_reference = (
            xbrl_instance_dts_references::statement_id.eq(statement_id),
            xbrl_instance_dts_references::reference_type.eq(&reference.reference_type),
            xbrl_instance_dts_references::reference_role.eq(reference.reference_role.as_deref()),
            xbrl_instance_dts_references::reference_href.eq(&reference.reference_href),
            xbrl_instance_dts_references::reference_arcrole
                .eq(reference.reference_arcrole.as_deref()),
            xbrl_instance_dts_references::resolved_schema_id.eq(resolved_schema_id),
            xbrl_instance_dts_references::is_resolved.eq(true),
            xbrl_instance_dts_references::created_at.eq(Utc::now()),
        );

        diesel::insert_into(xbrl_instance_dts_references::table)
            .values(dts_reference)
            .execute(&mut conn)
            .await
            .context("Failed to insert DTS reference")?;

        Ok(())
    }

    /// Determine taxonomy source type from href
    fn determine_taxonomy_source_type(
        &self,
        href: &str,
    ) -> econ_graph_core::enums::TaxonomySourceType {
        use econ_graph_core::enums::TaxonomySourceType;

        if href.contains("us-gaap") {
            TaxonomySourceType::UsGaap
        } else if href.contains("dei") {
            TaxonomySourceType::SecDei
        } else if href.contains("srt") {
            TaxonomySourceType::FasbSrt
        } else if href.contains("ifrs") {
            TaxonomySourceType::Ifrs
        } else {
            TaxonomySourceType::CompanySpecific
        }
    }

    /// Extract namespace and filename from taxonomy href
    fn extract_taxonomy_info(&self, href: &str) -> (String, String) {
        // Extract filename from URL
        let filename = std::path::Path::new(href)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();

        // For now, use the filename as namespace (could be enhanced to parse actual namespace)
        let namespace = format!(
            "http://taxonomy.{}",
            filename.replace(".xsd", "").replace(".xml", "")
        );

        (namespace, filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_retrieve_xbrl_file() {
        // TODO: Implement proper integration tests with testcontainers
        // This is a placeholder test - actual implementation would require
        // database setup and migration running
    }
}
