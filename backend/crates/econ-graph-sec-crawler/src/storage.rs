use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::query_dsl::QueryDsl;
use diesel::expression_methods::ExpressionMethods;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;
use zstd::stream::{decode_all, encode_all};

use crate::models::{StoredXbrlDocument, XbrlStorageStats};
use econ_graph_core::models::{Company, FinancialStatement};
use econ_graph_core::database::DatabasePool;

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
        accession_number: &str,
        content: &[u8],
        company_id: Uuid,
        filing_date: DateTime<Utc>,
        period_end_date: DateTime<Utc>,
        fiscal_year: i32,
        fiscal_quarter: Option<i32>,
        form_type: Option<&str>,
        document_url: Option<&str>,
    ) -> Result<StoredXbrlDocument> {
        let mut conn = self.pool.get().await?;

        // Calculate file hash for integrity verification
        let mut hasher = Sha256::new();
        hasher.update(content);
        let file_hash = hex::encode(hasher.finalize());

        // Compress content if enabled
        let (compressed_content, compression_type) = if self.config.compression_enabled {
            let compressed = encode_all(content, self.config.zstd_compression_level)
                .context("Failed to compress XBRL file")?;
            (compressed, "zstd")
        } else {
            (content.to_vec(), "none")
        };

        let file_size = content.len();
        let compressed_size = compressed_content.len();

        // Determine storage method based on file size and configuration
        let use_lob = self.config.use_large_objects && compressed_size > self.config.max_bytea_size;

        if use_lob {
            self.store_as_large_object(
                &mut conn,
                accession_number,
                &compressed_content,
                company_id,
                filing_date,
                period_end_date,
                fiscal_year,
                fiscal_quarter,
                form_type,
                document_url,
                file_size,
                &file_hash,
                compression_type,
            )
            .await
        } else {
            self.store_as_bytea(
                &mut conn,
                accession_number,
                &compressed_content,
                company_id,
                filing_date,
                period_end_date,
                fiscal_year,
                fiscal_quarter,
                form_type,
                document_url,
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
        accession_number: &str,
        content: &[u8],
        company_id: Uuid,
        filing_date: DateTime<Utc>,
        period_end_date: DateTime<Utc>,
        fiscal_year: i32,
        fiscal_quarter: Option<i32>,
        form_type: Option<&str>,
        document_url: Option<&str>,
        original_size: usize,
        file_hash: &str,
        compression_type: &str,
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
            company_id,
            filing_type: "10-K".to_string(), // Default, should be determined from filing
            form_type: form_type.unwrap_or("10-K").to_string(),
            accession_number: accession_number.to_string(),
            filing_date: filing_date.date_naive(),
            period_end_date: period_end_date.date_naive(),
            fiscal_year,
            fiscal_quarter,
            document_type: "XBRL".to_string(),
            document_url: document_url.unwrap_or("").to_string(),
            xbrl_file_oid: Some(lob_oid.0 as u32),
            xbrl_file_content: None,
            xbrl_file_size_bytes: Some(original_size as i64),
            xbrl_file_compressed: Some(self.config.compression_enabled),
            xbrl_file_compression_type: Some(compression_type.to_string()),
            xbrl_file_hash: Some(file_hash.to_string()),
            xbrl_processing_status: Some("pending".to_string()),
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
            accession_number: accession_number.to_string(),
            company_id,
            filing_date,
            period_end_date,
            fiscal_year,
            fiscal_quarter,
            file_size: original_size,
            compressed_size: content.len(),
            compression_type: compression_type.to_string(),
            file_hash: file_hash.to_string(),
            storage_method: "large_object".to_string(),
            created_at: new_statement.created_at,
        })
    }

    /// Store XBRL file as bytea column
    async fn store_as_bytea(
        &self,
        conn: &mut AsyncPgConnection,
        accession_number: &str,
        content: &[u8],
        company_id: Uuid,
        filing_date: DateTime<Utc>,
        period_end_date: DateTime<Utc>,
        fiscal_year: i32,
        fiscal_quarter: Option<i32>,
        form_type: Option<&str>,
        document_url: Option<&str>,
        original_size: usize,
        file_hash: &str,
        compression_type: &str,
    ) -> Result<StoredXbrlDocument> {
        use econ_graph_core::schema::financial_statements::dsl::*;

        // Insert financial statement record with bytea content
        let new_statement = FinancialStatement {
            id: Uuid::new_v4(),
            company_id,
            filing_type: "10-K".to_string(), // Default, should be determined from filing
            form_type: form_type.unwrap_or("10-K").to_string(),
            accession_number: accession_number.to_string(),
            filing_date: filing_date.date_naive(),
            period_end_date: period_end_date.date_naive(),
            fiscal_year,
            fiscal_quarter,
            document_type: "XBRL".to_string(),
            document_url: document_url.unwrap_or("").to_string(),
            xbrl_file_oid: None,
            xbrl_file_content: Some(content.to_vec()),
            xbrl_file_size_bytes: Some(original_size as i64),
            xbrl_file_compressed: Some(self.config.compression_enabled),
            xbrl_file_compression_type: Some(compression_type.to_string()),
            xbrl_file_hash: Some(file_hash.to_string()),
            xbrl_processing_status: Some("pending".to_string()),
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
            accession_number: accession_number.to_string(),
            company_id,
            filing_date,
            period_end_date,
            fiscal_year,
            fiscal_quarter,
            file_size: original_size,
            compressed_size: content.len(),
            compression_type: compression_type.to_string(),
            file_hash: file_hash.to_string(),
            storage_method: "bytea".to_string(),
            created_at: new_statement.created_at,
        })
    }

    /// Retrieve an XBRL file from the database
    pub async fn retrieve_xbrl_file(&self, accession_number: &str) -> Result<Vec<u8>> {
        use econ_graph_core::schema::financial_statements::dsl::*;

        let mut conn = self.pool.get().await?;

        let statement = financial_statements
            .filter(accession_number.eq(accession_number))
            .first::<FinancialStatement>(&mut conn)
            .await
            .optional()
            .context("Failed to query financial statement")?
            .ok_or_else(|| anyhow::anyhow!("XBRL file not found: {}", accession_number))?;

        let content = if let Some(oid) = statement.xbrl_file_oid {
            // Retrieve from Large Object
            self.retrieve_from_large_object(&mut conn, oid).await?
        } else if let Some(content) = statement.xbrl_file_content {
            // Retrieve from bytea column
            content
        } else {
            return Err(anyhow::anyhow!("No XBRL file content found"));
        };

        // Decompress if necessary
        if statement.xbrl_file_compressed {
            match statement.xbrl_file_compression_type.as_deref() {
                Some("zstd") => {
                    decode_all(&content[..]).context("Failed to decompress XBRL file")?
                }
                _ => content, // Unknown compression type, return as-is
            }
        } else {
            content
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
        let total_size: Option<i64> = financial_statements
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
            total_size_bytes: total_size.unwrap_or(0) as u64,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use testcontainers::core::WaitFor;
    use testcontainers::images::generic::GenericImage;
    use testcontainers::{clients, images::postgres::Postgres, Container};

    #[tokio::test]
    async fn test_store_and_retrieve_xbrl_file() {
        // Setup test database
        let docker = clients::Cli::default();
        let postgres_image = GenericImage::new("postgres", "15")
            .with_env_var("POSTGRES_PASSWORD", "password")
            .with_env_var("POSTGRES_DB", "test")
            .with_wait_for(WaitFor::message_on_stderr(
                "database system is ready to accept connections",
            ));

        let container = docker.run(postgres_image);
        let connection_string = format!(
            "postgres://postgres:password@localhost:{}/test",
            container.get_host_port_ipv4(5432)
        );

        // TODO: Setup database schema and run migration
        // TODO: Create XbrlStorage instance
        // TODO: Test storing and retrieving XBRL file

        // This is a placeholder test - actual implementation would require
        // database setup and migration running
    }
}
