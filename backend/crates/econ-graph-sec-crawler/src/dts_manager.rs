use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::database::DatabasePool;
use econ_graph_core::enums::{TaxonomyFileType, TaxonomySourceType};
use econ_graph_core::models::XbrlTaxonomySchema;
use econ_graph_core::schema::{xbrl_taxonomy_linkbases, xbrl_taxonomy_schemas};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

/// **DTS Manager**
///
/// Manages Discoverable Taxonomy Set (DTS) files for XBRL parsing.
/// Handles downloading, caching, and resolving taxonomy dependencies.
#[derive(Debug)]
pub struct DtsManager {
    pool: DatabasePool,
    cache_dir: PathBuf,
}

impl DtsManager {
    /// Create a new DTS manager
    pub fn new(pool: DatabasePool, cache_dir: PathBuf) -> Self {
        Self { pool, cache_dir }
    }

    /// Get the taxonomy cache directory path
    pub fn get_cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Resolve DTS references for an XBRL instance
    pub async fn resolve_dts_for_instance(
        &self,
        instance_file_path: &Path,
        statement_id: Uuid,
    ) -> Result<Vec<DtsResolution>> {
        let mut resolutions = Vec::new();

        // Parse the XBRL instance to find DTS references
        let dts_references = self
            .extract_dts_references_from_instance(instance_file_path)
            .await?;

        for reference in dts_references {
            let resolution = self.resolve_dts_reference(&reference, statement_id).await?;
            resolutions.push(resolution);
        }

        Ok(resolutions)
    }

    /// Extract DTS references from an XBRL instance file
    async fn extract_dts_references_from_instance(
        &self,
        instance_file_path: &Path,
    ) -> Result<Vec<DtsReference>> {
        let content = fs::read_to_string(instance_file_path).await?;

        use quick_xml::events::Event;
        use quick_xml::Reader;
        use std::io::Cursor;

        let mut reader = Reader::from_reader(Cursor::new(content.as_bytes()));
        reader.config_mut().trim_text(true);

        let mut references = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    if e.name().as_ref() == b"schemaRef" || e.name().as_ref() == b"linkbaseRef" {
                        let mut href = None;
                        let mut role = None;
                        let mut arcrole = None;

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"href" => {
                                    if let Ok(value) = std::str::from_utf8(&attr.value) {
                                        href = Some(value.to_string());
                                    }
                                }
                                b"role" => {
                                    if let Ok(value) = std::str::from_utf8(&attr.value) {
                                        role = Some(value.to_string());
                                    }
                                }
                                b"arcrole" => {
                                    if let Ok(value) = std::str::from_utf8(&attr.value) {
                                        arcrole = Some(value.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }

                        if let Some(href) = href {
                            let reference_type = if e.name().as_ref() == b"schemaRef" {
                                "schemaRef"
                            } else {
                                "linkbaseRef"
                            };

                            references.push(DtsReference {
                                reference_type: reference_type.to_string(),
                                reference_role: role,
                                reference_href: href,
                                reference_arcrole: arcrole,
                            });
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to parse XBRL for DTS discovery: {}",
                        e
                    ));
                }
            }
            buf.clear();
        }

        Ok(references)
    }

    /// Resolve a single DTS reference
    async fn resolve_dts_reference(
        &self,
        reference: &DtsReference,
        statement_id: Uuid,
    ) -> Result<DtsResolution> {
        // Check if we already have this taxonomy file in the database
        let existing_schema = self
            .find_existing_taxonomy(&reference.reference_href)
            .await?;

        if let Some(schema) = existing_schema {
            // We already have this taxonomy file
            return Ok(DtsResolution {
                reference: reference.clone(),
                schema_id: Some(schema.id),
                linkbase_id: None,
                local_file_path: Some(self.get_local_file_path(&schema)),
                is_resolved: true,
                resolution_error: None,
            });
        }

        // Try to find a local file that matches this reference
        let local_file = self
            .find_local_taxonomy_file(&reference.reference_href)
            .await?;

        if let Some(file_path) = local_file {
            // We have a local file, store it in the database
            let schema_id = self
                .store_local_taxonomy_file(&file_path, reference, statement_id)
                .await?;

            return Ok(DtsResolution {
                reference: reference.clone(),
                schema_id: Some(schema_id),
                linkbase_id: None,
                local_file_path: Some(file_path),
                is_resolved: true,
                resolution_error: None,
            });
        }

        // No local file found, mark as unresolved
        Ok(DtsResolution {
            reference: reference.clone(),
            schema_id: None,
            linkbase_id: None,
            local_file_path: None,
            is_resolved: false,
            resolution_error: Some(format!(
                "No local taxonomy file found for: {}",
                reference.reference_href
            )),
        })
    }

    /// Find existing taxonomy in database
    async fn find_existing_taxonomy(&self, href: &str) -> Result<Option<XbrlTaxonomySchema>> {
        let mut conn = self.pool.get().await?;

        // Try to match by filename or namespace
        let filename = self.extract_filename_from_href(href);

        let schema = diesel_async::RunQueryDsl::first(
            xbrl_taxonomy_schemas::table
                .filter(xbrl_taxonomy_schemas::schema_filename.eq(&filename))
                .limit(1),
            &mut conn,
        )
        .await
        .optional()?;

        Ok(schema)
    }

    /// Find local taxonomy file
    async fn find_local_taxonomy_file(&self, href: &str) -> Result<Option<PathBuf>> {
        let filename = self.extract_filename_from_href(href);
        let local_path = self.cache_dir.join(&filename);

        if local_path.exists() {
            Ok(Some(local_path))
        } else {
            Ok(None)
        }
    }

    /// Extract filename from href
    fn extract_filename_from_href(&self, href: &str) -> String {
        if href.starts_with("http://") || href.starts_with("https://") {
            // Extract filename from URL
            href.split('/').next_back().unwrap_or(href).to_string()
        } else {
            // Already a filename or relative path
            href.split('/').next_back().unwrap_or(href).to_string()
        }
    }

    /// Store local taxonomy file in database
    async fn store_local_taxonomy_file(
        &self,
        file_path: &Path,
        reference: &DtsReference,
        statement_id: Uuid,
    ) -> Result<Uuid> {
        let content = fs::read(file_path).await?;
        let file_size = content.len() as i64;

        // Calculate file hash
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let file_hash = format!("sha256:{:x}", hasher.finalize());

        // Determine file type and source type
        let file_type = if reference.reference_type == "schemaRef" {
            TaxonomyFileType::Schema
        } else {
            // Determine linkbase type from role or filename
            if reference.reference_href.contains("_lab") {
                TaxonomyFileType::LabelLinkbase
            } else if reference.reference_href.contains("_pre") {
                TaxonomyFileType::PresentationLinkbase
            } else if reference.reference_href.contains("_cal") {
                TaxonomyFileType::CalculationLinkbase
            } else if reference.reference_href.contains("_def") {
                TaxonomyFileType::DefinitionLinkbase
            } else {
                TaxonomyFileType::LabelLinkbase // Default
            }
        };

        let source_type = self.determine_taxonomy_source_type(&reference.reference_href);

        // Extract namespace and filename
        let (schema_namespace, schema_filename) =
            self.extract_taxonomy_info(&reference.reference_href);

        let schema_id = Uuid::new_v4();

        // Create taxonomy schema record
        let taxonomy_schema = XbrlTaxonomySchema {
            id: schema_id,
            schema_namespace,
            schema_filename,
            schema_version: None,
            schema_date: None,
            file_type,
            source_type,
            file_content: Some(content),
            file_oid: None,
            file_size_bytes: file_size,
            file_hash,
            is_compressed: false,
            compression_type: econ_graph_core::enums::CompressionType::None,
            source_url: Some(reference.reference_href.clone()),
            download_url: Some(reference.reference_href.clone()),
            original_filename: Some(file_path.file_name().unwrap().to_string_lossy().to_string()),
            processing_status: econ_graph_core::enums::ProcessingStatus::Downloaded,
            processing_error: None,
            processing_started_at: None,
            processing_completed_at: Some(Utc::now()),
            concepts_extracted: 0,
            relationships_extracted: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Insert into database
        let mut conn = self.pool.get().await?;
        diesel_async::RunQueryDsl::execute(
            diesel::insert_into(xbrl_taxonomy_schemas::table).values(&taxonomy_schema),
            &mut conn,
        )
        .await
        .context("Failed to insert taxonomy schema")?;

        Ok(schema_id)
    }

    /// Determine taxonomy source type from href
    fn determine_taxonomy_source_type(&self, href: &str) -> TaxonomySourceType {
        if href.contains("apple.com") || href.contains("aapl") {
            TaxonomySourceType::CompanySpecific
        } else if href.contains("chevron.com") || href.contains("cvx") {
            TaxonomySourceType::CompanySpecific
        } else if href.contains("us-gaap") {
            TaxonomySourceType::UsGaap
        } else if href.contains("dei") {
            TaxonomySourceType::SecDei
        } else if href.contains("srt") {
            TaxonomySourceType::FasbSrt
        } else if href.contains("ifrs") {
            TaxonomySourceType::Ifrs
        } else {
            TaxonomySourceType::OtherStandard
        }
    }

    /// Extract taxonomy information from href
    fn extract_taxonomy_info(&self, href: &str) -> (String, String) {
        let filename = self.extract_filename_from_href(href);

        // Determine namespace based on filename
        let namespace = if filename.starts_with("aapl-") {
            format!(
                "http://www.apple.com/{}",
                filename.replace(".xsd", "").replace(".xml", "")
            )
        } else if filename.starts_with("cvx-") {
            format!(
                "http://www.chevron.com/{}",
                filename.replace(".xsd", "").replace(".xml", "")
            )
        } else if filename.contains("us-gaap") {
            "http://fasb.org/us-gaap/2024".to_string()
        } else if filename.contains("dei") {
            "http://xbrl.sec.gov/dei/2024".to_string()
        } else if filename.contains("srt") {
            "http://fasb.org/srt/2024".to_string()
        } else {
            format!(
                "http://xbrl.org/{}",
                filename.replace(".xsd", "").replace(".xml", "")
            )
        };

        (namespace, filename)
    }

    /// Get local file path for a taxonomy schema
    fn get_local_file_path(&self, schema: &XbrlTaxonomySchema) -> PathBuf {
        self.cache_dir.join(&schema.schema_filename)
    }

    /// Create a mapping of taxonomy files for Arelle
    pub async fn create_taxonomy_mapping(
        &self,
        statement_id: Uuid,
    ) -> Result<HashMap<String, String>> {
        let mut conn = self.pool.get().await?;
        let mut mapping = HashMap::new();

        // Get all taxonomy schemas for this statement
        let schemas = diesel_async::RunQueryDsl::load(
            xbrl_taxonomy_schemas::table
                .filter(xbrl_taxonomy_schemas::processing_status.eq("downloaded")),
            &mut conn,
        )
        .await?;

        for schema in schemas {
            let local_path = self.get_local_file_path(&schema);
            if local_path.exists() {
                // Map various URL patterns to local file
                mapping.insert(
                    schema.source_url.clone().unwrap_or_default(),
                    schema.schema_filename.clone(),
                );
                mapping.insert(
                    schema.schema_filename.clone(),
                    schema.schema_filename.clone(),
                );

                // Add namespace-based mapping
                mapping.insert(
                    schema.schema_namespace.clone(),
                    schema.schema_filename.clone(),
                );
            }
        }

        Ok(mapping)
    }
}

/// **DTS Reference**
///
/// Represents a DTS reference found in an XBRL instance file.
#[derive(Debug, Clone)]
pub struct DtsReference {
    pub reference_type: String,
    pub reference_role: Option<String>,
    pub reference_href: String,
    pub reference_arcrole: Option<String>,
}

/// **DTS Resolution**
///
/// Represents the resolution of a DTS reference.
#[derive(Debug, Clone)]
pub struct DtsResolution {
    pub reference: DtsReference,
    pub schema_id: Option<Uuid>,
    pub linkbase_id: Option<Uuid>,
    pub local_file_path: Option<PathBuf>,
    pub is_resolved: bool,
    pub resolution_error: Option<String>,
}
