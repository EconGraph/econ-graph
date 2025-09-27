use anyhow::Result;
use chrono::NaiveDate;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::models::{DataPoint, EconomicSeries};
use crate::storage::{FinancialDataStorage, InMemoryStorage, ParquetStorage};

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
        let result = self.storage.read_series(id).await;
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
            .read_data_points(series_id, start_date, end_date)
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
}
