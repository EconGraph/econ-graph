use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::models::{
    CrawlQueueItem, DataPoint, DataSource, EconomicSeries, NewCrawlQueueItem, NewDataPoint,
    NewEconomicSeries, QueuePriority,
};

/// FRED API response for series metadata
#[derive(Debug, Deserialize)]
pub struct FredSeriesResponse {
    pub seriess: Vec<FredSeries>,
}

#[derive(Debug, Deserialize)]
pub struct FredSeries {
    pub id: String,
    pub title: String,
    pub observation_start: String,
    pub observation_end: String,
    pub frequency: String,
    pub frequency_short: String,
    pub units: String,
    pub units_short: String,
    pub seasonal_adjustment: String,
    pub seasonal_adjustment_short: String,
    pub last_updated: String,
    pub popularity: i32,
    pub notes: Option<String>,
}

/// FRED API response for observations
#[derive(Debug, Deserialize)]
pub struct FredObservationsResponse {
    pub observations: Vec<FredObservation>,
}

#[derive(Debug, Deserialize)]
pub struct FredObservation {
    pub realtime_start: String,
    pub realtime_end: String,
    pub date: String,
    pub value: String,
}

/// BLS API response
#[derive(Debug, Deserialize)]
pub struct BlsResponse {
    pub status: String,
    pub message: Vec<String>,
    #[serde(rename = "Results")]
    pub results: Option<BlsResults>,
}

#[derive(Debug, Deserialize)]
pub struct BlsResults {
    pub series: Vec<BlsSeries>,
}

#[derive(Debug, Deserialize)]
pub struct BlsSeries {
    #[serde(rename = "seriesID")]
    pub series_id: String,
    pub data: Vec<BlsDataPoint>,
}

#[derive(Debug, Deserialize)]
pub struct BlsDataPoint {
    pub year: String,
    pub period: String,
    #[serde(rename = "periodName")]
    pub period_name: String,
    pub value: String,
    pub footnotes: Vec<BlsFootnote>,
}

#[derive(Debug, Deserialize)]
pub struct BlsFootnote {
    pub code: String,
    pub text: String,
}

/// Crawler service for fetching economic data from external APIs
pub struct CrawlerService {
    client: Client,
    fred_api_key: Option<String>,
    bls_api_key: Option<String>,
}

impl CrawlerService {
    /// Create new crawler service
    pub fn new(fred_api_key: Option<String>, bls_api_key: Option<String>) -> Self {
        Self {
            client: Client::new(),
            fred_api_key,
            bls_api_key,
        }
    }

    /// Get or create FRED data source
    async fn get_or_create_fred_source(&self, pool: &DatabasePool) -> AppResult<DataSource> {
        let fred_source = DataSource::fred();
        DataSource::get_or_create(pool, fred_source).await
    }

    /// Get or create BLS data source
    async fn get_or_create_bls_source(&self, pool: &DatabasePool) -> AppResult<DataSource> {
        let bls_source = DataSource::bls();
        DataSource::get_or_create(pool, bls_source).await
    }

    /// Fetch FRED series metadata
    async fn fetch_fred_series_metadata(&self, series_id: &str) -> AppResult<FredSeries> {
        let api_key = self
            .fred_api_key
            .as_ref()
            .ok_or_else(|| AppError::ExternalApiError("FRED API key not configured".to_string()))?;

        let url = format!(
            "https://api.stlouisfed.org/fred/series?series_id={}&api_key={}&file_type=json",
            series_id, api_key
        );

        let response =
            self.client.get(&url).send().await.map_err(|e| {
                AppError::ExternalApiError(format!("FRED API request failed: {}", e))
            })?;

        if !response.status().is_success() {
            return Err(AppError::ExternalApiError(format!(
                "FRED API returned status: {}",
                response.status()
            )));
        }

        let fred_response: FredSeriesResponse = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse FRED response: {}", e))
        })?;

        fred_response.seriess.into_iter().next().ok_or_else(|| {
            AppError::ExternalApiError("No series found in FRED response".to_string())
        })
    }

    /// Fetch FRED observations
    async fn fetch_fred_observations(&self, series_id: &str) -> AppResult<Vec<FredObservation>> {
        let api_key = self
            .fred_api_key
            .as_ref()
            .ok_or_else(|| AppError::ExternalApiError("FRED API key not configured".to_string()))?;

        let url = format!(
            "https://api.stlouisfed.org/fred/series/observations?series_id={}&api_key={}&file_type=json&realtime_start=1776-07-04&realtime_end=9999-12-31",
            series_id, api_key
        );

        let response = self.client.get(&url).send().await.map_err(|e| {
            AppError::ExternalApiError(format!("FRED observations request failed: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(AppError::ExternalApiError(format!(
                "FRED observations API returned status: {}",
                response.status()
            )));
        }

        let fred_response: FredObservationsResponse = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse FRED observations: {}", e))
        })?;

        Ok(fred_response.observations)
    }

    /// Fetch BLS data
    async fn fetch_bls_data(
        &self,
        series_ids: &[String],
        start_year: i32,
        end_year: i32,
    ) -> AppResult<Vec<BlsSeries>> {
        let url = "https://api.bls.gov/publicAPI/v2/timeseries/data/";

        let request_body = serde_json::json!({
            "seriesid": series_ids,
            "startyear": start_year.to_string(),
            "endyear": end_year.to_string(),
            "registrationkey": self.bls_api_key
        });

        let response = self
            .client
            .post(url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AppError::ExternalApiError(format!("BLS API request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::ExternalApiError(format!(
                "BLS API returned status: {}",
                response.status()
            )));
        }

        let bls_response: BlsResponse = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse BLS response: {}", e))
        })?;

        if bls_response.status != "REQUEST_SUCCEEDED" {
            return Err(AppError::ExternalApiError(format!(
                "BLS API error: {}",
                bls_response.message.join(", ")
            )));
        }

        Ok(bls_response.results.map(|r| r.series).unwrap_or_default())
    }

    /// Upsert economic series from FRED data
    async fn upsert_economic_series(
        &self,
        pool: &DatabasePool,
        fred_series: &FredSeries,
        data_source_id: Uuid,
    ) -> AppResult<EconomicSeries> {
        // Try to find existing series
        match EconomicSeries::find_by_external_id(pool, &fred_series.id, data_source_id).await {
            Ok(existing) => {
                // Update existing series
                let update_data = crate::models::UpdateEconomicSeries {
                    title: Some(fred_series.title.clone()),
                    description: fred_series.notes.clone(),
                    units: Some(fred_series.units.clone()),
                    seasonal_adjustment: Some(fred_series.seasonal_adjustment.clone()),
                    last_updated: Some(
                        DateTime::parse_from_str(&fred_series.last_updated, "%Y-%m-%d %H:%M:%S%z")
                            .map_err(|e| {
                                AppError::ExternalApiError(format!(
                                    "Failed to parse FRED last_updated: {}",
                                    e
                                ))
                            })?
                            .with_timezone(&Utc),
                    ),
                    updated_at: Utc::now(),
                    ..Default::default()
                };

                EconomicSeries::update(pool, existing.id, &update_data).await
            }
            Err(_) => {
                // Create new series
                let new_series = NewEconomicSeries {
                    source_id: data_source_id,
                    external_id: fred_series.id.clone(),
                    title: fred_series.title.clone(),
                    description: fred_series.notes.clone(),
                    units: Some(fred_series.units.clone()),
                    frequency: fred_series.frequency.clone(),
                    seasonal_adjustment: Some(fred_series.seasonal_adjustment.clone()),
                    start_date: Some(
                        NaiveDate::parse_from_str(&fred_series.observation_start, "%Y-%m-%d")
                            .map_err(|e| {
                                AppError::ExternalApiError(format!(
                                    "Failed to parse FRED start date: {}",
                                    e
                                ))
                            })?,
                    ),
                    end_date: Some(
                        NaiveDate::parse_from_str(&fred_series.observation_end, "%Y-%m-%d")
                            .map_err(|e| {
                                AppError::ExternalApiError(format!(
                                    "Failed to parse FRED end date: {}",
                                    e
                                ))
                            })?,
                    ),
                    is_active: true,
                    first_discovered_at: Some(chrono::Utc::now()),
                    last_crawled_at: None,
                    first_missing_date: None,
                    crawl_status: None,
                    crawl_error_message: None,
                };

                EconomicSeries::create(pool, &new_series).await
            }
        }
    }

    /// Upsert economic series from BLS data
    async fn upsert_bls_economic_series(
        &self,
        pool: &DatabasePool,
        series_id: &str,
        data_source_id: Uuid,
    ) -> AppResult<EconomicSeries> {
        // For BLS, we have limited metadata, so we create a basic series
        match EconomicSeries::find_by_external_id(pool, series_id, data_source_id).await {
            Ok(existing) => Ok(existing),
            Err(_) => {
                let new_series = NewEconomicSeries {
                    source_id: data_source_id,
                    external_id: series_id.to_string(),
                    title: format!("BLS Series {}", series_id),
                    description: Some(format!(
                        "Economic data series from Bureau of Labor Statistics: {}",
                        series_id
                    )),
                    units: None,
                    frequency: "Monthly".to_string(), // Most BLS data is monthly
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

                EconomicSeries::create(pool, &new_series).await
            }
        }
    }

    /// Process FRED series
    pub async fn crawl_fred_series(&self, pool: &DatabasePool, series_id: &str) -> AppResult<()> {
        println!("Crawling FRED series: {}", series_id);

        // Get or create data source
        let data_source = self.get_or_create_fred_source(pool).await?;

        // Fetch series metadata
        let fred_series = self.fetch_fred_series_metadata(series_id).await?;

        // Upsert economic series
        let economic_series = self
            .upsert_economic_series(pool, &fred_series, data_source.id)
            .await?;

        // Fetch observations
        let observations = self.fetch_fred_observations(series_id).await?;

        // Process observations in batches
        let mut data_points = Vec::new();
        for obs in observations {
            if obs.value != "." {
                // FRED uses "." for missing values
                if let Ok(value) = obs.value.parse::<f64>() {
                    let date = NaiveDate::parse_from_str(&obs.date, "%Y-%m-%d").map_err(|e| {
                        AppError::ExternalApiError(format!(
                            "Failed to parse observation date: {}",
                            e
                        ))
                    })?;

                    let revision_date = NaiveDate::parse_from_str(&obs.realtime_end, "%Y-%m-%d")
                        .map_err(|e| {
                            AppError::ExternalApiError(format!(
                                "Failed to parse revision date: {}",
                                e
                            ))
                        })?;

                    let data_point = NewDataPoint {
                        series_id: economic_series.id,
                        date,
                        value: Some(BigDecimal::from_f64(value).ok_or_else(|| {
                            AppError::ExternalApiError(format!(
                                "Failed to convert value to BigDecimal: {}",
                                value
                            ))
                        })?),
                        revision_date,
                        is_original_release: obs.realtime_start == obs.realtime_end,
                    };

                    data_points.push(data_point);
                }
            }
        }

        // Batch insert data points
        if !data_points.is_empty() {
            DataPoint::create_batch(pool, &data_points).await?;
            println!(
                "Inserted {} data points for FRED series {}",
                data_points.len(),
                series_id
            );
        }

        Ok(())
    }

    /// Process BLS series
    pub async fn crawl_bls_series(&self, pool: &DatabasePool, series_id: &str) -> AppResult<()> {
        println!("Crawling BLS series: {}", series_id);

        // Get or create data source
        let data_source = self.get_or_create_bls_source(pool).await?;

        // Upsert economic series
        let economic_series = self
            .upsert_bls_economic_series(pool, series_id, data_source.id)
            .await?;

        // Fetch BLS data (last 10 years)
        let current_year = Utc::now().year();
        let start_year = current_year - 10;
        let bls_series_list = self
            .fetch_bls_data(&[series_id.to_string()], start_year, current_year)
            .await?;

        // Process BLS data
        for bls_series in bls_series_list {
            let mut data_points = Vec::new();

            for data_point in bls_series.data {
                if let Ok(value) = data_point.value.parse::<f64>() {
                    // Convert BLS period to date
                    let date = self.parse_bls_date(&data_point.year, &data_point.period)?;

                    let new_data_point = NewDataPoint {
                        series_id: economic_series.id,
                        date,
                        value: Some(BigDecimal::from_f64(value).ok_or_else(|| {
                            AppError::ExternalApiError(format!(
                                "Failed to convert BLS value to BigDecimal: {}",
                                value
                            ))
                        })?),
                        revision_date: date, // BLS typically doesn't revise historical data
                        is_original_release: true,
                    };

                    data_points.push(new_data_point);
                }
            }

            // Batch insert data points
            if !data_points.is_empty() {
                DataPoint::create_batch(pool, &data_points).await?;
                println!(
                    "Inserted {} data points for BLS series {}",
                    data_points.len(),
                    series_id
                );
            }
        }

        Ok(())
    }

    /// Parse BLS date from year and period
    fn parse_bls_date(&self, year: &str, period: &str) -> AppResult<NaiveDate> {
        let year_num: i32 = year
            .parse()
            .map_err(|e| AppError::ExternalApiError(format!("Failed to parse BLS year: {}", e)))?;

        // Handle different period formats
        let date = match period {
            "M01" => NaiveDate::from_ymd_opt(year_num, 1, 15),
            "M02" => NaiveDate::from_ymd_opt(year_num, 2, 15),
            "M03" => NaiveDate::from_ymd_opt(year_num, 3, 15),
            "M04" => NaiveDate::from_ymd_opt(year_num, 4, 15),
            "M05" => NaiveDate::from_ymd_opt(year_num, 5, 15),
            "M06" => NaiveDate::from_ymd_opt(year_num, 6, 15),
            "M07" => NaiveDate::from_ymd_opt(year_num, 7, 15),
            "M08" => NaiveDate::from_ymd_opt(year_num, 8, 15),
            "M09" => NaiveDate::from_ymd_opt(year_num, 9, 15),
            "M10" => NaiveDate::from_ymd_opt(year_num, 10, 15),
            "M11" => NaiveDate::from_ymd_opt(year_num, 11, 15),
            "M12" => NaiveDate::from_ymd_opt(year_num, 12, 15),
            "Q01" => NaiveDate::from_ymd_opt(year_num, 3, 31),
            "Q02" => NaiveDate::from_ymd_opt(year_num, 6, 30),
            "Q03" => NaiveDate::from_ymd_opt(year_num, 9, 30),
            "Q04" => NaiveDate::from_ymd_opt(year_num, 12, 31),
            "S01" => NaiveDate::from_ymd_opt(year_num, 6, 30), // First half
            "S02" => NaiveDate::from_ymd_opt(year_num, 12, 31), // Second half
            _ => NaiveDate::from_ymd_opt(year_num, 12, 31),    // Annual data
        };

        date.ok_or_else(|| {
            AppError::ExternalApiError(format!("Invalid BLS date: {} {}", year, period))
        })
    }

    /// Schedule FRED crawl by adding to queue
    pub async fn schedule_fred_crawl(
        &self,
        pool: &DatabasePool,
        series_id: &str,
        priority: QueuePriority,
    ) -> AppResult<()> {
        let queue_item = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: series_id.to_string(),
            priority: priority.into(),
            max_retries: 3,
            scheduled_for: None,
        };

        CrawlQueueItem::create(pool, &queue_item).await?;
        println!("Scheduled FRED crawl for series: {}", series_id);
        Ok(())
    }

    /// Schedule BLS crawl by adding to queue
    pub async fn schedule_bls_crawl(
        &self,
        pool: &DatabasePool,
        series_id: &str,
        priority: QueuePriority,
    ) -> AppResult<()> {
        let queue_item = NewCrawlQueueItem {
            source: "BLS".to_string(),
            series_id: series_id.to_string(),
            priority: priority.into(),
            max_retries: 3,
            scheduled_for: None,
        };

        CrawlQueueItem::create(pool, &queue_item).await?;
        println!("Scheduled BLS crawl for series: {}", series_id);
        Ok(())
    }

    /// Process queue items (worker function)
    pub async fn process_queue(&self, pool: &DatabasePool, worker_id: &str) -> AppResult<()> {
        loop {
            // Get next item from queue using SKIP LOCKED
            if let Some(item) = CrawlQueueItem::get_next_for_processing(pool, worker_id).await? {
                println!(
                    "Processing queue item: {} for series {}",
                    item.id, item.series_id
                );

                let result = match item.source.as_str() {
                    "FRED" => self.crawl_fred_series(pool, &item.series_id).await,
                    "BLS" => self.crawl_bls_series(pool, &item.series_id).await,
                    _ => Err(AppError::ExternalApiError(format!(
                        "Unknown source: {}",
                        item.source
                    ))),
                };

                match result {
                    Ok(_) => {
                        CrawlQueueItem::mark_completed(pool, item.id).await?;
                        println!("Successfully completed queue item: {}", item.id);
                    }
                    Err(e) => {
                        let error_msg = format!("Crawl failed: {}", e);
                        CrawlQueueItem::mark_failed(pool, item.id, error_msg).await?;
                        println!("Failed queue item {}: {}", item.id, e);
                    }
                }
            } else {
                // No items available, wait a bit
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }

    /// Trigger manual crawl (for testing/admin purposes)
    pub async fn trigger_manual_crawl(
        &self,
        pool: &DatabasePool,
        source: &str,
        series_id: &str,
    ) -> AppResult<()> {
        match source.to_uppercase().as_str() {
            "FRED" => self.crawl_fred_series(pool, series_id).await,
            "BLS" => self.crawl_bls_series(pool, series_id).await,
            _ => Err(AppError::ExternalApiError(format!(
                "Unknown source: {}",
                source
            ))),
        }
    }
}

// Module-level functions for compatibility with existing code
pub async fn trigger_manual_crawl(
    pool: &DatabasePool,
    source: &str,
    series_id: &str,
) -> AppResult<Vec<String>> {
    let crawler = CrawlerService::new(
        std::env::var("FRED_API_KEY").ok(),
        std::env::var("BLS_API_KEY").ok(),
    );

    crawler
        .trigger_manual_crawl(pool, source, series_id)
        .await?;
    Ok(vec![format!("{}:{}", source, series_id)])
}

pub async fn get_crawler_status() -> AppResult<serde_json::Value> {
    // Return basic crawler status
    Ok(serde_json::json!({
        "status": "active",
        "workers": 1,
        "last_run": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn schedule_fred_crawl(pool: &DatabasePool) -> AppResult<()> {
    let crawler = CrawlerService::new(
        std::env::var("FRED_API_KEY").ok(),
        std::env::var("BLS_API_KEY").ok(),
    );

    // Schedule some common FRED series
    let common_series = vec!["GDP", "UNRATE", "CPIAUCSL"];
    for series in common_series {
        crawler
            .schedule_fred_crawl(pool, series, QueuePriority::Normal)
            .await?;
    }
    Ok(())
}

pub async fn schedule_bls_crawl(pool: &DatabasePool) -> AppResult<()> {
    let crawler = CrawlerService::new(
        std::env::var("FRED_API_KEY").ok(),
        std::env::var("BLS_API_KEY").ok(),
    );

    // Schedule some common BLS series
    let common_series = vec!["LNS14000000", "CUUR0000SA0"];
    for series in common_series {
        crawler
            .schedule_bls_crawl(pool, series, QueuePriority::Normal)
            .await?;
    }
    Ok(())
}

pub async fn crawl_fred_series(pool: &DatabasePool, series_id: &str) -> AppResult<()> {
    let crawler = CrawlerService::new(
        std::env::var("FRED_API_KEY").ok(),
        std::env::var("BLS_API_KEY").ok(),
    );

    crawler.crawl_fred_series(pool, series_id).await
}

pub async fn crawl_bls_series(pool: &DatabasePool, series_id: &str) -> AppResult<()> {
    let crawler = CrawlerService::new(
        std::env::var("FRED_API_KEY").ok(),
        std::env::var("BLS_API_KEY").ok(),
    );

    crawler.crawl_bls_series(pool, series_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bls_date_parsing() {
        // REQUIREMENT: BLS crawler should correctly parse period codes into dates
        // PURPOSE: Verify that BLS period formats (M01, Q01, etc.) are converted to proper dates
        // This ensures data points have accurate timestamps for charting and analysis

        let crawler = CrawlerService::new(None, None);

        // Test monthly periods - required for monthly employment data
        assert_eq!(
            crawler.parse_bls_date("2023", "M01").unwrap(),
            NaiveDate::from_ymd_opt(2023, 1, 15).unwrap()
        );
        assert_eq!(
            crawler.parse_bls_date("2023", "M12").unwrap(),
            NaiveDate::from_ymd_opt(2023, 12, 15).unwrap()
        );

        // Test quarterly periods - required for quarterly GDP data
        assert_eq!(
            crawler.parse_bls_date("2023", "Q01").unwrap(),
            NaiveDate::from_ymd_opt(2023, 3, 31).unwrap()
        );
        assert_eq!(
            crawler.parse_bls_date("2023", "Q04").unwrap(),
            NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()
        );

        // Test annual data - fallback for yearly statistics
        assert_eq!(
            crawler.parse_bls_date("2023", "A01").unwrap(),
            NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()
        );
    }
}
