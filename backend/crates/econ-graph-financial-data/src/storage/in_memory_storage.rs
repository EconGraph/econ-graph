use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::FinancialDataStorage;
use crate::models::{DataPoint, EconomicSeries};

/// In-memory storage implementation for testing
///
/// This provides a simple HashMap-based storage that implements
/// the same FinancialDataStorage trait as ParquetStorage and IcebergStorage
///
/// Used for testing and development when you don't need persistent storage
pub struct InMemoryStorage {
    series_cache: Arc<RwLock<HashMap<Uuid, EconomicSeries>>>,
    data_points_cache: Arc<RwLock<HashMap<Uuid, Vec<DataPoint>>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            series_cache: Arc::new(RwLock::new(HashMap::new())),
            data_points_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FinancialDataStorage for InMemoryStorage {
    async fn write_series(&self, series: &EconomicSeries) -> Result<()> {
        let mut cache = self.series_cache.write().await;
        cache.insert(series.id, series.clone());
        Ok(())
    }

    async fn read_series(&self, series_id: Uuid) -> Result<Option<EconomicSeries>> {
        let cache = self.series_cache.read().await;
        Ok(cache.get(&series_id).cloned())
    }

    async fn write_data_points(&self, series_id: Uuid, points: &[DataPoint]) -> Result<()> {
        let mut cache = self.data_points_cache.write().await;
        let existing_points = cache.entry(series_id).or_insert_with(Vec::new);

        for point in points {
            existing_points.push(point.clone());
        }
        Ok(())
    }

    async fn read_data_points(
        &self,
        series_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<DataPoint>> {
        let cache = self.data_points_cache.read().await;
        let points = cache.get(&series_id).cloned().unwrap_or_default();

        // Filter by date range if provided
        let filtered_points = if let (Some(start), Some(end)) = (start_date, end_date) {
            points
                .into_iter()
                .filter(|point| point.date >= start && point.date <= end)
                .collect()
        } else if let Some(start) = start_date {
            points
                .into_iter()
                .filter(|point| point.date >= start)
                .collect()
        } else if let Some(end) = end_date {
            points
                .into_iter()
                .filter(|point| point.date <= end)
                .collect()
        } else {
            points
        };

        Ok(filtered_points)
    }

    async fn list_series(&self) -> Result<Vec<EconomicSeries>> {
        let cache = self.series_cache.read().await;
        Ok(cache.values().cloned().collect())
    }
}
