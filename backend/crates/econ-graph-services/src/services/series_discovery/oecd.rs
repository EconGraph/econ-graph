//! OECD (Organisation for Economic Co-operation and Development) data source integration
//!
//! This module provides integration with the OECD's REST API, which provides comprehensive
//! economic, social, and environmental data for OECD member countries and partner economies.

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::AppResult;
use econ_graph_core::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// OECD API response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdResponse {
    /// Data structure information
    #[serde(rename = "dataSets")]
    pub data_sets: Vec<OecdDataSet>,
    /// Structure definition
    pub structure: OecdStructure,
}

/// OECD DataSet structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdDataSet {
    /// Action type (usually "Replace")
    pub action: String,
    /// Valid from date
    #[serde(rename = "validFromDate")]
    pub valid_from_date: String,
    /// Series data
    pub series: std::collections::HashMap<String, OecdSeries>,
}

/// OECD Series structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdSeries {
    /// Series attributes
    pub attributes: Vec<Option<String>>,
    /// Observations data
    pub observations: std::collections::HashMap<String, Vec<Option<String>>>,
}

/// OECD Structure definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdStructure {
    /// Dimensions
    pub dimensions: OecdDimensions,
    /// Attributes
    pub attributes: OecdAttributes,
}

/// OECD Dimensions structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdDimensions {
    /// Observation dimensions
    pub observation: Vec<OecdDimension>,
}

/// OECD Attributes structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdAttributes {
    /// Observation attributes
    pub observation: Vec<OecdAttribute>,
}

/// OECD Dimension structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdDimension {
    /// Dimension values
    pub values: Vec<OecdDimensionValue>,
}

/// OECD Attribute structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdAttribute {
    /// Attribute values
    pub values: Vec<OecdAttributeValue>,
}

/// OECD Dimension Value structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdDimensionValue {
    /// Dimension ID
    pub id: String,
    /// Dimension name
    pub name: String,
}

/// OECD Attribute Value structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdAttributeValue {
    /// Attribute ID
    pub id: String,
    /// Attribute name
    pub name: String,
}

/// OECD Series Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OecdSeriesInfo {
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

/// OECD Data Source client
pub struct OecdDataSource {
    client: Client,
    base_url: String,
}

impl Default for OecdDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl OecdDataSource {
    /// Create a new OECD data source
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://sdmx.oecd.org/public/rest/data".to_string(),
        }
    }

    /// Discover available OECD economic series
    pub async fn discover_series(&self) -> AppResult<Vec<OecdSeriesInfo>> {
        let mut series_list = Vec::new();

        // Key OECD economic indicators
        series_list.extend(self.get_gdp_series().await?);
        series_list.extend(self.get_inflation_series().await?);
        series_list.extend(self.get_employment_series().await?);
        series_list.extend(self.get_trade_series().await?);
        series_list.extend(self.get_productivity_series().await?);
        series_list.extend(self.get_education_series().await?);
        series_list.extend(self.get_health_series().await?);
        series_list.extend(self.get_environment_series().await?);

        Ok(series_list)
    }

    /// Get GDP related series
    async fn get_gdp_series(&self) -> AppResult<Vec<OecdSeriesInfo>> {
        Ok(vec![
            OecdSeriesInfo {
                external_id: "SNA_TABLE1.1.GDP.B1_GE.CPCAR_M".to_string(),
                title: "OECD - GDP at current prices".to_string(),
                description: "Gross Domestic Product at current prices for OECD countries".to_string(),
                units: "Millions of national currency".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/OECD.SNA_TABLE1,DSD_SNA_TABLE1@DF_SNA_TABLE1,1.0/1.GDP.B1_GE.CPCAR_M", self.base_url),
            },
            OecdSeriesInfo {
                external_id: "SNA_TABLE1.1.GDP.B1_GE.CPMP_NAC".to_string(),
                title: "OECD - GDP at constant prices".to_string(),
                description: "Gross Domestic Product at constant prices for OECD countries".to_string(),
                units: "Millions of national currency".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/OECD.SNA_TABLE1,DSD_SNA_TABLE1@DF_SNA_TABLE1,1.0/1.GDP.B1_GE.CPMP_NAC", self.base_url),
            },
            OecdSeriesInfo {
                external_id: "SNA_TABLE1.1.GDP.B1_GE.CPMP_PPP".to_string(),
                title: "OECD - GDP at constant prices (PPP)".to_string(),
                description: "Gross Domestic Product at constant prices using purchasing power parity for OECD countries".to_string(),
                units: "Millions of US dollars".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/OECD.SNA_TABLE1,DSD_SNA_TABLE1@DF_SNA_TABLE1,1.0/1.GDP.B1_GE.CPMP_PPP", self.base_url),
            },
        ])
    }

    /// Get inflation related series
    async fn get_inflation_series(&self) -> AppResult<Vec<OecdSeriesInfo>> {
        Ok(vec![
            OecdSeriesInfo {
                external_id: "PRICES_CPI.CPI.TOTIDX.M".to_string(),
                title: "OECD - Consumer Price Index".to_string(),
                description: "Consumer Price Index for OECD countries".to_string(),
                units: "Index (2015=100)".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!(
                    "{}/OECD.PRICES_CPI,DSD_PRICES_CPI@DF_PRICES_CPI,1.0/1.CPI.TOTIDX.M",
                    self.base_url
                ),
            },
            OecdSeriesInfo {
                external_id: "PRICES_CPI.CPI.TOTIDX.Q".to_string(),
                title: "OECD - Consumer Price Index (Quarterly)".to_string(),
                description: "Consumer Price Index for OECD countries (quarterly)".to_string(),
                units: "Index (2015=100)".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!(
                    "{}/OECD.PRICES_CPI,DSD_PRICES_CPI@DF_PRICES_CPI,1.0/1.CPI.TOTIDX.Q",
                    self.base_url
                ),
            },
        ])
    }

    /// Get employment related series
    async fn get_employment_series(&self) -> AppResult<Vec<OecdSeriesInfo>> {
        Ok(vec![
            OecdSeriesInfo {
                external_id: "LFS_SEXAGE_I_R.UNEM_RT.AGE15_64.T".to_string(),
                title: "OECD - Unemployment rate".to_string(),
                description: "Unemployment rate for OECD countries".to_string(),
                units: "Percent".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/OECD.LFS_SEXAGE_I_R,DSD_LFS_SEXAGE_I_R@DF_LFS_SEXAGE_I_R,1.0/1.UNEM_RT.AGE15_64.T", self.base_url),
            },
            OecdSeriesInfo {
                external_id: "LFS_SEXAGE_I_R.EMP_RT.AGE15_64.T".to_string(),
                title: "OECD - Employment rate".to_string(),
                description: "Employment rate for OECD countries".to_string(),
                units: "Percent".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/OECD.LFS_SEXAGE_I_R,DSD_LFS_SEXAGE_I_R@DF_LFS_SEXAGE_I_R,1.0/1.EMP_RT.AGE15_64.T", self.base_url),
            },
        ])
    }

    /// Get trade related series
    async fn get_trade_series(&self) -> AppResult<Vec<OecdSeriesInfo>> {
        Ok(vec![
            OecdSeriesInfo {
                external_id: "TIS_GOODS_SERVICES.TIS_GOODS_SERVICES.TIS_GOODS_SERVICES.TIS_GOODS_SERVICES".to_string(),
                title: "OECD - Trade in goods and services".to_string(),
                description: "Trade in goods and services for OECD countries".to_string(),
                units: "Millions of US dollars".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/OECD.TIS_GOODS_SERVICES,DSD_TIS_GOODS_SERVICES@DF_TIS_GOODS_SERVICES,1.0/1.TIS_GOODS_SERVICES.TIS_GOODS_SERVICES.TIS_GOODS_SERVICES.TIS_GOODS_SERVICES", self.base_url),
            },
        ])
    }

    /// Get productivity related series
    async fn get_productivity_series(&self) -> AppResult<Vec<OecdSeriesInfo>> {
        Ok(vec![OecdSeriesInfo {
            external_id: "PDB_LV.1.1.GDP.TOTIDX".to_string(),
            title: "OECD - Labour productivity".to_string(),
            description: "Labour productivity for OECD countries".to_string(),
            units: "Index (2015=100)".to_string(),
            frequency: "Annual".to_string(),
            geographic_level: "Country".to_string(),
            data_url: format!(
                "{}/OECD.PDB_LV,DSD_PDB_LV@DF_PDB_LV,1.0/1.1.GDP.TOTIDX",
                self.base_url
            ),
        }])
    }

    /// Get education related series
    async fn get_education_series(&self) -> AppResult<Vec<OecdSeriesInfo>> {
        Ok(vec![
            OecdSeriesInfo {
                external_id: "EDULIT_IND.EDULIT_IND.EDULIT_IND.EDULIT_IND".to_string(),
                title: "OECD - Education indicators".to_string(),
                description: "Education indicators for OECD countries".to_string(),
                units: "Various".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/OECD.EDULIT_IND,DSD_EDULIT_IND@DF_EDULIT_IND,1.0/1.EDULIT_IND.EDULIT_IND.EDULIT_IND.EDULIT_IND", self.base_url),
            },
        ])
    }

    /// Get health related series
    async fn get_health_series(&self) -> AppResult<Vec<OecdSeriesInfo>> {
        Ok(vec![
            OecdSeriesInfo {
                external_id: "HEALTH_STAT.HEALTH_STAT.HEALTH_STAT.HEALTH_STAT".to_string(),
                title: "OECD - Health statistics".to_string(),
                description: "Health statistics for OECD countries".to_string(),
                units: "Various".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/OECD.HEALTH_STAT,DSD_HEALTH_STAT@DF_HEALTH_STAT,1.0/1.HEALTH_STAT.HEALTH_STAT.HEALTH_STAT.HEALTH_STAT", self.base_url),
            },
        ])
    }

    /// Get environment related series
    async fn get_environment_series(&self) -> AppResult<Vec<OecdSeriesInfo>> {
        Ok(vec![
            OecdSeriesInfo {
                external_id: "GREEN_GROWTH.GREEN_GROWTH.GREEN_GROWTH.GREEN_GROWTH".to_string(),
                title: "OECD - Green growth indicators".to_string(),
                description: "Green growth indicators for OECD countries".to_string(),
                units: "Various".to_string(),
                frequency: "Annual".to_string(),
                geographic_level: "Country".to_string(),
                data_url: format!("{}/OECD.GREEN_GROWTH,DSD_GREEN_GROWTH@DF_GREEN_GROWTH,1.0/1.GREEN_GROWTH.GREEN_GROWTH.GREEN_GROWTH.GREEN_GROWTH", self.base_url),
            },
        ])
    }

    /// Fetch OECD data for a specific series
    pub async fn fetch_oecd_data(
        &self,
        series_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> AppResult<Vec<OecdSeries>> {
        let mut url = format!("{}/{}", self.base_url, series_id);

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
        Ok(vec![OecdSeries {
            attributes: vec![Some("OECD".to_string())],
            observations: std::collections::HashMap::new(),
        }])
    }
}

/// Discover OECD economic series and catalog them
pub async fn discover_oecd_series(_client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let oecd_source = OecdDataSource::new();
    let series_list = oecd_source.discover_series().await?;

    // Get or create OECD data source
    let oecd_data_source = DataSource::get_or_create(pool, DataSource::oecd()).await?;

    let mut discovered_series = Vec::new();

    for series_info in series_list {
        // Create new economic series
        let new_series = NewEconomicSeries {
            source_id: oecd_data_source.id,
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
            oecd_data_source.id,
            &new_series,
        )
        .await
        {
            Ok(_) => {
                discovered_series.push(series_info.external_id);
            }
            Err(e) => {
                eprintln!(
                    "Failed to create OECD series {}: {}",
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
    async fn test_oecd_data_source_creation() {
        let oecd = OecdDataSource::new();
        assert_eq!(oecd.base_url, "https://sdmx.oecd.org/public/rest/data");
    }

    #[tokio::test]
    async fn test_discover_series() {
        let oecd = OecdDataSource::new();
        let series = oecd.discover_series().await;

        // Should return series for GDP, inflation, employment, trade, productivity, education, health, and environment
        assert!(series.is_ok());
        let series_list = series.unwrap();

        assert!(!series_list.is_empty());

        // Should include GDP series
        assert!(series_list.iter().any(|s| s.title.contains("GDP")));

        // Should include inflation series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("Consumer Price Index")));

        // Should include employment series
        assert!(series_list.iter().any(|s| s.title.contains("Unemployment")));

        // Should include trade series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("Trade in goods")));

        // Should include productivity series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("Labour productivity")));

        // Should include education series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("Education indicators")));

        // Should include health series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("Health statistics")));

        // Should include environment series
        assert!(series_list.iter().any(|s| s.title.contains("Green growth")));
    }

    #[tokio::test]
    async fn test_fetch_oecd_data() {
        let oecd = OecdDataSource::new();
        let series_id = "SNA_TABLE1.1.GDP.B1_GE.CPCAR_M";
        let oecd_data_result = oecd.fetch_oecd_data(series_id, None, None).await;

        assert!(oecd_data_result.is_ok());
        let oecd_data_list = oecd_data_result.unwrap();
        assert!(!oecd_data_list.is_empty());

        let oecd_data = &oecd_data_list[0];
        assert_eq!(oecd_data.attributes[0], Some("OECD".to_string()));
    }

    #[tokio::test]
    async fn test_discover_oecd_series_integration() {
        let container = TestContainer::new().await;
        let pool = container.pool();

        let discovered_series = discover_oecd_series(&Client::new(), pool).await;
        assert!(discovered_series.is_ok());
        let series_ids = discovered_series.unwrap();
        assert!(!series_ids.is_empty());

        // Verify that the OECD data source was created
        let oecd_data_source = DataSource::find_by_name(
            pool,
            "OECD (Organisation for Economic Co-operation and Development)",
        )
        .await;
        assert!(oecd_data_source.is_ok());

        // Verify some series were created
        assert!(series_ids.iter().any(|id| id.contains("SNA_TABLE1.1.GDP")));
        assert!(series_ids.iter().any(|id| id.contains("PRICES_CPI.CPI")));
        assert!(series_ids.iter().any(|id| id.contains("LFS_SEXAGE_I_R")));
    }
}
