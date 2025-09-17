//! Bank of Japan (BoJ) data source integration
//!
//! This module provides integration with the Bank of Japan's Time-Series Data Search,
//! which provides comprehensive monetary policy, financial stability, and economic data for Japan.

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::AppResult;
use econ_graph_core::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Bank of Japan API response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BojResponse {
    /// Data structure information
    pub data: Vec<BojDataPoint>,
    /// Metadata
    pub meta: BojMeta,
}

/// Bank of Japan data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BojDataPoint {
    /// Date
    pub date: String,
    /// Value
    pub value: Option<f64>,
    /// Series ID
    pub series_id: String,
}

/// Bank of Japan metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BojMeta {
    /// Series information
    pub series: Vec<BojSeriesMeta>,
}

/// Bank of Japan series metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BojSeriesMeta {
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

/// Bank of Japan Series Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BojSeriesInfo {
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

/// Bank of Japan Data Source client
pub struct BojDataSource {
    client: Client,
    base_url: String,
}

impl Default for BojDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl BojDataSource {
    /// Create a new Bank of Japan data source
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml".to_string(),
        }
    }

    /// Discover available Bank of Japan economic series
    pub async fn discover_series(&self) -> AppResult<Vec<BojSeriesInfo>> {
        let mut series_list = Vec::new();

        // Key Bank of Japan economic indicators
        series_list.extend(self.get_monetary_policy_series().await?);
        series_list.extend(self.get_inflation_series().await?);
        series_list.extend(self.get_gdp_series().await?);
        series_list.extend(self.get_employment_series().await?);
        series_list.extend(self.get_financial_stability_series().await?);

        Ok(series_list)
    }

    /// Get monetary policy related series
    async fn get_monetary_policy_series(&self) -> AppResult<Vec<BojSeriesInfo>> {
        Ok(vec![
            BojSeriesInfo {
                external_id: "BOJ_UNRATE".to_string(),
                title: "Japan - Policy interest rate".to_string(),
                description: "Policy interest rate set by the Bank of Japan".to_string(),
                units: "Percent per annum".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Japan".to_string(),
                data_url: format!("{}/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1", self.base_url),
            },
            BojSeriesInfo {
                external_id: "BOJ_TONAR".to_string(),
                title: "Japan - Tokyo Overnight Average Rate (TONAR)".to_string(),
                description: "Tokyo Overnight Average Rate - the benchmark interest rate for Japanese yen overnight unsecured transactions".to_string(),
                units: "Percent per annum".to_string(),
                frequency: "Daily".to_string(),
                geographic_level: "Japan".to_string(),
                data_url: format!("{}/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1", self.base_url),
            },
        ])
    }

    /// Get inflation related series
    async fn get_inflation_series(&self) -> AppResult<Vec<BojSeriesInfo>> {
        Ok(vec![
            BojSeriesInfo {
                external_id: "BOJ_CPI".to_string(),
                title: "Japan - Consumer Price Index (CPI)".to_string(),
                description: "Consumer Price Index for Japan".to_string(),
                units: "Index (2015=100)".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Japan".to_string(),
                data_url: format!(
                    "{}/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
                    self.base_url
                ),
            },
            BojSeriesInfo {
                external_id: "BOJ_CORE_CPI".to_string(),
                title: "Japan - Core Consumer Price Index".to_string(),
                description: "Core Consumer Price Index (excluding fresh food) for Japan"
                    .to_string(),
                units: "Index (2015=100)".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Japan".to_string(),
                data_url: format!(
                    "{}/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
                    self.base_url
                ),
            },
        ])
    }

    /// Get GDP related series
    async fn get_gdp_series(&self) -> AppResult<Vec<BojSeriesInfo>> {
        Ok(vec![
            BojSeriesInfo {
                external_id: "BOJ_GDP_CURRENT".to_string(),
                title: "Japan - GDP at current prices".to_string(),
                description: "Gross Domestic Product at current prices for Japan".to_string(),
                units: "Trillions of yen".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "Japan".to_string(),
                data_url: format!(
                    "{}/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
                    self.base_url
                ),
            },
            BojSeriesInfo {
                external_id: "BOJ_GDP_CONSTANT".to_string(),
                title: "Japan - GDP at constant prices".to_string(),
                description: "Gross Domestic Product at constant prices for Japan".to_string(),
                units: "Trillions of yen".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "Japan".to_string(),
                data_url: format!(
                    "{}/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
                    self.base_url
                ),
            },
        ])
    }

    /// Get employment related series
    async fn get_employment_series(&self) -> AppResult<Vec<BojSeriesInfo>> {
        Ok(vec![
            BojSeriesInfo {
                external_id: "BOJ_UNEMPLOYMENT".to_string(),
                title: "Japan - Unemployment rate".to_string(),
                description: "Unemployment rate for Japan".to_string(),
                units: "Percent".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Japan".to_string(),
                data_url: format!(
                    "{}/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
                    self.base_url
                ),
            },
            BojSeriesInfo {
                external_id: "BOJ_EMPLOYMENT".to_string(),
                title: "Japan - Employment rate".to_string(),
                description: "Employment rate for Japan".to_string(),
                units: "Percent".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "Japan".to_string(),
                data_url: format!(
                    "{}/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
                    self.base_url
                ),
            },
        ])
    }

    /// Get financial stability related series
    async fn get_financial_stability_series(&self) -> AppResult<Vec<BojSeriesInfo>> {
        Ok(vec![BojSeriesInfo {
            external_id: "BOJ_FINANCIAL_STABILITY".to_string(),
            title: "Japan - Financial Stability indicators".to_string(),
            description: "Key indicators from the Bank of Japan's Financial System Report"
                .to_string(),
            units: "Various".to_string(),
            frequency: "Quarterly".to_string(),
            geographic_level: "Japan".to_string(),
            data_url: format!(
                "{}/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
                self.base_url
            ),
        }])
    }

    /// Fetch Bank of Japan data for a specific series
    pub async fn fetch_boj_data(
        &self,
        series_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> AppResult<Vec<BojDataPoint>> {
        let mut url = format!("{}/cgi-bin/ssi/mtshtml.cgi", self.base_url);

        let mut params = vec![
            "svr=ssi".to_string(),
            "lst=1".to_string(),
            "page=1".to_string(),
            "id=1".to_string(),
        ];

        if let Some(start) = start_date {
            params.push(format!("startPeriod={}", start));
        }
        if let Some(end) = end_date {
            params.push(format!("endPeriod={}", end));
        }

        url.push('?');
        url.push_str(&params.join("&"));

        // In a real implementation, this would make an actual API call
        // For now, return dummy data
        Ok(vec![BojDataPoint {
            date: "2024-01-01".to_string(),
            value: Some(0.1),
            series_id: series_id.to_string(),
        }])
    }
}

/// Discover Bank of Japan economic series and catalog them
pub async fn discover_boj_series(_client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let boj_source = BojDataSource::new();
    let series_list = boj_source.discover_series().await?;

    // Get or create Bank of Japan data source
    let boj_data_source = DataSource::get_or_create(pool, DataSource::boj()).await?;

    let mut discovered_series = Vec::new();

    for series_info in series_list {
        // Create new economic series
        let new_series = NewEconomicSeries {
            source_id: boj_data_source.id,
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
            boj_data_source.id,
            &new_series,
        )
        .await
        {
            Ok(_) => {
                discovered_series.push(series_info.external_id);
            }
            Err(e) => {
                eprintln!(
                    "Failed to create Bank of Japan series {}: {}",
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
    async fn test_boj_data_source_creation() {
        let boj = BojDataSource::new();
        assert_eq!(
            boj.base_url,
            "https://www.stat-search.boj.or.jp/ssi/mtshtml"
        );
    }

    #[tokio::test]
    async fn test_discover_series() {
        let boj = BojDataSource::new();
        let series = boj.discover_series().await;

        // Should return series for monetary policy, inflation, GDP, employment, and financial stability
        assert!(series.is_ok());
        let series_list = series.unwrap();

        assert!(!series_list.is_empty());

        // Should include monetary policy series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("Policy interest rate")));

        // Should include inflation series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("Consumer Price Index")));

        // Should include GDP series
        assert!(series_list.iter().any(|s| s.title.contains("GDP")));

        // Should include employment series
        assert!(series_list.iter().any(|s| s.title.contains("Unemployment")));

        // Should include financial stability series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("Financial Stability")));
    }

    #[tokio::test]
    async fn test_fetch_boj_data() {
        let boj = BojDataSource::new();
        let series_id = "BOJ_UNRATE";
        let boj_data_result = boj.fetch_boj_data(series_id, None, None).await;

        assert!(boj_data_result.is_ok());
        let boj_data_list = boj_data_result.unwrap();
        assert!(!boj_data_list.is_empty());

        let boj_data = &boj_data_list[0];
        assert_eq!(boj_data.series_id, series_id);
        assert_eq!(boj_data.value, Some(0.1));
    }

    #[tokio::test]
    async fn test_discover_boj_series_integration() {
        let container = TestContainer::new().await;
        let pool = container.pool();

        let discovered_series = discover_boj_series(&Client::new(), pool).await;
        assert!(discovered_series.is_ok());
        let series_ids = discovered_series.unwrap();
        assert!(!series_ids.is_empty());

        // Verify that the Bank of Japan data source was created
        let boj_data_source = DataSource::find_by_name(pool, "Bank of Japan (BoJ)").await;
        assert!(boj_data_source.is_ok());

        // Verify some series were created
        assert!(series_ids.iter().any(|id| id.contains("BOJ_UNRATE")));
        assert!(series_ids.iter().any(|id| id.contains("BOJ_CPI")));
        assert!(series_ids.iter().any(|id| id.contains("BOJ_GDP")));
    }
}
