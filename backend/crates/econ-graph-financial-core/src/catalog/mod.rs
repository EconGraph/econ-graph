//! Data catalog module for tracking series metadata and data ranges
//!
//! This module provides a simple file-based catalog system that tracks:
//! - Available time series and their metadata
//! - Data ranges and coverage for each series
//! - File locations and organization
//! - Data freshness and update timestamps

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub mod file_catalog;
pub mod series_metadata;

pub use file_catalog::FileCatalog;
pub use series_metadata::SeriesMetadata;

/// Main catalog interface for managing series metadata and data discovery
#[derive(Debug, Clone)]
pub struct DataCatalog {
    catalog_dir: PathBuf,
    file_catalog: FileCatalog,
}

impl DataCatalog {
    /// Create a new data catalog in the specified directory
    pub fn new(catalog_dir: impl Into<PathBuf>) -> Result<Self> {
        let catalog_dir = catalog_dir.into();
        std::fs::create_dir_all(&catalog_dir)?;

        let file_catalog = FileCatalog::new(&catalog_dir)?;

        Ok(Self {
            catalog_dir,
            file_catalog,
        })
    }

    /// Add or update a series in the catalog
    pub async fn add_series(&self, metadata: SeriesMetadata) -> Result<()> {
        self.file_catalog.add_series(metadata).await
    }

    /// Get series metadata by ID
    pub async fn get_series(&self, series_id: Uuid) -> Result<Option<SeriesMetadata>> {
        self.file_catalog.get_series(series_id).await
    }

    /// List all series in the catalog
    pub async fn list_series(&self) -> Result<Vec<SeriesMetadata>> {
        self.file_catalog.list_series().await
    }

    /// Update data range for a series
    pub async fn update_data_range(
        &self,
        series_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<()> {
        self.file_catalog
            .update_data_range(series_id, start_date, end_date)
            .await
    }

    /// Find series with data in a specific date range
    pub async fn find_series_by_date_range(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<SeriesMetadata>> {
        self.file_catalog
            .find_series_by_date_range(start_date, end_date)
            .await
    }

    /// Find series by external ID
    pub async fn find_series_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<Option<SeriesMetadata>> {
        self.file_catalog
            .find_series_by_external_id(external_id)
            .await
    }

    /// Get catalog statistics
    pub async fn get_stats(&self) -> Result<CatalogStats> {
        self.file_catalog.get_stats().await
    }

    /// Discover and scan existing data files
    pub async fn scan_data_directory(&self, data_dir: &Path) -> Result<ScanResult> {
        self.file_catalog.scan_data_directory(data_dir).await
    }

    /// Validate catalog consistency
    pub async fn validate_catalog(&self, data_dir: &Path) -> Result<ValidationResult> {
        self.file_catalog.validate_catalog(data_dir).await
    }
}

/// Statistics about the catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogStats {
    pub total_series: usize,
    pub total_data_points: u64,
    pub earliest_date: Option<NaiveDate>,
    pub latest_date: Option<NaiveDate>,
    pub last_updated: DateTime<Utc>,
}

/// Result of scanning a data directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub files_discovered: usize,
    pub series_discovered: usize,
    pub partitions_discovered: usize,
    pub errors: Vec<String>,
}

/// Result of catalog validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub missing_files: Vec<PathBuf>,
    pub orphaned_files: Vec<PathBuf>,
    pub inconsistent_metadata: Vec<Uuid>,
    pub errors: Vec<String>,
}

impl Default for DataCatalog {
    fn default() -> Self {
        Self::new("./catalog").expect("Failed to create default catalog")
    }
}
