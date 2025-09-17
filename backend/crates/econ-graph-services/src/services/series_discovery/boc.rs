//! Bank of Canada (BoC) data source integration
//!
//! This module provides integration with the Bank of Canada's statistical database,
//! which provides comprehensive monetary policy, financial stability, and economic data for Canada.

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::AppResult;
use econ_graph_core::models::{DataSource, NewSeriesMetadata, SeriesMetadata};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Bank of Canada Series Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BocSeriesInfo {
    pub external_id: String,
    pub title: String,
    pub description: String,
    pub units: String,
    pub frequency: String,
    pub geographic_level: String,
    pub data_url: String,
}

/// Bank of Canada Data Source client
pub struct BocDataSource {
    client: Client,
    base_url: String,
}

impl Default for BocDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl BocDataSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://www.bankofcanada.ca/valet".to_string(),
        }
    }

    pub async fn discover_series(&self) -> AppResult<Vec<BocSeriesInfo>> {
        Ok(vec![
            BocSeriesInfo {
                external_id: "V39079".to_string(),
                title: "Canada - Overnight Rate Target".to_string(),
                description: "The target for the overnight rate set by the Bank of Canada"
                    .to_string(),
                units: "Percent per annum".to_string(),
                frequency: "Daily".to_string(),
                geographic_level: "Canada".to_string(),
                data_url: format!("{}/observations/V39079", self.base_url),
            },
            BocSeriesInfo {
                external_id: "V41690914".to_string(),
                title: "Canada - Consumer Price Index".to_string(),
                description: "Consumer Price Index for Canada".to_string(),
                units: "Index (2002=100)".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Canada".to_string(),
                data_url: format!("{}/observations/V41690914", self.base_url),
            },
            BocSeriesInfo {
                external_id: "V62700578".to_string(),
                title: "Canada - GDP".to_string(),
                description: "Gross Domestic Product for Canada".to_string(),
                units: "Millions of Canadian dollars".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "Canada".to_string(),
                data_url: format!("{}/observations/V62700578", self.base_url),
            },
        ])
    }
}

pub async fn discover_boc_series(_client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let boc_source = BocDataSource::new();
    let series_list = boc_source.discover_series().await?;
    let boc_data_source = DataSource::get_or_create(pool, DataSource::boc()).await?;
    let mut discovered_series = Vec::new();

    for series_info in series_list {
        let new_metadata = NewSeriesMetadata {
            source_id: boc_data_source.id,
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
            boc_data_source.id,
            &series_info.external_id,
            &new_metadata,
        )
        .await
        {
            Ok(_) => discovered_series.push(series_info.external_id),
            Err(e) => eprintln!(
                "Failed to create Bank of Canada series {}: {}",
                series_info.external_id, e
            ),
        }
    }

    Ok(discovered_series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::test_utils::TestContainer;
    use tokio;

    #[tokio::test]
    async fn test_boc_data_source_creation() {
        let boc = BocDataSource::new();
        assert_eq!(boc.base_url, "https://www.bankofcanada.ca/valet");
    }

    #[tokio::test]
    async fn test_discover_series() {
        let boc = BocDataSource::new();
        let series = boc.discover_series().await;
        assert!(series.is_ok());
        let series_list = series.unwrap();
        assert!(!series_list.is_empty());
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("Overnight Rate")));
    }

    #[tokio::test]
    async fn test_discover_boc_series_integration() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let discovered_series = discover_boc_series(&Client::new(), pool).await;
        assert!(discovered_series.is_ok());
        let series_ids = discovered_series.unwrap();
        assert!(!series_ids.is_empty());
    }
}
