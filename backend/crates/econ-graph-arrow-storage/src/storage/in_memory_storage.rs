//! In-memory storage implementation for financial data
//!
//! Simple in-memory storage for testing and development

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::types::{DataPoint, EconomicSeries};

/// In-memory storage implementation
#[derive(Debug)]
pub struct InMemoryStorage {
    series: Arc<RwLock<HashMap<Uuid, EconomicSeries>>>,
    data_points: Arc<RwLock<HashMap<Uuid, Vec<DataPoint>>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            series: Arc::new(RwLock::new(HashMap::new())),
            data_points: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl super::FinancialDataStorage for InMemoryStorage {
    async fn store_series(&self, series: &EconomicSeries) -> Result<()> {
        let mut series_map = self.series.write().await;
        series_map.insert(series.id, series.clone());
        Ok(())
    }

    async fn get_series(&self, id: &Uuid) -> Result<Option<EconomicSeries>> {
        let series_map = self.series.read().await;
        Ok(series_map.get(id).cloned())
    }

    async fn store_data_points(&self, points: &[DataPoint]) -> Result<()> {
        let mut data_points_map = self.data_points.write().await;
        for point in points {
            data_points_map
                .entry(point.series_id)
                .or_insert_with(Vec::new)
                .push(point.clone());
        }
        Ok(())
    }

    async fn get_data_points(&self, series_id: &Uuid) -> Result<Vec<DataPoint>> {
        let data_points_map = self.data_points.read().await;
        Ok(data_points_map.get(series_id).cloned().unwrap_or_default())
    }
}
