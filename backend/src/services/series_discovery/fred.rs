//! FRED (Federal Reserve Economic Data) API integration for series discovery

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

/// FRED API response for series search
#[derive(Debug, Deserialize)]
pub struct FredSearchResponse {
    pub realtime_start: String,
    pub realtime_end: String,
    pub order_by: String,
    pub sort_order: String,
    pub count: i32,
    pub offset: i32,
    pub limit: i32,
    pub seriess: Vec<FredSeriesInfo>,
}

/// FRED series information structure
#[derive(Debug, Clone, Deserialize)]
pub struct FredSeriesInfo {
    pub id: String,
    pub realtime_start: String,
    pub realtime_end: String,
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
    pub group_popularity: i32,
    pub notes: Option<String>,
}

/// Discover all FRED series by searching through categories
pub async fn discover_fred_series(
    client: &Client,
    fred_api_key: &Option<String>,
    pool: &DatabasePool,
) -> AppResult<Vec<String>> {
    let api_key = fred_api_key
        .as_ref()
        .ok_or_else(|| AppError::ExternalApiError("FRED API key not configured".to_string()))?;

    let fred_source = DataSource::get_or_create(pool, DataSource::fred()).await?;
    let mut discovered_series = Vec::new();

    // Search for economic indicators
    let search_terms = vec![
        "GDP",
        "unemployment",
        "inflation",
        "interest rate",
        "employment",
        "consumer price",
        "producer price",
        "retail sales",
        "industrial production",
        "housing",
        "trade",
        "balance",
        "debt",
        "revenue",
        "expenditure",
    ];

    for term in search_terms {
        println!("Searching FRED for: {}", term);

        let url = format!(
            "https://api.stlouisfed.org/fred/series/search?search_text={}&api_key={}&file_type=json&limit=1000",
            term, api_key
        );

        let response = client.get(&url).send().await.map_err(|e| {
            AppError::ExternalApiError(format!("FRED search request failed: {}", e))
        })?;

        if !response.status().is_success() {
            println!("FRED search failed for '{}': {}", term, response.status());
            continue;
        }

        let search_response: FredSearchResponse = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse FRED search response: {}", e))
        })?;

        for series_info in search_response.seriess {
            // Store series metadata in database
            store_fred_series(pool, &fred_source.id, &series_info).await?;
            discovered_series.push(series_info.id);
        }
    }

    println!("Discovered {} FRED series total", discovered_series.len());
    Ok(discovered_series)
}

/// Store FRED series metadata in database
async fn store_fred_series(
    pool: &DatabasePool,
    source_id: &Uuid,
    series_info: &FredSeriesInfo,
) -> AppResult<()> {
    let new_series = NewEconomicSeries {
        source_id: *source_id,
        external_id: series_info.id.clone(),
        title: series_info.title.clone(),
        description: series_info.notes.clone(),
        units: Some(series_info.units.clone()),
        frequency: series_info.frequency.clone(),
        seasonal_adjustment: Some(series_info.seasonal_adjustment.clone()),
        start_date: None,
        end_date: None,
        is_active: true,
        first_discovered_at: Some(chrono::Utc::now()),
        last_crawled_at: None,
        first_missing_date: None,
        crawl_status: None,
        crawl_error_message: None,
    };

    EconomicSeries::get_or_create(pool, &series_info.id, *source_id, &new_series).await?;
    Ok(())
}

/// Search FRED series by query string
pub async fn search_fred_series(
    client: &Client,
    api_key: &Option<String>,
    query: &str,
) -> AppResult<Vec<FredSeriesInfo>> {
    let api_key = api_key
        .as_ref()
        .ok_or_else(|| AppError::ExternalApiError("FRED API key required".to_string()))?;

    let url = format!(
        "https://api.stlouisfed.org/fred/series/search?search_text={}&api_key={}&file_type=json",
        query, api_key
    );

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::ExternalApiError(format!("FRED API request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::ExternalApiError(format!(
            "FRED API returned error: {}",
            response.status()
        )));
    }

    let search_response: FredSearchResponse = response
        .json()
        .await
        .map_err(|e| AppError::ExternalApiError(format!("Failed to parse FRED response: {}", e)))?;

    Ok(search_response.seriess)
}
