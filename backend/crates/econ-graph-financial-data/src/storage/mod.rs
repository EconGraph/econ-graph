use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use uuid::Uuid;

use crate::models::{DataPoint, EconomicSeries};

/// Storage abstraction for financial data using Arrow Flight
/// This allows us to switch between Parquet files (V1) and Iceberg (V2)
/// without changing the GraphQL API or business logic
///
/// All implementations use Arrow Flight for zero-copy, low-latency data transfer
#[async_trait]
pub trait FinancialDataStorage: Send + Sync {
    /// Write a series to storage via Arrow Flight
    async fn write_series(&self, series: &EconomicSeries) -> Result<()>;

    /// Read a series from storage via Arrow Flight
    async fn read_series(&self, series_id: Uuid) -> Result<Option<EconomicSeries>>;

    /// Write data points for a series via Arrow Flight
    async fn write_data_points(&self, series_id: Uuid, points: &[DataPoint]) -> Result<()>;

    /// Read data points for a series via Arrow Flight
    async fn read_data_points(
        &self,
        series_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<DataPoint>>;

    /// List all series via Arrow Flight
    async fn list_series(&self) -> Result<Vec<EconomicSeries>>;
}

pub mod iceberg_storage;
pub mod in_memory_storage;
pub mod parquet_storage;

// Re-export the implementations
// pub use iceberg_storage::IcebergStorage; // TODO: Enable for V2
pub use in_memory_storage::InMemoryStorage;
pub use parquet_storage::ParquetStorage;
