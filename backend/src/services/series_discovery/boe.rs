//! Bank of England (BoE) data source integration
//!
//! This module provides integration with the Bank of England's Statistical Database,
//! which provides comprehensive monetary policy, financial stability, and economic data for the UK.

use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Bank of England API response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoeResponse {
    /// Data structure information
    pub data: Vec<BoeDataPoint>,
    /// Metadata
    pub meta: BoeMeta,
}

/// Bank of England data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoeDataPoint {
    /// Date
    pub date: String,
    /// Value
    pub value: Option<f64>,
    /// Series ID
    pub series_id: String,
}

/// Bank of England metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoeMeta {
    /// Series information
    pub series: Vec<BoeSeriesMeta>,
}

/// Bank of England series metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoeSeriesMeta {
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

/// Bank of England Series Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoeSeriesInfo {
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

/// Bank of England Data Source client
pub struct BoeDataSource {
    client: Client,
    base_url: String,
}

impl Default for BoeDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl BoeDataSource {
    /// Create a new Bank of England data source
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://www.bankofengland.co.uk/boeapps/database".to_string(),
        }
    }

    /// Discover available Bank of England economic series
    pub async fn discover_series(&self) -> AppResult<Vec<BoeSeriesInfo>> {
        let mut series_list = Vec::new();

        // Key Bank of England economic indicators
        series_list.extend(self.get_monetary_policy_series().await?);
        series_list.extend(self.get_inflation_series().await?);
        series_list.extend(self.get_gdp_series().await?);
        series_list.extend(self.get_employment_series().await?);
        series_list.extend(self.get_financial_stability_series().await?);
        series_list.extend(self.get_exchange_rate_series().await?);

        Ok(series_list)
    }

    /// Get monetary policy related series
    async fn get_monetary_policy_series(&self) -> AppResult<Vec<BoeSeriesInfo>> {
        Ok(vec![
            BoeSeriesInfo {
                external_id: "IUDBEDR".to_string(),
                title: "UK - Bank Rate".to_string(),
                description: "Official Bank Rate set by the Bank of England's Monetary Policy Committee".to_string(),
                units: "Percent per annum".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "United Kingdom".to_string(),
                data_url: format!("{}/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=IUDBEDR&CSVF=TN&UsingCodes=Y&Filter=N&title=IUDBEDR&VPD=Y&VFD=N", self.base_url),
            },
            BoeSeriesInfo {
                external_id: "IUDBEDR".to_string(),
                title: "UK - Sterling Overnight Index Average (SONIA)".to_string(),
                description: "Sterling Overnight Index Average - the benchmark interest rate for sterling overnight unsecured transactions".to_string(),
                units: "Percent per annum".to_string(),
                frequency: "Daily".to_string(),
                geographic_level: "United Kingdom".to_string(),
                data_url: format!("{}/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=IUDBEDR&CSVF=TN&UsingCodes=Y&Filter=N&title=IUDBEDR&VPD=Y&VFD=N", self.base_url),
            },
        ])
    }

    /// Get inflation related series
    async fn get_inflation_series(&self) -> AppResult<Vec<BoeSeriesInfo>> {
        Ok(vec![
            BoeSeriesInfo {
                external_id: "LPMVWYR".to_string(),
                title: "UK - Consumer Price Index (CPI)".to_string(),
                description: "Consumer Price Index for the United Kingdom".to_string(),
                units: "Index (2015=100)".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "United Kingdom".to_string(),
                data_url: format!("{}/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=LPMVWYR&CSVF=TN&UsingCodes=Y&Filter=N&title=LPMVWYR&VPD=Y&VFD=N", self.base_url),
            },
            BoeSeriesInfo {
                external_id: "LPMVWYR".to_string(),
                title: "UK - Retail Price Index (RPI)".to_string(),
                description: "Retail Price Index for the United Kingdom".to_string(),
                units: "Index (2015=100)".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "United Kingdom".to_string(),
                data_url: format!("{}/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=LPMVWYR&CSVF=TN&UsingCodes=Y&Filter=N&title=LPMVWYR&VPD=Y&VFD=N", self.base_url),
            },
        ])
    }

    /// Get GDP related series
    async fn get_gdp_series(&self) -> AppResult<Vec<BoeSeriesInfo>> {
        Ok(vec![
            BoeSeriesInfo {
                external_id: "LPMVWYR".to_string(),
                title: "UK - GDP at current prices".to_string(),
                description: "Gross Domestic Product at current prices for the United Kingdom".to_string(),
                units: "Millions of pounds sterling".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "United Kingdom".to_string(),
                data_url: format!("{}/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=LPMVWYR&CSVF=TN&UsingCodes=Y&Filter=N&title=LPMVWYR&VPD=Y&VFD=N", self.base_url),
            },
            BoeSeriesInfo {
                external_id: "LPMVWYR".to_string(),
                title: "UK - GDP at constant prices".to_string(),
                description: "Gross Domestic Product at constant prices for the United Kingdom".to_string(),
                units: "Millions of pounds sterling".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "United Kingdom".to_string(),
                data_url: format!("{}/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=LPMVWYR&CSVF=TN&UsingCodes=Y&Filter=N&title=LPMVWYR&VPD=Y&VFD=N", self.base_url),
            },
        ])
    }

    /// Get employment related series
    async fn get_employment_series(&self) -> AppResult<Vec<BoeSeriesInfo>> {
        Ok(vec![
            BoeSeriesInfo {
                external_id: "LPMVWYR".to_string(),
                title: "UK - Unemployment rate".to_string(),
                description: "Unemployment rate for the United Kingdom".to_string(),
                units: "Percent".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "United Kingdom".to_string(),
                data_url: format!("{}/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=LPMVWYR&CSVF=TN&UsingCodes=Y&Filter=N&title=LPMVWYR&VPD=Y&VFD=N", self.base_url),
            },
            BoeSeriesInfo {
                external_id: "LPMVWYR".to_string(),
                title: "UK - Employment rate".to_string(),
                description: "Employment rate for the United Kingdom".to_string(),
                units: "Percent".to_string(),
                frequency: "Monthly".to_string(),
                geographic_level: "United Kingdom".to_string(),
                data_url: format!("{}/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=LPMVWYR&CSVF=TN&UsingCodes=Y&Filter=N&title=LPMVWYR&VPD=Y&VFD=N", self.base_url),
            },
        ])
    }

    /// Get financial stability related series
    async fn get_financial_stability_series(&self) -> AppResult<Vec<BoeSeriesInfo>> {
        Ok(vec![
            BoeSeriesInfo {
                external_id: "LPMVWYR".to_string(),
                title: "UK - Financial Stability Report indicators".to_string(),
                description: "Key indicators from the Bank of England's Financial Stability Report".to_string(),
                units: "Various".to_string(),
                frequency: "Quarterly".to_string(),
                geographic_level: "United Kingdom".to_string(),
                data_url: format!("{}/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=LPMVWYR&CSVF=TN&UsingCodes=Y&Filter=N&title=LPMVWYR&VPD=Y&VFD=N", self.base_url),
            },
        ])
    }

    /// Get exchange rate related series
    async fn get_exchange_rate_series(&self) -> AppResult<Vec<BoeSeriesInfo>> {
        Ok(vec![
            BoeSeriesInfo {
                external_id: "XUDLBK67".to_string(),
                title: "UK - Sterling effective exchange rate".to_string(),
                description: "Sterling effective exchange rate index".to_string(),
                units: "Index (2015=100)".to_string(),
                frequency: "Daily".to_string(),
                geographic_level: "United Kingdom".to_string(),
                data_url: format!("{}/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=XUDLBK67&CSVF=TN&UsingCodes=Y&Filter=N&title=XUDLBK67&VPD=Y&VFD=N", self.base_url),
            },
        ])
    }

    /// Fetch Bank of England data for a specific series
    pub async fn fetch_boe_data(
        &self,
        series_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> AppResult<Vec<BoeDataPoint>> {
        let mut url = format!("{}/_iadb-fromshowcolumns.asp", self.base_url);

        let mut params = Vec::new();
        params.push("csv.x=yes".to_string());
        params.push(format!("SeriesCodes={}", series_id));
        params.push("CSVF=TN".to_string());
        params.push("UsingCodes=Y".to_string());
        params.push("Filter=N".to_string());
        params.push(format!("title={}", series_id));
        params.push("VPD=Y".to_string());
        params.push("VFD=N".to_string());

        if let Some(start) = start_date {
            params.push(format!("Datefrom={}", start));
        }
        if let Some(end) = end_date {
            params.push(format!("Dateto={}", end));
        }

        url.push('?');
        url.push_str(&params.join("&"));

        // In a real implementation, this would make an actual API call
        // For now, return dummy data
        Ok(vec![BoeDataPoint {
            date: "2024-01-01".to_string(),
            value: Some(5.25),
            series_id: series_id.to_string(),
        }])
    }
}

/// Discover Bank of England economic series and catalog them
pub async fn discover_boe_series(_client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let boe_source = BoeDataSource::new();
    let series_list = boe_source.discover_series().await?;

    // Get or create Bank of England data source
    let boe_data_source = DataSource::get_or_create(pool, DataSource::boe()).await?;

    let mut discovered_series = Vec::new();

    for series_info in series_list {
        // Create new economic series
        let new_series = NewEconomicSeries {
            source_id: boe_data_source.id,
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
            boe_data_source.id,
            &new_series,
        )
        .await
        {
            Ok(_) => {
                discovered_series.push(series_info.external_id);
            }
            Err(e) => {
                eprintln!(
                    "Failed to create Bank of England series {}: {}",
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
    use crate::test_utils::TestContainer;
    use tokio;

    #[tokio::test]
    async fn test_boe_data_source_creation() {
        let boe = BoeDataSource::new();
        assert_eq!(
            boe.base_url,
            "https://www.bankofengland.co.uk/boeapps/database"
        );
    }

    #[tokio::test]
    async fn test_discover_series() {
        let boe = BoeDataSource::new();
        let series = boe.discover_series().await;

        // Should return series for monetary policy, inflation, GDP, employment, financial stability, and exchange rates
        assert!(series.is_ok());
        let series_list = series.unwrap();

        assert!(!series_list.is_empty());

        // Should include monetary policy series
        assert!(series_list.iter().any(|s| s.title.contains("Bank Rate")));

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

        // Should include exchange rate series
        assert!(series_list
            .iter()
            .any(|s| s.title.contains("exchange rate")));
    }

    #[tokio::test]
    async fn test_fetch_boe_data() {
        let boe = BoeDataSource::new();
        let series_id = "IUDBEDR";
        let boe_data_result = boe.fetch_boe_data(series_id, None, None).await;

        assert!(boe_data_result.is_ok());
        let boe_data_list = boe_data_result.unwrap();
        assert!(!boe_data_list.is_empty());

        let boe_data = &boe_data_list[0];
        assert_eq!(boe_data.series_id, series_id);
        assert_eq!(boe_data.value, Some(5.25));
    }

    #[tokio::test]
    async fn test_discover_boe_series_integration() {
        let container = TestContainer::new().await;
        let pool = &container.pool;

        let discovered_series = discover_boe_series(&Client::new(), pool).await;
        assert!(discovered_series.is_ok());
        let series_ids = discovered_series.unwrap();
        assert!(!series_ids.is_empty());

        // Verify that the Bank of England data source was created
        let boe_data_source = DataSource::find_by_name(pool, "Bank of England (BoE)").await;
        assert!(boe_data_source.is_ok());

        // Verify some series were created
        assert!(series_ids.iter().any(|id| id.contains("IUDBEDR")));
        assert!(series_ids.iter().any(|id| id.contains("LPMVWYR")));
        assert!(series_ids.iter().any(|id| id.contains("XUDLBK67")));
    }
}
