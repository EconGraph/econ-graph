//! Reserve Bank of Australia (RBA) data source integration
//!
//! This module provides integration with the Reserve Bank of Australia's statistical database,
//! which provides comprehensive monetary policy, financial stability, and economic data for Australia.

use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::models::{DataSource, NewSeriesMetadata, SeriesMetadata};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// RBA Series Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbaSeriesInfo {
    pub external_id: String,
    pub title: String,
    pub description: String,
    pub units: String,
    pub frequency: String,
    pub geographic_level: String,
    pub data_url: String,
}

/// RBA Data Source client
pub struct RbaDataSource {
    client: Client,
    base_url: String,
}

impl Default for RbaDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl RbaDataSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://www.rba.gov.au/statistics".to_string(),
        }
    }

    pub async fn discover_series(&self) -> AppResult<Vec<RbaSeriesInfo>> {
        Ok(vec![
            RbaSeriesInfo {
                external_id: "F1.1".to_string(),
                title: "Australia - Cash Rate Target".to_string(),
                description: "The cash rate target set by the Reserve Bank Board".to_string(),
                units: "Percent per annum".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Australia".to_string(),
                data_url: format!("{}/f01-hist.html", self.base_url),
            },
            RbaSeriesInfo {
                external_id: "G1".to_string(),
                title: "Australia - Consumer Price Index".to_string(),
                description: "Consumer Price Index for Australia".to_string(),
                units: "Index (2011-12=100)".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "Australia".to_string(),
                data_url: format!("{}/g01-hist.html", self.base_url),
            },
            RbaSeriesInfo {
                external_id: "G1".to_string(),
                title: "Australia - GDP".to_string(),
                description: "Gross Domestic Product for Australia".to_string(),
                units: "Billions of Australian dollars".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "Australia".to_string(),
                data_url: format!("{}/g01-hist.html", self.base_url),
            },
        ])
    }
}

pub async fn discover_rba_series(_client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let rba_source = RbaDataSource::new();
    let series_list = rba_source.discover_series().await?;
    let rba_data_source = DataSource::get_or_create(pool, DataSource::rba()).await?;
    let mut discovered_series = Vec::new();

    for series_info in series_list {
        let new_metadata = NewSeriesMetadata {
            source_id: rba_data_source.id,
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
            rba_data_source.id,
            &series_info.external_id,
            &new_metadata,
        )
        .await
        {
            Ok(_) => discovered_series.push(series_info.external_id),
            Err(e) => eprintln!(
                "Failed to create RBA series {}: {}",
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
    async fn test_rba_data_source_creation() {
        let rba = RbaDataSource::new();
        assert_eq!(rba.base_url, "https://www.rba.gov.au/statistics");
    }

    #[tokio::test]
    async fn test_discover_series() {
        let rba = RbaDataSource::new();
        let series = rba.discover_series().await;
        assert!(series.is_ok());
        let series_list = series.unwrap();
        assert!(!series_list.is_empty());
        assert!(series_list.iter().any(|s| s.title.contains("Cash Rate")));
    }

    #[tokio::test]
    async fn test_discover_rba_series_integration() {
        let container = TestContainer::new().await;
        let pool = &container.pool;
        let discovered_series = discover_rba_series(&Client::new(), pool).await;
        assert!(discovered_series.is_ok());
        let series_ids = discovered_series.unwrap();
        assert!(!series_ids.is_empty());
    }
}
