//! International Labour Organization (ILO) data source integration
//!
//! This module provides integration with the International Labour Organization's database,
//! which provides comprehensive global labor market and employment data.

use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::models::{DataSource, NewSeriesMetadata, SeriesMetadata};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// International Labour Organization Series Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IloSeriesInfo {
    pub external_id: String,
    pub title: String,
    pub description: String,
    pub units: String,
    pub frequency: String,
    pub geographic_level: String,
    pub data_url: String,
}

/// International Labour Organization Data Source client
pub struct IloDataSource {
    client: Client,
    base_url: String,
}

impl Default for IloDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl IloDataSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://www.ilo.org".to_string(),
        }
    }

    pub async fn discover_series(&self) -> AppResult<Vec<IloSeriesInfo>> {
        Ok(vec![
            IloSeriesInfo {
                external_id: "ILO_UNEMPLOYMENT".to_string(),
                title: "ILO - Unemployment Rate".to_string(),
                description: "Unemployment rate from International Labour Organization".to_string(),
                units: "Percent".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!(
                    "{}/global/statistics-and-databases/lang--en/index.htm",
                    self.base_url
                ),
            },
            IloSeriesInfo {
                external_id: "ILO_EMPLOYMENT".to_string(),
                title: "ILO - Employment Rate".to_string(),
                description: "Employment rate from International Labour Organization".to_string(),
                units: "Percent".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!(
                    "{}/global/statistics-and-databases/lang--en/index.htm",
                    self.base_url
                ),
            },
            IloSeriesInfo {
                external_id: "ILO_LABOR_FORCE".to_string(),
                title: "ILO - Labor Force Participation Rate".to_string(),
                description:
                    "Labor force participation rate from International Labour Organization"
                        .to_string(),
                units: "Percent".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!(
                    "{}/global/statistics-and-databases/lang--en/index.htm",
                    self.base_url
                ),
            },
        ])
    }
}

pub async fn discover_ilo_series(_client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let ilo_source = IloDataSource::new();
    let series_list = ilo_source.discover_series().await?;
    let ilo_data_source = DataSource::get_or_create(pool, DataSource::ilo()).await?;
    let mut discovered_series = Vec::new();

    for series_info in series_list {
        let new_metadata = NewSeriesMetadata {
            source_id: ilo_data_source.id,
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
            ilo_data_source.id,
            &series_info.external_id,
            &new_metadata,
        )
        .await
        {
            Ok(_) => discovered_series.push(series_info.external_id),
            Err(e) => eprintln!(
                "Failed to create ILO series {}: {}",
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
    async fn test_ilo_data_source_creation() {
        let ilo = IloDataSource::new();
        assert_eq!(ilo.base_url, "https://www.ilo.org");
    }

    #[tokio::test]
    async fn test_discover_series() {
        let ilo = IloDataSource::new();
        let series = ilo.discover_series().await;
        assert!(series.is_ok());
        let series_list = series.unwrap();
        assert!(!series_list.is_empty());
        assert!(series_list.iter().any(|s| s.title.contains("Unemployment")));
    }

    #[tokio::test]
    async fn test_discover_ilo_series_integration() {
        let container = TestContainer::new().await;
        let pool = &container.pool;
        let discovered_series = discover_ilo_series(&Client::new(), pool).await;
        assert!(discovered_series.is_ok());
        let series_ids = discovered_series.unwrap();
        assert!(!series_ids.is_empty());
    }
}
