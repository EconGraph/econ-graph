//! File-based catalog implementation
//!
//! This module implements a simple file-based catalog system that stores
//! series metadata in JSON files for easy discovery and management.

use anyhow::Result;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::fs as async_fs;
use uuid::Uuid;

use super::{CatalogStats, ScanResult, ValidationResult};
use crate::catalog::SeriesMetadata;

/// File-based catalog implementation
#[derive(Debug, Clone)]
pub struct FileCatalog {
    catalog_dir: PathBuf,
    catalog_file: PathBuf,
    index_file: PathBuf,
}

/// Catalog index for fast lookups
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogIndex {
    series_by_id: HashMap<Uuid, String>, // series_id -> external_id
    series_by_external_id: HashMap<String, Uuid>, // external_id -> series_id
    series_by_date_range: Vec<(Uuid, NaiveDate, NaiveDate)>, // series_id, start_date, end_date
    last_updated: chrono::DateTime<Utc>,
}

impl FileCatalog {
    /// Create a new file catalog
    pub fn new(catalog_dir: &Path) -> Result<Self> {
        let catalog_dir = catalog_dir.to_path_buf();
        std::fs::create_dir_all(&catalog_dir)?;

        let catalog_file = catalog_dir.join("catalog.json");
        let index_file = catalog_dir.join("index.json");

        Ok(Self {
            catalog_dir,
            catalog_file,
            index_file,
        })
    }

    /// Add or update a series in the catalog
    pub async fn add_series(&self, metadata: SeriesMetadata) -> Result<()> {
        // Load existing catalog
        let mut catalog = self.load_catalog().await?;

        // Add or update series
        catalog.insert(metadata.series_id, metadata.clone());

        // Save updated catalog
        self.save_catalog(&catalog).await?;

        // Update index
        self.update_index(&metadata).await?;

        Ok(())
    }

    /// Get series metadata by ID
    pub async fn get_series(&self, series_id: Uuid) -> Result<Option<SeriesMetadata>> {
        let catalog = self.load_catalog().await?;
        Ok(catalog.get(&series_id).cloned())
    }

    /// List all series in the catalog
    pub async fn list_series(&self) -> Result<Vec<SeriesMetadata>> {
        let catalog = self.load_catalog().await?;
        Ok(catalog.values().cloned().collect())
    }

    /// Update data range for a series
    pub async fn update_data_range(
        &self,
        series_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<()> {
        let mut catalog = self.load_catalog().await?;

        if let Some(metadata) = catalog.get_mut(&series_id) {
            metadata.update_data_range(start_date, end_date, 0, 0); // TODO: Get actual counts

            // Clone metadata before saving catalog to avoid borrow checker issues
            let metadata_clone = metadata.clone();

            self.save_catalog(&catalog).await?;
            self.update_index(&metadata_clone).await?;
        }

        Ok(())
    }

    /// Find series with data in a specific date range
    pub async fn find_series_by_date_range(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<SeriesMetadata>> {
        let index = self.load_index().await?;
        let catalog = self.load_catalog().await?;

        let mut matching_series = Vec::new();

        for (series_id, series_start, series_end) in index.series_by_date_range {
            if series_start <= end_date && series_end >= start_date {
                if let Some(metadata) = catalog.get(&series_id) {
                    matching_series.push(metadata.clone());
                }
            }
        }

        Ok(matching_series)
    }

    /// Find series by external ID
    pub async fn find_series_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<Option<SeriesMetadata>> {
        let index = self.load_index().await?;
        let catalog = self.load_catalog().await?;

        if let Some(series_id) = index.series_by_external_id.get(external_id) {
            Ok(catalog.get(series_id).cloned())
        } else {
            Ok(None)
        }
    }

    /// Get catalog statistics
    pub async fn get_stats(&self) -> Result<CatalogStats> {
        let catalog = self.load_catalog().await?;

        let total_series = catalog.len();
        let mut total_data_points = 0u64;
        let mut earliest_date = None;
        let mut latest_date = None;

        for metadata in catalog.values() {
            total_data_points += metadata.data_range.total_points;

            if let Some(start) = metadata.data_range.start_date {
                earliest_date = Some(earliest_date.map_or(start, |e: NaiveDate| e.min(start)));
            }

            if let Some(end) = metadata.data_range.end_date {
                latest_date = Some(latest_date.map_or(end, |l: NaiveDate| l.max(end)));
            }
        }

        Ok(CatalogStats {
            total_series,
            total_data_points,
            earliest_date,
            latest_date,
            last_updated: Utc::now(),
        })
    }

    /// Discover and scan existing data files
    pub async fn scan_data_directory(&self, data_dir: &Path) -> Result<ScanResult> {
        let mut files_discovered = 0;
        let mut series_discovered = 0;
        let mut partitions_discovered = 0;
        let mut errors = Vec::new();

        if !data_dir.exists() {
            return Ok(ScanResult {
                files_discovered: 0,
                series_discovered: 0,
                partitions_discovered: 0,
                errors: vec!["Data directory does not exist".to_string()],
            });
        }

        // Scan for partition directories
        if let Ok(entries) = fs::read_dir(data_dir) {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_dir()
                            && path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.starts_with("year="))
                                .unwrap_or(false)
                        {
                            partitions_discovered += 1;

                            // Scan partition for series files
                            if let Ok(partition_entries) = fs::read_dir(&path) {
                                for partition_entry in partition_entries {
                                    match partition_entry {
                                        Ok(partition_entry) => {
                                            let file_path = partition_entry.path();
                                            if file_path.is_file()
                                                && file_path
                                                    .extension()
                                                    .and_then(|ext| ext.to_str())
                                                    .map(|ext| ext == "parquet")
                                                    .unwrap_or(false)
                                            {
                                                files_discovered += 1;

                                                // Extract series ID from filename
                                                if let Some(file_name) =
                                                    file_path.file_name().and_then(|n| n.to_str())
                                                {
                                                    if file_name.starts_with("series_")
                                                        && file_name.ends_with(".parquet")
                                                    {
                                                        if let Some(id_str) = file_name
                                                            .strip_prefix("series_")
                                                            .and_then(|s| {
                                                                s.strip_suffix(".parquet")
                                                            })
                                                        {
                                                            if Uuid::parse_str(id_str).is_ok() {
                                                                series_discovered += 1;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => errors
                                            .push(format!("Error reading partition entry: {}", e)),
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => errors.push(format!("Error reading data directory entry: {}", e)),
                }
            }
        }

        Ok(ScanResult {
            files_discovered,
            series_discovered,
            partitions_discovered,
            errors,
        })
    }

    /// Validate catalog consistency
    pub async fn validate_catalog(&self, data_dir: &Path) -> Result<ValidationResult> {
        let catalog = self.load_catalog().await?;
        let mut missing_files = Vec::new();
        let orphaned_files = Vec::new();
        let inconsistent_metadata = Vec::new();
        let mut errors = Vec::new();

        // Check that all cataloged files exist
        for metadata in catalog.values() {
            for file_path in &metadata.coverage.file_paths {
                let full_path = data_dir.join(file_path);
                if !full_path.exists() {
                    missing_files.push(full_path);
                }
            }
        }

        // Check for orphaned files (files that exist but aren't in catalog)
        if let Ok(scan_result) = self.scan_data_directory(data_dir).await {
            // This is a simplified check - in a full implementation,
            // we'd cross-reference with the catalog
            if !scan_result.errors.is_empty() {
                errors.extend(scan_result.errors);
            }
        }

        let is_valid = missing_files.is_empty() && orphaned_files.is_empty() && errors.is_empty();

        Ok(ValidationResult {
            is_valid,
            missing_files,
            orphaned_files,
            inconsistent_metadata,
            errors,
        })
    }

    /// Load catalog from disk
    async fn load_catalog(&self) -> Result<HashMap<Uuid, SeriesMetadata>> {
        if !self.catalog_file.exists() {
            return Ok(HashMap::new());
        }

        let content = async_fs::read_to_string(&self.catalog_file).await?;
        let catalog: HashMap<Uuid, SeriesMetadata> = serde_json::from_str(&content)?;
        Ok(catalog)
    }

    /// Save catalog to disk
    async fn save_catalog(&self, catalog: &HashMap<Uuid, SeriesMetadata>) -> Result<()> {
        let content = serde_json::to_string_pretty(catalog)?;
        async_fs::write(&self.catalog_file, content).await?;
        Ok(())
    }

    /// Load index from disk
    async fn load_index(&self) -> Result<CatalogIndex> {
        if !self.index_file.exists() {
            return Ok(CatalogIndex::default());
        }

        let content = async_fs::read_to_string(&self.index_file).await?;
        let index: CatalogIndex = serde_json::from_str(&content)?;
        Ok(index)
    }

    /// Save index to disk
    async fn save_index(&self, index: &CatalogIndex) -> Result<()> {
        let content = serde_json::to_string_pretty(index)?;
        async_fs::write(&self.index_file, content).await?;
        Ok(())
    }

    /// Update index with new series metadata
    async fn update_index(&self, metadata: &SeriesMetadata) -> Result<()> {
        let mut index = self.load_index().await?;

        // Update series ID mapping
        index
            .series_by_id
            .insert(metadata.series_id, metadata.external_id.clone());
        index
            .series_by_external_id
            .insert(metadata.external_id.clone(), metadata.series_id);

        // Update date range index
        index
            .series_by_date_range
            .retain(|(id, _, _)| *id != metadata.series_id);

        if let (Some(start_date), Some(end_date)) =
            (metadata.data_range.start_date, metadata.data_range.end_date)
        {
            index
                .series_by_date_range
                .push((metadata.series_id, start_date, end_date));
        }

        // Sort by start date for efficient range queries
        index
            .series_by_date_range
            .sort_by_key(|(_, start, _)| *start);

        index.last_updated = Utc::now();

        self.save_index(&index).await?;
        Ok(())
    }
}

impl Default for CatalogIndex {
    fn default() -> Self {
        Self {
            series_by_id: HashMap::new(),
            series_by_external_id: HashMap::new(),
            series_by_date_range: Vec::new(),
            last_updated: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_catalog_basic_operations() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let catalog_dir = temp_dir.path();
        let catalog = FileCatalog::new(catalog_dir)?;

        // Test adding a series
        let series_id = Uuid::new_v4();
        let metadata = SeriesMetadata::new(
            series_id,
            "TEST_SERIES".to_string(),
            "Test Series".to_string(),
            "Daily".to_string(),
            "Test".to_string(),
        );

        catalog.add_series(metadata.clone()).await?;

        // Test retrieving the series
        let retrieved = catalog.get_series(series_id).await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().external_id, "TEST_SERIES");

        // Test listing all series
        let all_series = catalog.list_series().await?;
        assert_eq!(all_series.len(), 1);
        assert_eq!(all_series[0].series_id, series_id);

        // Test finding by external ID
        let found = catalog.find_series_by_external_id("TEST_SERIES").await?;
        assert!(found.is_some());
        assert_eq!(found.unwrap().series_id, series_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_catalog_stats() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let catalog_dir = temp_dir.path();
        let catalog = FileCatalog::new(catalog_dir)?;

        // Add a series with data
        let series_id = Uuid::new_v4();
        let mut metadata = SeriesMetadata::new(
            series_id,
            "TEST_SERIES".to_string(),
            "Test Series".to_string(),
            "Daily".to_string(),
            "Test".to_string(),
        );

        let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end_date = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        metadata.update_data_range(Some(start_date), Some(end_date), 365, 5);

        catalog.add_series(metadata).await?;

        let stats = catalog.get_stats().await?;
        assert_eq!(stats.total_series, 1);
        assert_eq!(stats.total_data_points, 365);
        assert_eq!(stats.earliest_date, Some(start_date));
        assert_eq!(stats.latest_date, Some(end_date));

        Ok(())
    }

    #[tokio::test]
    async fn test_date_range_query() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let catalog_dir = temp_dir.path();
        let catalog = FileCatalog::new(catalog_dir)?;

        // Add series with different date ranges
        let series1_id = Uuid::new_v4();
        let mut metadata1 = SeriesMetadata::new(
            series1_id,
            "SERIES_1".to_string(),
            "Series 1".to_string(),
            "Daily".to_string(),
            "Test".to_string(),
        );
        metadata1.update_data_range(
            Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2024, 6, 30).unwrap()),
            180,
            0,
        );
        catalog.add_series(metadata1).await?;

        let series2_id = Uuid::new_v4();
        let mut metadata2 = SeriesMetadata::new(
            series2_id,
            "SERIES_2".to_string(),
            "Series 2".to_string(),
            "Daily".to_string(),
            "Test".to_string(),
        );
        metadata2.update_data_range(
            Some(NaiveDate::from_ymd_opt(2024, 7, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
            180,
            0,
        );
        catalog.add_series(metadata2).await?;

        // Query for overlapping date range
        let results = catalog
            .find_series_by_date_range(
                NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                NaiveDate::from_ymd_opt(2024, 8, 31).unwrap(),
            )
            .await?;

        assert_eq!(results.len(), 2); // Both series should match

        // Query for non-overlapping date range
        let results = catalog
            .find_series_by_date_range(
                NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            )
            .await?;

        assert_eq!(results.len(), 0); // No series should match

        Ok(())
    }
}
