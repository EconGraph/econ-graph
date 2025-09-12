//! World Bank API integration for series discovery

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

/// World Bank API response for indicators
#[derive(Debug, Deserialize)]
pub struct WorldBankIndicatorsResponse {
    pub indicator: Vec<WorldBankIndicator>,
}

#[derive(Debug, Deserialize)]
pub struct WorldBankIndicator {
    pub id: String,
    pub name: String,
    pub source: WorldBankSource,
    #[serde(rename = "sourceNote")]
    pub source_note: Option<String>,
    #[serde(rename = "sourceOrganization")]
    pub source_organization: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WorldBankSource {
    pub id: String,
    pub value: String,
}

/// World Bank series information structure
#[derive(Debug, Clone)]
pub struct WorldBankSeriesInfo {
    pub series_id: String,
    pub title: String,
    pub description: Option<String>,
    pub frequency: String,
    pub units: String,
    pub source: String,
    pub country: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// Discover World Bank series using the World Bank Indicators API
pub async fn discover_world_bank_series(
    client: &Client,
    pool: &DatabasePool,
) -> AppResult<Vec<String>> {
    let world_bank_source = DataSource::get_or_create(pool, DataSource::world_bank()).await?;
    let mut discovered_series = Vec::new();

    // Get economic indicators from World Bank API
    let indicators = fetch_world_bank_economic_indicators(client).await?;
    println!("Found {} World Bank economic indicators", indicators.len());

    // Convert indicators to series and store in database
    for indicator in indicators {
        let series_info = WorldBankSeriesInfo {
            series_id: indicator.id.clone(),
            title: indicator.name.clone(),
            description: indicator.source_note.clone(),
            frequency: "Annual".to_string(), // Most World Bank data is annual
            units: "Various".to_string(),    // World Bank uses various units
            source: indicator.source.value.clone(),
            country: None,                              // Global indicators
            start_date: Some("1960-01-01".to_string()), // World Bank data typically starts around 1960
            end_date: None,
        };

        // Store series metadata in database
        store_world_bank_series(pool, &world_bank_source.id, &series_info).await?;
        discovered_series.push(series_info.series_id);
    }

    println!(
        "Discovered {} World Bank series total",
        discovered_series.len()
    );
    Ok(discovered_series)
}

/// Fetch economic indicators from World Bank API
async fn fetch_world_bank_economic_indicators(
    client: &Client,
) -> AppResult<Vec<WorldBankIndicator>> {
    // World Bank API doesn't require authentication
    let url = "https://api.worldbank.org/v2/indicator?format=json&per_page=1000";

    let response = client.get(url).send().await.map_err(|e| {
        AppError::ExternalApiError(format!("World Bank indicators request failed: {}", e))
    })?;

    if !response.status().is_success() {
        return Err(AppError::ExternalApiError(format!(
            "World Bank API returned status: {}",
            response.status()
        )));
    }

    // World Bank API returns an array with metadata and data
    let json_response: serde_json::Value = response.json().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse World Bank response: {}", e))
    })?;

    // Extract indicators from the response
    let indicators = if let Some(array) = json_response.as_array() {
        if array.len() >= 2 {
            // Second element contains the actual data
            serde_json::from_value::<WorldBankIndicatorsResponse>(array[1].clone())
                .map_err(|e| {
                    AppError::ExternalApiError(format!(
                        "Failed to parse World Bank indicators: {}",
                        e
                    ))
                })?
                .indicator
        } else {
            return Err(AppError::ExternalApiError(
                "Invalid World Bank API response format".to_string(),
            ));
        }
    } else {
        return Err(AppError::ExternalApiError(
            "World Bank API response is not an array".to_string(),
        ));
    };

    // Filter for economic indicators (focus on key economic metrics)
    let economic_indicators: Vec<WorldBankIndicator> = indicators
        .into_iter()
        .filter(|indicator| {
            let name_lower = indicator.name.to_lowercase();
            let id_lower = indicator.id.to_lowercase();

            // Filter for key economic indicators
            name_lower.contains("gdp")
                || name_lower.contains("gross domestic product")
                || name_lower.contains("inflation")
                || name_lower.contains("unemployment")
                || name_lower.contains("interest rate")
                || name_lower.contains("exchange rate")
                || name_lower.contains("trade")
                || name_lower.contains("debt")
                || name_lower.contains("revenue")
                || name_lower.contains("expenditure")
                || name_lower.contains("current account")
                || name_lower.contains("balance of payments")
                || id_lower.contains("ny.gdp")
                || id_lower.contains("fp.cpi")
                || id_lower.contains("sl.uem")
                || id_lower.contains("fr.inr")
                || id_lower.contains("ne.trd")
                || id_lower.contains("gc.rev")
                || id_lower.contains("gc.xpn")
        })
        .collect();

    Ok(economic_indicators)
}

/// Store World Bank series metadata in database
async fn store_world_bank_series(
    pool: &DatabasePool,
    source_id: &Uuid,
    series_info: &WorldBankSeriesInfo,
) -> AppResult<()> {
    let new_series = NewEconomicSeries {
        source_id: *source_id,
        external_id: series_info.series_id.clone(),
        title: series_info.title.clone(),
        description: series_info.description.clone(),
        units: Some(series_info.units.clone()),
        frequency: series_info.frequency.clone(),
        seasonal_adjustment: None, // World Bank data varies by series
        start_date: series_info
            .start_date
            .as_ref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
        end_date: series_info
            .end_date
            .as_ref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
        is_active: true,
        first_discovered_at: Some(chrono::Utc::now()),
        last_crawled_at: None,
        first_missing_date: None,
        crawl_status: None,
        crawl_error_message: None,
    };

    EconomicSeries::get_or_create(pool, &series_info.series_id, *source_id, &new_series).await?;
    Ok(())
}
