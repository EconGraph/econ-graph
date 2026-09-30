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
    /// Largest file (after compression) stored in the bytea column, in bytes. Larger files are
    /// rejected with [`XbrlFileTooLarge`] and not stored.
    pub max_bytea_size: usize,
    /// Zstandard compression level (1-22, higher = better compression, slower)
    pub zstd_compression_level: i32,
    /// Whether to enable compression
    pub compression_enabled: bool,
}

impl Default for XbrlStorageConfig {
    fn default() -> Self {
        Self {
            max_bytea_size: 100 * 1024 * 1024, // 100MB
            zstd_compression_level: 3,         // Good balance of speed vs compression
            compression_enabled: true,
        }
    }
}

/// A filing's XBRL file is over [`XbrlStorageConfig::max_bytea_size`] after compression, so it
/// was not stored. The crawler treats it as a non-retryable per-filing error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "XBRL file too large, not stored: {accession_number} is {compressed_size} bytes \
     after compression (limit {max_size})"
)]
pub struct XbrlFileTooLarge {
    pub accession_number: String,
    pub compressed_size: usize,
    pub max_size: usize,
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

    /// Store an XBRL file in the database. Fails with [`XbrlFileTooLarge`] (before touching the
    /// database) when the compressed file is over the configured limit.
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

        if compressed_size > self.config.max_bytea_size {
            return Err(XbrlFileTooLarge {
                accession_number: acc_num.to_string(),
                compressed_size,
                max_size: self.config.max_bytea_size,
            }
            .into());
        }

        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get database connection: {}", e))?;

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

        let Some(content) = statement.xbrl_file_content else {
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

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<XbrlStorageStats> {
        use econ_graph_core::schema::financial_statements::dsl::*;

        let mut conn = self.pool.get().await?;

        // All aggregates share one PostgreSQL statement snapshot, so inserts/deletes cannot
        // make the compression count exceed the total used for subtraction below.
        let (total_files, total_size, bytea_count, compressed_count): (
            i64,
            Option<BigDecimal>,
            i64,
            i64,
        ) = financial_statements
            .select((
                diesel::dsl::count_star(),
                diesel::dsl::sum(xbrl_file_size_bytes),
                diesel::dsl::count(xbrl_file_content),
                diesel::dsl::count(
                    diesel::dsl::case_when(xbrl_file_compressed.eq(true), 1_i32),
                ),
            ))
            .get_result(&mut conn)
            .await
            .context("Failed to query XBRL storage statistics")?;

        Ok(XbrlStorageStats {
            total_files: total_files as u64,
            total_size_bytes: total_size
                .unwrap_or(BigDecimal::from(0))
                .to_string()
                .parse::<u64>()
                .unwrap_or(0),
            bytea_files: bytea_count as u64,
            compressed_files: compressed_count as u64,
            uncompressed_files: total_files as u64 - compressed_count as u64,
        })
    }

    /// Delete an XBRL file from the database
    pub async fn delete_xbrl_file(&self, acc_num: &str) -> Result<()> {
        use econ_graph_core::schema::financial_statements::dsl::*;

        let mut conn = self.pool.get().await?;

        // Delete the financial statement record (cascades to related tables)
        diesel::delete(financial_statements.filter(accession_number.eq(acc_num)))
            .execute(&mut conn)
            .await
            .context("Failed to delete financial statement")?;

        Ok(())
    }
}

/// DTS (taxonomy) storage. Only the XBRL parser's DTS download uses it, so it is compiled out with
/// that path.
#[cfg(feature = "xbrl-parser")]
impl XbrlStorage {
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
        let file_hash = format!("sha256:{}", hex::encode(hasher.finalize()));

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
    #[ignore = "requires a PostgreSQL DATABASE_URL"]
    async fn storage_stats_handle_empty_mixed_and_large_totals() {
        use diesel_async::pooled_connection::AsyncDieselConnectionManager;

        // One pooled connection keeps the temporary table private to this test while calling
        // the public API. No migrations or writes to the real financial_statements table.
        let pool = DatabasePool::builder()
            .max_size(1)
            .build(AsyncDieselConnectionManager::<AsyncPgConnection>::new(
                std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            ))
            .await
            .unwrap();
        {
            let mut conn = pool.get().await.unwrap();
            diesel::sql_query(
                "CREATE TEMPORARY TABLE financial_statements (
                    xbrl_file_size_bytes bigint,
                    xbrl_file_content bytea,
                    xbrl_file_compressed boolean NOT NULL
                )",
            )
            .execute(&mut conn)
            .await
            .unwrap();
        }
        let storage = XbrlStorage::new(pool.clone(), XbrlStorageConfig::default());
        let empty = storage.get_storage_stats().await.unwrap();
        assert_eq!(empty.total_files, 0);
        assert_eq!(empty.total_size_bytes, 0);
        assert_eq!(empty.bytea_files, 0);
        assert_eq!(empty.compressed_files, 0);
        assert_eq!(empty.uncompressed_files, 0);

        {
            let mut conn = pool.get().await.unwrap();
            diesel::sql_query(
                "INSERT INTO financial_statements VALUES
                    (100, decode('01', 'hex'), true),
                    (200, NULL, false),
                    (NULL, NULL, true),
                    (0, decode('', 'hex'), false)",
            )
            .execute(&mut conn)
            .await
            .unwrap();
        }
        let mixed = storage.get_storage_stats().await.unwrap();
        assert_eq!(mixed.total_files, 4);
        assert_eq!(mixed.total_size_bytes, 300);
        assert_eq!(mixed.bytea_files, 2);
        assert_eq!(mixed.compressed_files, 2);
        assert_eq!(mixed.uncompressed_files, 2);

        {
            let mut conn = pool.get().await.unwrap();
            diesel::sql_query("TRUNCATE financial_statements")
                .execute(&mut conn)
                .await
                .unwrap();
            diesel::sql_query("INSERT INTO financial_statements VALUES (NULL, NULL, false)")
                .execute(&mut conn)
                .await
                .unwrap();
        }
        let null_size = storage.get_storage_stats().await.unwrap();
        assert_eq!(null_size.total_files, 1);
        assert_eq!(null_size.total_size_bytes, 0);
        assert_eq!(null_size.uncompressed_files, 1);

        {
            let mut conn = pool.get().await.unwrap();
            diesel::sql_query(
                "INSERT INTO financial_statements VALUES
                    (9223372036854775807, NULL, true),
                    (9223372036854775807, NULL, true)",
            )
            .execute(&mut conn)
            .await
            .unwrap();
        }
        let large = storage.get_storage_stats().await.unwrap();
        assert_eq!(large.total_size_bytes, u64::MAX - 1);
        assert_eq!(large.total_files, 3);
        assert_eq!(
            large.compressed_files + large.uncompressed_files,
            large.total_files
        );
    }

    #[tokio::test]
    async fn oversized_file_is_rejected_before_touching_the_database() {
        // The pool has no database behind it (or, with DATABASE_URL set, `comp_id` names no
        // company): any attempt to store would fail with a different error.
        let storage = XbrlStorage::new(
            econ_graph_crawler::testkit::lazy_pool(),
            XbrlStorageConfig {
                max_bytea_size: 16,
                compression_enabled: false,
                ..XbrlStorageConfig::default()
            },
        );
        let now = Utc::now();
        let err = storage
            .store_xbrl_file(
                "0000000001-24-000001",
                &[b'x'; 17],
                Uuid::new_v4(),
                now,
                now,
                2024,
                Some(1),
                Some("10-K"),
                None,
            )
            .await
            .unwrap_err();

        assert_eq!(
            err.downcast_ref::<XbrlFileTooLarge>(),
            Some(&XbrlFileTooLarge {
                accession_number: "0000000001-24-000001".to_string(),
                compressed_size: 17,
                max_size: 16,
            })
        );
        assert!(err.to_string().contains("too large, not stored"));
    }
}
