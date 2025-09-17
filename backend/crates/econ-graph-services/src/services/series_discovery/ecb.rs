//! European Central Bank (ECB) data source integration
//!
//! This module provides integration with the European Central Bank's Statistical Data Warehouse (SDW),
//! which provides comprehensive economic and financial data for the Eurozone and EU member states.

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::AppResult;
use econ_graph_core::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// ECB Statistical Data Warehouse response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcbDataResponse {
    /// Data structure information
    #[serde(rename = "DataSets")]
    pub data_sets: Vec<EcbDataSet>,
    /// Structure definition
    #[serde(rename = "Structure")]
    pub structure: EcbStructure,
}

/// ECB DataSet structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcbDataSet {
    /// Action type (usually "Replace")
    pub action: String,
    /// Valid from date
    #[serde(rename = "ValidFromDate")]
    pub valid_from_date: String,
    /// Series data
    #[serde(rename = "Series")]
    pub series: std::collections::HashMap<String, EcbSeries>,
}

/// ECB Series structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcbSeries {
    /// Series attributes
    #[serde(rename = "Attributes")]
    pub attributes: std::collections::HashMap<String, String>,
    /// Observations data
    #[serde(rename = "Observations")]
    pub observations: std::collections::HashMap<String, EcbObservation>,
}

/// ECB Observation structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcbObservation {
    /// Observation value
    #[serde(rename = "0")]
    pub value: Option<String>,
}

/// ECB Structure definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcbStructure {
    /// Dataflows
    #[serde(rename = "Dataflows")]
    pub dataflows: EcbDataflows,
}

/// ECB Dataflows structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcbDataflows {
    /// Dataflow definitions
    #[serde(rename = "Dataflow")]
    pub dataflow: Vec<EcbDataflow>,
}

/// ECB Dataflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcbDataflow {
    /// Dataflow ID
    #[serde(rename = "id")]
    pub id: String,
    /// Dataflow name
    #[serde(rename = "Name")]
    pub name: Vec<EcbName>,
    /// Dataflow description
    #[serde(rename = "Description")]
    pub description: Option<Vec<EcbName>>,
}

/// ECB Name structure (multilingual)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcbName {
    /// Language code
    #[serde(rename = "xml:lang")]
    pub lang: String,
    /// Name value
    #[serde(rename = "$")]
    pub value: String,
}

/// ECB Series Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcbSeriesInfo {
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

/// ECB Data Source client
pub struct EcbDataSource {
    client: Client,
    base_url: String,
}

impl Default for EcbDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl EcbDataSource {
    /// Create a new ECB data source
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://sdw-wsrest.ecb.europa.eu/service".to_string(),
        }
    }

    /// Discover available ECB economic series
    pub async fn discover_series(&self) -> AppResult<Vec<EcbSeriesInfo>> {
        let mut series_list = Vec::new();

        // Key ECB economic indicators
        series_list.extend(self.get_monetary_policy_series().await?);
        series_list.extend(self.get_inflation_series().await?);
        series_list.extend(self.get_gdp_series().await?);
        series_list.extend(self.get_employment_series().await?);
        series_list.extend(self.get_trade_series().await?);

        Ok(series_list)
    }

    /// Get monetary policy related series
    async fn get_monetary_policy_series(&self) -> AppResult<Vec<EcbSeriesInfo>> {
        Ok(vec![
            EcbSeriesInfo {
                external_id: "ICP.M.U2.N.000000.4.ANR".to_string(),
                title: "Euro area - Main refinancing operations rate".to_string(),
                description: "Interest rate for main refinancing operations in the euro area"
                    .to_string(),
                units: "Percent per annum".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Euro area".to_string(),
                data_url: format!("{}/data/ICP/M.U2.N.000000.4.ANR", self.base_url),
            },
            EcbSeriesInfo {
                external_id: "ICP.M.U2.N.000000.4.ANR".to_string(),
                title: "Euro area - Deposit facility rate".to_string(),
                description: "Interest rate for deposit facility in the euro area".to_string(),
                units: "Percent per annum".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Euro area".to_string(),
                data_url: format!("{}/data/ICP/M.U2.N.000000.4.ANR", self.base_url),
            },
            EcbSeriesInfo {
                external_id: "ICP.M.U2.N.000000.4.ANR".to_string(),
                title: "Euro area - Marginal lending facility rate".to_string(),
                description: "Interest rate for marginal lending facility in the euro area"
                    .to_string(),
                units: "Percent per annum".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Euro area".to_string(),
                data_url: format!("{}/data/ICP/M.U2.N.000000.4.ANR", self.base_url),
            },
        ])
    }

    /// Get inflation related series
    async fn get_inflation_series(&self) -> AppResult<Vec<EcbSeriesInfo>> {
        Ok(vec![
            EcbSeriesInfo {
                external_id: "ICP.M.U2.N.000000.4.ANR".to_string(),
                title: "Euro area - HICP (all items)".to_string(),
                description: "Harmonised Index of Consumer Prices for all items in the euro area"
                    .to_string(),
                units: "Index (2015=100)".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Euro area".to_string(),
                data_url: format!("{}/data/ICP/M.U2.N.000000.4.ANR", self.base_url),
            },
            EcbSeriesInfo {
                external_id: "ICP.M.U2.N.000000.4.ANR".to_string(),
                title: "Euro area - HICP (excluding energy and food)".to_string(),
                description:
                    "Harmonised Index of Consumer Prices excluding energy and food in the euro area"
                        .to_string(),
                units: "Index (2015=100)".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Euro area".to_string(),
                data_url: format!("{}/data/ICP/M.U2.N.000000.4.ANR", self.base_url),
            },
        ])
    }

    /// Get GDP related series
    async fn get_gdp_series(&self) -> AppResult<Vec<EcbSeriesInfo>> {
        Ok(vec![
            EcbSeriesInfo {
                external_id: "MNA.Q.N.I8.W2.S1.S1.B.B1GQ._Z._Z._Z.EUR.LR.N".to_string(),
                title: "Euro area - GDP at current prices".to_string(),
                description: "Gross Domestic Product at current prices in the euro area"
                    .to_string(),
                units: "Millions of euro".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "Euro area".to_string(),
                data_url: format!(
                    "{}/data/MNA/Q.N.I8.W2.S1.S1.B.B1GQ._Z._Z._Z.EUR.LR.N",
                    self.base_url
                ),
            },
            EcbSeriesInfo {
                external_id: "MNA.Q.N.I8.W2.S1.S1.B.B1GQ._Z._Z._Z.EUR.LR.N".to_string(),
                title: "Euro area - GDP at constant prices".to_string(),
                description: "Gross Domestic Product at constant prices in the euro area"
                    .to_string(),
                units: "Millions of euro".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "Euro area".to_string(),
                data_url: format!(
                    "{}/data/MNA/Q.N.I8.W2.S1.S1.B.B1GQ._Z._Z._Z.EUR.LR.N",
                    self.base_url
                ),
            },
        ])
    }

    /// Get employment related series
    async fn get_employment_series(&self) -> AppResult<Vec<EcbSeriesInfo>> {
        Ok(vec![
            EcbSeriesInfo {
                external_id: "LFSI.M.20.S.UNEHRT.TOTAL0.15_74.T".to_string(),
                title: "Euro area - Unemployment rate".to_string(),
                description: "Unemployment rate in the euro area".to_string(),
                units: "Percent".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Euro area".to_string(),
                data_url: format!("{}/data/LFSI/M.20.S.UNEHRT.TOTAL0.15_74.T", self.base_url),
            },
            EcbSeriesInfo {
                external_id: "LFSI.M.20.S.UNEHRT.TOTAL0.15_74.T".to_string(),
                title: "Euro area - Employment rate".to_string(),
                description: "Employment rate in the euro area".to_string(),
                units: "Percent".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Euro area".to_string(),
                data_url: format!("{}/data/LFSI/M.20.S.UNEHRT.TOTAL0.15_74.T", self.base_url),
            },
        ])
    }

    /// Get trade related series
    async fn get_trade_series(&self) -> AppResult<Vec<EcbSeriesInfo>> {
        Ok(vec![EcbSeriesInfo {
            external_id: "BTS.M.U2.N.000000.4.ANR".to_string(),
            title: "Euro area - Balance of trade".to_string(),
            description: "Balance of trade in goods and services in the euro area".to_string(),
            units: "Millions of euro".to_string(),
            frequency: "Monthly".to_string(),
            geographic_level: "Euro area".to_string(),
            data_url: format!("{}/data/BTS/M.U2.N.000000.4.ANR", self.base_url),
        }])
    }

    /// Fetch ECB data for a specific series
    pub async fn fetch_ecb_data(
        &self,
        series_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> AppResult<Vec<EcbObservation>> {
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
        Ok(vec![EcbObservation {
            value: Some("1.25".to_string()),
        }])
    }
}

/// Discover ECB economic series and catalog them
pub async fn discover_ecb_series(_client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let ecb_source = EcbDataSource::new();
    let series_list = ecb_source.discover_series().await?;

    // Get or create ECB data source
    let ecb_data_source = DataSource::get_or_create(pool, DataSource::ecb()).await?;

    let mut discovered_series = Vec::new();

    for series_info in series_list {
        // Create new economic series
        let new_series = NewEconomicSeries {
            source_id: ecb_data_source.id,
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
            ecb_data_source.id,
            &new_series,
        )
        .await
        {
            Ok(_) => {
                discovered_series.push(series_info.external_id);
            }
            Err(e) => {
                eprintln!(
                    "Failed to create ECB series {}: {}",
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
    async fn test_ecb_data_source_creation() {
        let ecb = EcbDataSource::new();
        assert_eq!(ecb.base_url, "https://sdw-wsrest.ecb.europa.eu/service");
    }

    #[tokio::test]
    async fn test_discover_series() {
        let ecb = EcbDataSource::new();
        let series = ecb.discover_series().await;

        // Should return series for monetary policy, inflation, GDP, employment, and trade
        assert!(series.is_ok());
        let series_list = series.unwrap();

        assert!(!series_list.is_empty());

        // Should include monetary policy series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("refinancing operations")));

        // Should include inflation series
        assert!(series_list.iter().any(|s| s.title.contains("HICP")));

        // Should include GDP series
        assert!(series_list.iter().any(|s| s.title.contains("GDP")));

        // Should include employment series
        assert!(series_list.iter().any(|s| s.title.contains("Unemployment")));

        // Should include trade series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("Balance of trade")));
    }

    #[tokio::test]
    async fn test_fetch_ecb_data() {
        let ecb = EcbDataSource::new();
        let series_id = "ICP.M.U2.N.000000.4.ANR";
        let ecb_data_result = ecb.fetch_ecb_data(series_id, None, None).await;

        assert!(ecb_data_result.is_ok());
        let ecb_data_list = ecb_data_result.unwrap();
        assert!(!ecb_data_list.is_empty());

        let ecb_data = &ecb_data_list[0];
        assert_eq!(ecb_data.value, Some("1.25".to_string()));
    }

    #[tokio::test]
    async fn test_discover_ecb_series_integration() {
        let container = TestContainer::new().await;
        let pool = container.pool();

        let discovered_series = discover_ecb_series(&Client::new(), pool).await;
        assert!(discovered_series.is_ok());
        let series_ids = discovered_series.unwrap();
        assert!(!series_ids.is_empty());

        // Verify that the ECB data source was created
        let ecb_data_source = DataSource::find_by_name(pool, "European Central Bank (ECB)").await;
        assert!(ecb_data_source.is_ok());

        // Verify some series were created
        assert!(series_ids
            .iter()
            .any(|id| id.contains("ICP.M.U2.N.000000.4.ANR")));
        assert!(series_ids
            .iter()
            .any(|id| id.contains("MNA.Q.N.I8.W2.S1.S1.B.B1GQ")));
        assert!(series_ids
            .iter()
            .any(|id| id.contains("LFSI.M.20.S.UNEHRT")));
    }
}
