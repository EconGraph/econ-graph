//! Enhanced crawler service with crawl attempts tracking and data source visibility controls

use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel::ExpressionMethods;
use diesel_async::RunQueryDsl;
// Removed futures_util import as we're using .await? instead of .map_err()
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::{CrawlAttempt, DataPoint, NewCrawlAttempt, NewDataPoint};

/// Enhanced crawler service with comprehensive tracking
pub struct EnhancedCrawlerService {
    client: Client,
    fred_api_key: Option<String>,
    bls_api_key: Option<String>,
}

impl EnhancedCrawlerService {
    pub fn new(fred_api_key: Option<String>, bls_api_key: Option<String>) -> Self {
        Self {
            client: Client::new(),
            fred_api_key,
            bls_api_key,
        }
    }

    /// Crawl a series with comprehensive tracking
    pub async fn crawl_series_with_tracking(
        &self,
        pool: &DatabasePool,
        series_id: &Uuid,
        external_id: &str,
        source_name: &str,
    ) -> AppResult<CrawlResult> {
        let start_time = Utc::now();

        // Create crawl attempt record
        let new_attempt = NewCrawlAttempt {
            series_id: *series_id,
            attempted_at: Some(start_time),
            crawl_method: "api".to_string(),
            crawl_url: Some(self.build_crawl_url(source_name, external_id)),
            data_found: Some(false),
            new_data_points: Some(0),
            success: Some(false),
            retry_count: Some(0),
            ..Default::default()
        };

        let attempt = CrawlAttempt::create(pool, &new_attempt).await?;

        // Perform the actual crawl
        let crawl_result = match source_name {
            "FRED" => self.crawl_fred_series(pool, series_id, external_id).await,
            "BLS" => self.crawl_bls_series(pool, series_id, external_id).await,
            _ => Err(AppError::ExternalApiError(format!(
                "Unsupported source: {}",
                source_name
            ))),
        };

        let end_time = Utc::now();
        let response_time_ms = (end_time - start_time).num_milliseconds() as i32;

        // Update crawl attempt with results
        let _updated_attempt = match &crawl_result {
            Ok(result) => {
                CrawlAttempt::update_completion(
                    pool,
                    &attempt.id,
                    true, // success
                    result.data_found,
                    result.new_data_points,
                    result.latest_data_date,
                    result.data_freshness_hours,
                    None, // error_type
                    None, // error_message
                    Some(response_time_ms),
                    Some(result.data_size_bytes),
                )
                .await?
            }
            Err(e) => {
                CrawlAttempt::update_completion(
                    pool,
                    &attempt.id,
                    false, // success
                    false, // data_found
                    0,     // new_data_points
                    None,  // latest_data_date
                    None,  // data_freshness_hours
                    Some("crawl_error".to_string()),
                    Some(e.to_string()),
                    Some(response_time_ms),
                    None, // data_size_bytes
                )
                .await?
            }
        };

        // Update series last_crawled_at
        self.update_series_crawl_status(pool, series_id, &crawl_result)
            .await?;

        crawl_result
    }

    /// Build crawl URL based on source and series ID
    fn build_crawl_url(&self, source_name: &str, external_id: &str) -> String {
        match source_name {
            "FRED" => {
                if let Some(api_key) = &self.fred_api_key {
                    format!("https://api.stlouisfed.org/fred/series/observations?series_id={}&api_key={}&file_type=json", external_id, api_key)
                } else {
                    format!("https://api.stlouisfed.org/fred/series/observations?series_id={}&file_type=json", external_id)
                }
            }
            "BLS" => {
                format!(
                    "https://api.bls.gov/publicAPI/v2/timeseries/data/{}",
                    external_id
                )
            }
            _ => format!("unknown://{}/{}", source_name, external_id),
        }
    }

    /// Crawl FRED series with tracking
    async fn crawl_fred_series(
        &self,
        pool: &DatabasePool,
        series_id: &Uuid,
        external_id: &str,
    ) -> AppResult<CrawlResult> {
        let api_key = self
            .fred_api_key
            .as_ref()
            .ok_or_else(|| AppError::ExternalApiError("FRED API key not configured".to_string()))?;

        let url = format!(
            "https://api.stlouisfed.org/fred/series/observations?series_id={}&api_key={}&file_type=json&limit=1000",
            external_id, api_key
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::ExternalApiError(format!("FRED request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::ExternalApiError(format!(
                "FRED API returned status: {}",
                response.status()
            )));
        }

        let data_size_bytes = response.content_length().unwrap_or(0) as i32;
        let fred_response: FredObservationsResponse = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse FRED response: {}", e))
        })?;

        let mut new_data_points = 0;
        let mut latest_data_date = None;
        let mut data_freshness_hours = None;

        for observation in &fred_response.observations {
            if observation.value != "." && !observation.value.is_empty() {
                let date =
                    NaiveDate::parse_from_str(&observation.date, "%Y-%m-%d").map_err(|e| {
                        AppError::ExternalApiError(format!("Invalid date format: {}", e))
                    })?;

                let value =
                    BigDecimal::parse_bytes(observation.value.as_bytes(), 10).ok_or_else(|| {
                        AppError::ExternalApiError("Invalid value format".to_string())
                    })?;

                // Check if this data point already exists
                let existing_point =
                    DataPoint::get_by_series_and_date(pool, *series_id, &date).await?;

                if existing_point.is_none() {
                    let new_point = NewDataPoint {
                        series_id: *series_id,
                        date,
                        value: Some(value),
                        revision_date: Utc::now().date_naive(),
                        is_original_release: true,
                    };

                    DataPoint::create(pool, &new_point).await?;
                    new_data_points += 1;
                }

                // Track latest data date
                if latest_data_date.is_none() || date > latest_data_date.unwrap() {
                    latest_data_date = Some(date);

                    // Calculate data freshness (hours since publication)
                    let now = Utc::now().date_naive();
                    if date <= now {
                        let days_diff = (now - date).num_days();
                        data_freshness_hours = Some((days_diff * 24) as i32);
                    }
                }
            }
        }

        Ok(CrawlResult {
            data_found: new_data_points > 0 || !fred_response.observations.is_empty(),
            new_data_points,
            latest_data_date,
            data_freshness_hours,
            data_size_bytes,
        })
    }

    /// Crawl BLS series with tracking
    async fn crawl_bls_series(
        &self,
        _pool: &DatabasePool,
        _series_id: &Uuid,
        _external_id: &str,
    ) -> AppResult<CrawlResult> {
        // BLS crawling implementation would go here
        // For now, return a placeholder result
        Ok(CrawlResult {
            data_found: false,
            new_data_points: 0,
            latest_data_date: None,
            data_freshness_hours: None,
            data_size_bytes: 0,
        })
    }

    /// Update series crawl status after crawling
    async fn update_series_crawl_status(
        &self,
        pool: &DatabasePool,
        series_id: &Uuid,
        crawl_result: &AppResult<CrawlResult>,
    ) -> AppResult<()> {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let (crawl_status, crawl_error_message) = match crawl_result {
            Ok(result) => {
                if result.data_found {
                    ("success".to_string(), None)
                } else {
                    ("no_data".to_string(), Some("No new data found".to_string()))
                }
            }
            Err(e) => ("failed".to_string(), Some(e.to_string())),
        };

        diesel::update(
            econ_graph_core::schema::economic_series::table
                .filter(econ_graph_core::schema::economic_series::id.eq(*series_id)),
        )
        .set((
            econ_graph_core::schema::economic_series::last_crawled_at.eq(Some(Utc::now())),
            econ_graph_core::schema::economic_series::crawl_status.eq(Some(crawl_status)),
            econ_graph_core::schema::economic_series::crawl_error_message.eq(crawl_error_message),
            econ_graph_core::schema::economic_series::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    /// Get crawlable series based on visibility controls and crawl frequency
    pub async fn get_crawlable_series(
        &self,
        pool: &DatabasePool,
    ) -> AppResult<Vec<CrawlableSeries>> {
        let _conn = pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // For now, return an empty vector to avoid the complex SQL query
        // TODO: Implement proper query using Diesel's query builder
        let crawlable_series = Vec::new();

        Ok(crawlable_series)
    }

    /// Get crawl statistics for a series
    pub async fn get_series_crawl_statistics(
        &self,
        pool: &DatabasePool,
        series_id: &Uuid,
    ) -> AppResult<CrawlStatistics> {
        CrawlAttempt::get_crawl_statistics(pool, series_id).await
    }
}

/// Result of a crawl operation
#[derive(Debug, Clone)]
pub struct CrawlResult {
    pub data_found: bool,
    pub new_data_points: i32,
    pub latest_data_date: Option<NaiveDate>,
    pub data_freshness_hours: Option<i32>,
    pub data_size_bytes: i32,
}

/// Series that can be crawled
#[derive(Debug, QueryableByName)]
#[diesel(table_name = econ_graph_core::schema::economic_series)]
pub struct CrawlableSeries {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub series_id: Uuid,
    #[diesel(sql_type = diesel::sql_types::Varchar)]
    pub external_id: String,
    #[diesel(sql_type = diesel::sql_types::Varchar)]
    pub title: String,
    #[diesel(sql_type = diesel::sql_types::Varchar)]
    pub source_name: String,
    #[diesel(sql_type = diesel::sql_types::Int4)]
    pub crawl_frequency_hours: i32,
    #[diesel(sql_type = diesel::sql_types::Timestamptz)]
    pub last_crawled_at: Option<DateTime<Utc>>,
    #[diesel(sql_type = diesel::sql_types::Varchar)]
    pub crawl_status: Option<String>,
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

// Re-export for compatibility
pub use econ_graph_core::models::crawl_attempt::CrawlStatistics;
