use anyhow::Result;
use chrono::NaiveDate;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::catalog::{CatalogStats, ScanResult, ValidationResult};
use crate::models::{DataPoint, EconomicSeries};
use crate::storage::{FinancialDataStorage, IcebergStorage, InMemoryStorage, ParquetStorage};

/// Database abstraction for financial data service
///
/// This uses the storage abstraction to provide a clean interface
/// that can work with both Parquet (V1) and Iceberg (V2) backends
#[derive(Clone)]
pub struct Database {
    storage: Arc<dyn FinancialDataStorage + Send + Sync>,
}

impl Database {
    /// Create a new database with Parquet storage (V1)
    pub async fn new_with_parquet(data_dir: impl Into<String>) -> Result<Self> {
        tracing::info!("Initializing database with Parquet storage");

        let storage = ParquetStorage::new(data_dir.into());
        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    /// Create a new database with custom partitioning storage (V2)
    pub async fn new_with_custom_partitioning(data_dir: impl Into<String>) -> Result<Self> {
        tracing::info!("Initializing database with custom partitioning storage");

        let storage = IcebergStorage::new(data_dir.into())?;
        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    /// Create a new database with in-memory storage (for testing)
    pub async fn new_in_memory() -> Result<Self> {
        tracing::info!("Initializing database with in-memory storage for testing");

        // For testing, we'll use a simple in-memory implementation
        // This will be replaced with the storage abstraction

        let storage = InMemoryStorage::new();
        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    pub async fn get_series(&self, id: Uuid) -> Result<Option<EconomicSeries>> {
        let start_time = Instant::now();
        let result = self.storage.read_series(&id).await;
        let duration = start_time.elapsed();
        let success = result.is_ok();

        tracing::debug!(
            "Database operation: get_series, duration: {:?}, success: {}",
            duration,
            success
        );

        result
    }

    pub async fn get_data_points(
        &self,
        series_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<DataPoint>> {
        let start_time = Instant::now();
        let result = self
            .storage
            .read_data_points(&series_id, start_date, end_date)
            .await;
        let duration = start_time.elapsed();
        let success = result.is_ok();

        tracing::debug!(
            "Database operation: get_data_points, duration: {:?}, success: {}",
            duration,
            success
        );

        result
    }

    pub async fn create_series(&self, series: EconomicSeries) -> Result<()> {
        let start_time = Instant::now();
        let result = self.storage.write_series(&series).await;
        let duration = start_time.elapsed();
        let success = result.is_ok();

        tracing::debug!(
            "Database operation: create_series, duration: {:?}, success: {}",
            duration,
            success
        );

        result
    }

    pub async fn create_data_points(&self, points: Vec<DataPoint>) -> Result<()> {
        let start_time = Instant::now();
        let result = if let Some(first_point) = points.first() {
            self.storage
                .write_data_points(first_point.series_id, &points)
                .await
        } else {
            Ok(())
        };
        let duration = start_time.elapsed();
        let success = result.is_ok();

        tracing::debug!(
            "Database operation: create_data_points, duration: {:?}, success: {}",
            duration,
            success
        );

        result
    }

    pub async fn list_series(&self) -> Result<Vec<EconomicSeries>> {
        let start_time = Instant::now();
        let result = self.storage.list_series().await;
        let duration = start_time.elapsed();
        let success = result.is_ok();

        tracing::debug!(
            "Database operation: list_series, duration: {:?}, success: {}",
            duration,
            success
        );

        result
    }

    /// Get catalog statistics
    pub async fn get_catalog_stats(&self) -> Result<CatalogStats> {
        // For now, we'll return basic stats from the storage
        // In the future, this could be enhanced with more detailed statistics
        let series = self.storage.list_series().await?;

        let total_series = series.len();
        let total_data_points = 0u64;
        let mut earliest_date = None;
        let mut latest_date = None;

        for series in &series {
            // TODO: Get actual data point counts from storage
            // For now, we'll use placeholder values

            if let Some(start) = series.start_date {
                earliest_date = Some(earliest_date.map_or(start, |e: NaiveDate| e.min(start)));
            }

            if let Some(end) = series.end_date {
                latest_date = Some(latest_date.map_or(end, |l: NaiveDate| l.max(end)));
            }
        }

        Ok(CatalogStats {
            total_series,
            total_data_points,
            earliest_date,
            latest_date,
            last_updated: chrono::Utc::now(),
        })
    }

    /// Find series by date range
    pub async fn find_series_by_date_range(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<EconomicSeries>> {
        let all_series = self.storage.list_series().await?;

        let matching_series = all_series
            .into_iter()
            .filter(|series| match (series.start_date, series.end_date) {
                (Some(series_start), Some(series_end)) => {
                    series_start <= end_date && series_end >= start_date
                }
                (Some(series_start), None) => series_start <= end_date,
                (None, Some(series_end)) => series_end >= start_date,
                (None, None) => false,
            })
            .collect();

        Ok(matching_series)
    }

    /// Find series by external ID
    pub async fn find_series_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<Option<EconomicSeries>> {
        let all_series = self.storage.list_series().await?;

        Ok(all_series
            .into_iter()
            .find(|series| series.external_id == external_id))
    }

    /// Scan data directory for new files
    pub async fn scan_data_directory(&self) -> Result<ScanResult> {
        // For now, return a placeholder scan result
        // In a full implementation, this would scan the actual data directory
        Ok(ScanResult {
            files_discovered: 0,
            series_discovered: 0,
            partitions_discovered: 0,
            errors: vec!["Scan functionality not yet implemented".to_string()],
        })
    }

    /// Validate catalog consistency
    pub async fn validate_catalog(&self) -> Result<ValidationResult> {
        // For now, return a placeholder validation result
        // In a full implementation, this would validate the actual catalog
        Ok(ValidationResult {
            is_valid: true,
            missing_files: Vec::new(),
            orphaned_files: Vec::new(),
            inconsistent_metadata: Vec::new(),
            errors: vec!["Validation functionality not yet implemented".to_string()],
        })
    }
}
