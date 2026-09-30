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
        use diesel_async::AsyncConnection;
        use econ_graph_core::enums::{TaxonomyFileType, TaxonomySourceType};
        use econ_graph_core::models::XbrlTaxonomySchema;
        use sha2::{Digest, Sha256};

        // Calculate file hash
        let mut hasher = Sha256::new();
        hasher.update(content);
        // The database hash column holds the 64 hex digits, without an algorithm prefix.
        let file_hash = hex::encode(hasher.finalize());

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

        // Hold one connection only for the writes, and roll back the schema if its
        // DTS reference cannot be stored.
        let mut conn = self.pool.get().await?;
        conn.transaction::<(), anyhow::Error, _>(async move |conn| {
            diesel::insert_into(econ_graph_core::schema::xbrl_taxonomy_schemas::table)
                .values(&taxonomy_schema)
                .execute(conn)
                .await
                .context("Failed to insert taxonomy schema")?;

            self.store_dts_reference(conn, reference, statement_id, &taxonomy_schema.id)
                .await
        })
        .await
    }

    /// Store DTS reference in the database
    async fn store_dts_reference(
        &self,
        conn: &mut AsyncPgConnection,
        reference: &crate::models::DtsReference,
        statement_id: &Uuid,
        resolved_schema_id: &Uuid,
    ) -> Result<()> {
        use econ_graph_core::schema::xbrl_instance_dts_references;

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
            .execute(conn)
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

    #[cfg(feature = "xbrl-parser")]
    mod taxonomy {
        use super::*;
        use diesel_async::pooled_connection::AsyncDieselConnectionManager;
        use econ_graph_core::schema::{
            companies, xbrl_instance_dts_references, xbrl_taxonomy_schemas,
        };
        use econ_graph_core::test_utils::TestContainer;
        use std::time::Duration;

        fn reference() -> crate::models::DtsReference {
            crate::models::DtsReference {
                reference_type: "schemaRef".to_string(),
                reference_role: None,
                reference_href: "https://example.com/atomic-storage.xsd".to_string(),
                reference_arcrole: None,
            }
        }

        #[tokio::test]
        #[serial_test::serial]
        async fn taxonomy_schema_rolls_back_when_reference_insert_fails() {
            let container = TestContainer::new().await;
            container.clean_database().await.unwrap();
            let pool = container.pool().clone();
            let storage = XbrlStorage::new(pool.clone(), XbrlStorageConfig::default());
            let reference = reference();

            // No statement exists for this ID: only the second insert should fail.
            let error = storage
                .store_taxonomy_component(
                    &reference,
                    b"<schema/>",
                    &reference.reference_href,
                    &Uuid::new_v4(),
                )
                .await
                .unwrap_err();
            assert!(error.to_string().contains("Failed to insert DTS reference"));
            assert!(matches!(
                error.downcast_ref::<diesel::result::Error>(),
                Some(diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::ForeignKeyViolation,
                    _
                ))
            ));

            let mut conn = pool.get().await.unwrap();
            let schemas: i64 = xbrl_taxonomy_schemas::table
                .count()
                .get_result(&mut conn)
                .await
                .unwrap();
            let references: i64 = xbrl_instance_dts_references::table
                .count()
                .get_result(&mut conn)
                .await
                .unwrap();
            assert_eq!(
                schemas, 0,
                "failed reference must not leave an orphan schema"
            );
            assert_eq!(references, 0);
        }

        #[tokio::test]
        #[serial_test::serial]
        async fn taxonomy_storage_succeeds_with_one_pool_connection() {
            let container = TestContainer::new().await;
            container.clean_database().await.unwrap();
            let database_url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://localhost/econ_graph_test".to_string());
            let pool = DatabasePool::builder()
                .max_size(1)
                .connection_timeout(Duration::from_secs(2))
                .build(AsyncDieselConnectionManager::<AsyncPgConnection>::new(
                    database_url,
                ))
                .await
                .unwrap();
            let company_id = Uuid::new_v4();
            {
                let mut conn = pool.get().await.unwrap();
                diesel::insert_into(companies::table)
                    .values((
                        companies::id.eq(company_id),
                        companies::cik.eq("0009999901"),
                        companies::name.eq("Taxonomy storage test"),
                        companies::is_active.eq(true),
                        companies::created_at.eq(Utc::now()),
                        companies::updated_at.eq(Utc::now()),
                    ))
                    .execute(&mut conn)
                    .await
                    .unwrap();
            }
            let storage = XbrlStorage::new(pool.clone(), XbrlStorageConfig::default());
            let now = Utc::now();
            let statement = storage
                .store_xbrl_file(
                    "0009999901-24-000001",
                    b"<xbrl/>",
                    company_id,
                    now,
                    now,
                    2024,
                    None,
                    Some("10-K"),
                    None,
                )
                .await
                .unwrap();
            let reference = reference();
            tokio::time::timeout(
                Duration::from_secs(5),
                storage.store_taxonomy_component(
                    &reference,
                    b"<schema/>",
                    &reference.reference_href,
                    &statement.id,
                ),
            )
            .await
            .expect("taxonomy storage must not wait for a second pool connection")
            .unwrap();

            let mut conn = pool.get().await.unwrap();
            let (schema_id, file_hash): (Uuid, String) = xbrl_taxonomy_schemas::table
                .select((xbrl_taxonomy_schemas::id, xbrl_taxonomy_schemas::file_hash))
                .get_result(&mut conn)
                .await
                .unwrap();
            assert_eq!(file_hash.len(), 64);
            assert_eq!(file_hash, hex::encode(Sha256::digest(b"<schema/>")));
            let resolved: (Option<Uuid>, bool) = xbrl_instance_dts_references::table
                .filter(xbrl_instance_dts_references::statement_id.eq(statement.id))
                .select((
                    xbrl_instance_dts_references::resolved_schema_id,
                    xbrl_instance_dts_references::is_resolved,
                ))
                .get_result(&mut conn)
                .await
                .unwrap();
            assert_eq!(resolved, (Some(schema_id), true));
        }
    }
}
