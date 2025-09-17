//! World Trade Organization (WTO) data source integration
//!
//! This module provides integration with the WTO's Trade Statistics Database,
//! which provides comprehensive international trade data for WTO member countries.

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::AppResult;
use econ_graph_core::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// WTO API response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WtoResponse {
    /// Data structure information
    pub data: Vec<WtoDataPoint>,
    /// Metadata
    pub meta: WtoMeta,
}

/// WTO data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WtoDataPoint {
    /// Date
    pub date: String,
    /// Value
    pub value: Option<f64>,
    /// Series ID
    pub series_id: String,
    /// Country
    pub country: String,
    /// Product
    pub product: String,
}

/// WTO metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WtoMeta {
    /// Series information
    pub series: Vec<WtoSeriesMeta>,
}

/// WTO series metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WtoSeriesMeta {
    /// Series ID
    pub id: String,
    /// Series title
    pub title: String,
    /// Series description
    pub description: String,
    /// Units
    pub units: String,
    /// Frequency
    pub frequency: String,
}

/// WTO Series Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WtoSeriesInfo {
    /// External series ID
    pub external_id: String,
    /// Series title
    pub title: String,
    /// Series description
    pub description: String,
    /// Units of measurement
    pub units: String,
    /// Data frequency
    pub frequency: String,
    /// Geographic coverage
    pub geographic_level: String,
    /// Data URL
    pub data_url: String,
}

/// WTO Data Source client
pub struct WtoDataSource {
    client: Client,
    base_url: String,
}

impl Default for WtoDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl WtoDataSource {
    /// Create a new WTO data source
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api.wto.org/timeseries/v1".to_string(),
        }
    }

    /// Discover available WTO economic series
    pub async fn discover_series(&self) -> AppResult<Vec<WtoSeriesInfo>> {
        let mut series_list = Vec::new();

        // Key WTO economic indicators
        series_list.extend(self.get_merchandise_trade_series().await?);
        series_list.extend(self.get_services_trade_series().await?);
        series_list.extend(self.get_trade_policy_series().await?);

        Ok(series_list)
    }

    /// Get merchandise trade related series
    async fn get_merchandise_trade_series(&self) -> AppResult<Vec<WtoSeriesInfo>> {
        Ok(vec![
            WtoSeriesInfo {
                external_id: "MT_GOODS_EXP".to_string(),
                title: "WTO - Merchandise exports".to_string(),
                description: "Merchandise exports for WTO member countries".to_string(),
                units: "Millions of US dollars".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/data/MT_GOODS_EXP", self.base_url),
            },
            WtoSeriesInfo {
                external_id: "MT_GOODS_IMP".to_string(),
                title: "WTO - Merchandise imports".to_string(),
                description: "Merchandise imports for WTO member countries".to_string(),
                units: "Millions of US dollars".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/data/MT_GOODS_IMP", self.base_url),
            },
        ])
    }

    /// Get services trade related series
    async fn get_services_trade_series(&self) -> AppResult<Vec<WtoSeriesInfo>> {
        Ok(vec![
            WtoSeriesInfo {
                external_id: "ST_SERVICES_EXP".to_string(),
                title: "WTO - Services exports".to_string(),
                description: "Services exports for WTO member countries".to_string(),
                units: "Millions of US dollars".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/data/ST_SERVICES_EXP", self.base_url),
            },
            WtoSeriesInfo {
                external_id: "ST_SERVICES_IMP".to_string(),
                title: "WTO - Services imports".to_string(),
                description: "Services imports for WTO member countries".to_string(),
                units: "Millions of US dollars".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/data/ST_SERVICES_IMP", self.base_url),
            },
        ])
    }

    /// Get trade policy related series
    async fn get_trade_policy_series(&self) -> AppResult<Vec<WtoSeriesInfo>> {
        Ok(vec![WtoSeriesInfo {
            external_id: "TP_TARIFFS".to_string(),
            title: "WTO - Applied tariffs".to_string(),
            description: "Applied tariffs for WTO member countries".to_string(),
            units: "Percent".to_string(),
            frequency: "Annual".to_string(),
            geographic_level: "Country".to_string(),
            data_url: format!("{}/data/TP_TARIFFS", self.base_url),
        }])
    }

    /// Fetch WTO data for a specific series
    pub async fn fetch_wto_data(
        &self,
        series_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> AppResult<Vec<WtoDataPoint>> {
        let mut url = format!("{}/data/{}", self.base_url, series_id);

        let mut params = Vec::new();
        if let Some(start) = start_date {
            params.push(format!("startPeriod={}", start));
        }
        if let Some(end) = end_date {
            params.push(format!("endPeriod={}", end));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        // In a real implementation, this would make an actual API call
        // For now, return dummy data
        Ok(vec![WtoDataPoint {
            date: "2024-01-01".to_string(),
            value: Some(1000.0),
            series_id: series_id.to_string(),
            country: "USA".to_string(),
            product: "All".to_string(),
        }])
    }
}

/// Discover WTO economic series and catalog them
pub async fn discover_wto_series(_client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let wto_source = WtoDataSource::new();
    let series_list = wto_source.discover_series().await?;

    // Get or create WTO data source
    let wto_data_source = DataSource::get_or_create(pool, DataSource::wto()).await?;

    let mut discovered_series = Vec::new();

    for series_info in series_list {
        // Create new economic series
        let new_series = NewEconomicSeries {
            source_id: wto_data_source.id,
            external_id: series_info.external_id.clone(),
            title: series_info.title,
            description: Some(series_info.description),
            units: Some(series_info.units),
            frequency: series_info.frequency,
            seasonal_adjustment: None,
            start_date: None,
            end_date: None,
            is_active: true,
            first_discovered_at: Some(chrono::Utc::now()),
            last_crawled_at: None,
            first_missing_date: None,
            crawl_status: None,
            crawl_error_message: None,
        };

        // Get or create the series
        match EconomicSeries::get_or_create(
            pool,
            &series_info.external_id,
            wto_data_source.id,
            &new_series,
        )
        .await
        {
            Ok(_) => {
                discovered_series.push(series_info.external_id);
            }
            Err(e) => {
                eprintln!(
                    "Failed to create WTO series {}: {}",
                    series_info.external_id, e
                );
            }
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
    async fn test_wto_data_source_creation() {
        let wto = WtoDataSource::new();
        assert_eq!(wto.base_url, "https://api.wto.org/timeseries/v1");
    }

    #[tokio::test]
    async fn test_discover_series() {
        let wto = WtoDataSource::new();
        let series = wto.discover_series().await;

        // Should return series for merchandise trade, services trade, and trade policy
        assert!(series.is_ok());
        let series_list = series.unwrap();

        assert!(!series_list.is_empty());

        // Should include merchandise trade series
        assert!(series_list.iter().any(|s| s.title.contains("Merchandise")));

        // Should include services trade series
        assert!(series_list.iter().any(|s| s.title.contains("Services")));

        // Should include trade policy series
        assert!(series_list.iter().any(|s| s.title.contains("tariffs")));
    }

    #[tokio::test]
    async fn test_fetch_wto_data() {
        let wto = WtoDataSource::new();
        let series_id = "MT_GOODS_EXP";
        let wto_data_result = wto.fetch_wto_data(series_id, None, None).await;

        assert!(wto_data_result.is_ok());
        let wto_data_list = wto_data_result.unwrap();
        assert!(!wto_data_list.is_empty());

        let wto_data = &wto_data_list[0];
        assert_eq!(wto_data.series_id, series_id);
        assert_eq!(wto_data.value, Some(1000.0));
    }

    #[tokio::test]
    async fn test_discover_wto_series_integration() {
        let container = TestContainer::new().await;
        let pool = container.pool();

        let discovered_series = discover_wto_series(&Client::new(), pool).await;
        assert!(discovered_series.is_ok());
        let series_ids = discovered_series.unwrap();
        assert!(!series_ids.is_empty());

        // Verify that the WTO data source was created
        let wto_data_source =
            DataSource::find_by_name(pool, "World Trade Organization (WTO)").await;
        assert!(wto_data_source.is_ok());

        // Verify some series were created
        assert!(series_ids.iter().any(|id| id.contains("MT_GOODS")));
        assert!(series_ids.iter().any(|id| id.contains("ST_SERVICES")));
        assert!(series_ids.iter().any(|id| id.contains("TP_TARIFFS")));
    }
}
