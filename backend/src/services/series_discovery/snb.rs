//! Swiss National Bank (SNB) data source integration
//!
//! This module provides integration with the Swiss National Bank's statistical database,
//! which provides comprehensive monetary policy, financial stability, and economic data for Switzerland.

use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::models::{DataSource, NewSeriesMetadata, SeriesMetadata};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Swiss National Bank Series Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnbSeriesInfo {
    pub external_id: String,
    pub title: String,
    pub description: String,
    pub units: String,
    pub frequency: String,
    pub geographic_level: String,
    pub data_url: String,
}

/// Swiss National Bank Data Source client
pub struct SnbDataSource {
    client: Client,
    base_url: String,
}

impl Default for SnbDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SnbDataSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://data.snb.ch".to_string(),
        }
    }

    pub async fn discover_series(&self) -> AppResult<Vec<SnbSeriesInfo>> {
        Ok(vec![
            SnbSeriesInfo {
                external_id: "ir".to_string(),
                title: "Switzerland - SNB Policy Rate".to_string(),
                description: "Swiss National Bank policy rate".to_string(),
                units: "Percent per annum".to_string(),
                frequency: "Daily".to_string(),
                geographic_level: "Switzerland".to_string(),
                data_url: format!("{}/en/ir", self.base_url),
            },
            SnbSeriesInfo {
                external_id: "gdp".to_string(),
                title: "Switzerland - GDP".to_string(),
                description: "Gross Domestic Product for Switzerland".to_string(),
                units: "Billions of Swiss francs".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "Switzerland".to_string(),
                data_url: format!("{}/en/gdp", self.base_url),
            },
            SnbSeriesInfo {
                external_id: "cpi".to_string(),
                title: "Switzerland - Consumer Price Index".to_string(),
                description: "Consumer Price Index for Switzerland".to_string(),
                units: "Index (December 2020=100)".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Switzerland".to_string(),
                data_url: format!("{}/en/cpi", self.base_url),
            },
        ])
    }
}

pub async fn discover_snb_series(_client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let snb_source = SnbDataSource::new();
    let series_list = snb_source.discover_series().await?;
    let snb_data_source = DataSource::get_or_create(pool, DataSource::snb()).await?;
    let mut discovered_series = Vec::new();

    for series_info in series_list {
        let new_metadata = NewSeriesMetadata {
            source_id: snb_data_source.id,
            external_id: series_info.external_id.clone(),
            title: series_info.title,
            description: Some(series_info.description),
            units: Some(series_info.units),
            frequency: Some(series_info.frequency),
            geographic_level: Some(series_info.geographic_level),
            data_url: Some(series_info.data_url),
            api_endpoint: None,
            is_active: true,
        };

        match SeriesMetadata::get_or_create(
            pool,
            snb_data_source.id,
            &series_info.external_id,
            &new_metadata,
        )
        .await
        {
            Ok(_) => discovered_series.push(series_info.external_id),
            Err(e) => eprintln!(
                "Failed to create Swiss National Bank series {}: {}",
                series_info.external_id, e
            ),
        }
    }

    Ok(discovered_series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestContainer;
    use tokio;

    #[tokio::test]
    async fn test_snb_data_source_creation() {
        let snb = SnbDataSource::new();
        assert_eq!(snb.base_url, "https://data.snb.ch");
    }

    #[tokio::test]
    async fn test_discover_series() {
        let snb = SnbDataSource::new();
        let series = snb.discover_series().await;
        assert!(series.is_ok());
        let series_list = series.unwrap();
        assert!(!series_list.is_empty());
        assert!(series_list.iter().any(|s| s.title.contains("Policy Rate")));
    }

    #[tokio::test]
    async fn test_discover_snb_series_integration() {
        let container = TestContainer::new().await;
        let pool = &container.pool;
        let discovered_series = discover_snb_series(&Client::new(), pool).await;
        assert!(discovered_series.is_ok());
        let series_ids = discovered_series.unwrap();
        assert!(!series_ids.is_empty());
    }
}
