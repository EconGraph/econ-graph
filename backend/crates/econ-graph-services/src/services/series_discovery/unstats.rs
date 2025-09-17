//! UN Statistics Division data source integration
//!
//! This module provides integration with the UN Statistics Division's database,
//! which provides comprehensive global economic, social, and environmental data.

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::AppResult;
use econ_graph_core::models::{DataSource, NewSeriesMetadata, SeriesMetadata};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// UN Statistics Division Series Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnstatsSeriesInfo {
    pub external_id: String,
    pub title: String,
    pub description: String,
    pub units: String,
    pub frequency: String,
    pub geographic_level: String,
    pub data_url: String,
}

/// UN Statistics Division Data Source client
pub struct UnstatsDataSource {
    client: Client,
    base_url: String,
}

impl Default for UnstatsDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl UnstatsDataSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://unstats.un.org".to_string(),
        }
    }

    pub async fn discover_series(&self) -> AppResult<Vec<UnstatsSeriesInfo>> {
        Ok(vec![
            UnstatsSeriesInfo {
                external_id: "UN_GDP".to_string(),
                title: "UN - GDP per capita".to_string(),
                description: "Gross Domestic Product per capita from UN Statistics Division"
                    .to_string(),
                units: "Current US dollars".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/unsd/snaama/Basic", self.base_url),
            },
            UnstatsSeriesInfo {
                external_id: "UN_POPULATION".to_string(),
                title: "UN - Total Population".to_string(),
                description: "Total population estimates from UN Statistics Division".to_string(),
                units: "Persons".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/unsd/demographic-social/products/dyb", self.base_url),
            },
            UnstatsSeriesInfo {
                external_id: "UN_LIFE_EXPECTANCY".to_string(),
                title: "UN - Life Expectancy at Birth".to_string(),
                description: "Life expectancy at birth from UN Statistics Division".to_string(),
                units: "Years".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/unsd/demographic-social/products/dyb", self.base_url),
            },
        ])
    }
}

pub async fn discover_unstats_series(
    _client: &Client,
    pool: &DatabasePool,
) -> AppResult<Vec<String>> {
    let unstats_source = UnstatsDataSource::new();
    let series_list = unstats_source.discover_series().await?;
    let unstats_data_source = DataSource::get_or_create(pool, DataSource::unstats()).await?;
    let mut discovered_series = Vec::new();

    for series_info in series_list {
        let new_metadata = NewSeriesMetadata {
            source_id: unstats_data_source.id,
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
            unstats_data_source.id,
            &series_info.external_id,
            &new_metadata,
        )
        .await
        {
            Ok(_) => discovered_series.push(series_info.external_id),
            Err(e) => eprintln!(
                "Failed to create UN Statistics series {}: {}",
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
    async fn test_unstats_data_source_creation() {
        let unstats = UnstatsDataSource::new();
        assert_eq!(unstats.base_url, "https://unstats.un.org");
    }

    #[tokio::test]
    async fn test_discover_series() {
        let unstats = UnstatsDataSource::new();
        let series = unstats.discover_series().await;
        assert!(series.is_ok());
        let series_list = series.unwrap();
        assert!(!series_list.is_empty());
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("GDP per capita")));
    }

    #[tokio::test]
    async fn test_discover_unstats_series_integration() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let discovered_series = discover_unstats_series(&Client::new(), pool).await;
        assert!(discovered_series.is_ok());
        let series_ids = discovered_series.unwrap();
        assert!(!series_ids.is_empty());
    }
}
