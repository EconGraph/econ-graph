//! Storage trait stub for financial-core
//! 
//! TODO: Import from econ-graph-arrow-storage once dependencies are resolved

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::models::{DataPoint, EconomicSeries};

/// Financial data storage trait
#[async_trait]
pub trait FinancialDataStorage: Send + Sync {
    async fn store_series(&self, series: &EconomicSeries) -> Result<()>;
    async fn get_series(&self, id: &Uuid) -> Result<Option<EconomicSeries>>;
    async fn store_data_points(&self, points: &[DataPoint]) -> Result<()>;
    async fn get_data_points(&self, series_id: &Uuid) -> Result<Vec<DataPoint>>;
    
    // Additional methods expected by the database module
    async fn read_series(&self, id: &Uuid) -> Result<Option<EconomicSeries>> {
        self.get_series(id).await
    }
    
    async fn read_data_points(&self, series_id: &Uuid, _start_date: Option<chrono::NaiveDate>, _end_date: Option<chrono::NaiveDate>) -> Result<Vec<DataPoint>> {
        self.get_data_points(series_id).await
    }
    
    async fn write_series(&self, series: &EconomicSeries) -> Result<()> {
        self.store_series(series).await
    }
    
    async fn write_data_points(&self, _series_id: Uuid, points: &[DataPoint]) -> Result<()> {
        self.store_data_points(points).await
    }
    
    async fn list_series(&self) -> Result<Vec<EconomicSeries>> {
        // Default implementation returns empty list
        Ok(vec![])
    }
}

/// Parquet storage implementation (stub)
#[derive(Debug, Clone)]
pub struct ParquetStorage;

impl ParquetStorage {
    pub fn new(_data_dir: impl Into<String>) -> Self {
        Self {}
    }
}

/// Iceberg storage implementation (stub)
#[derive(Debug, Clone)]
pub struct IcebergStorage;

impl IcebergStorage {
    pub fn new(_data_dir: impl Into<String>) -> Result<Self> {
        Ok(Self {})
    }
}

/// In-memory storage implementation (stub)
#[derive(Debug)]
pub struct InMemoryStorage;

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl FinancialDataStorage for ParquetStorage {
    async fn store_series(&self, _series: &EconomicSeries) -> Result<()> {
        Ok(())
    }

    async fn get_series(&self, _id: &Uuid) -> Result<Option<EconomicSeries>> {
        Ok(None)
    }

    async fn store_data_points(&self, _points: &[DataPoint]) -> Result<()> {
        Ok(())
    }

    async fn get_data_points(&self, _series_id: &Uuid) -> Result<Vec<DataPoint>> {
        Ok(vec![])
    }
}

#[async_trait]
impl FinancialDataStorage for IcebergStorage {
    async fn store_series(&self, _series: &EconomicSeries) -> Result<()> {
        Ok(())
    }

    async fn get_series(&self, _id: &Uuid) -> Result<Option<EconomicSeries>> {
        Ok(None)
    }

    async fn store_data_points(&self, _points: &[DataPoint]) -> Result<()> {
        Ok(())
    }

    async fn get_data_points(&self, _series_id: &Uuid) -> Result<Vec<DataPoint>> {
        Ok(vec![])
    }
}

#[async_trait]
impl FinancialDataStorage for InMemoryStorage {
    async fn store_series(&self, _series: &EconomicSeries) -> Result<()> {
        Ok(())
    }

    async fn get_series(&self, _id: &Uuid) -> Result<Option<EconomicSeries>> {
        Ok(None)
    }

    async fn store_data_points(&self, _points: &[DataPoint]) -> Result<()> {
        Ok(())
    }

    async fn get_data_points(&self, _series_id: &Uuid) -> Result<Vec<DataPoint>> {
        Ok(vec![])
    }
}
