//! Census Bureau API integration for series discovery

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

/// Census API response for datasets
#[derive(Debug, Deserialize)]
pub struct CensusDatasetsResponse {
    pub dataset: Vec<CensusDataset>,
}

#[derive(Debug, Deserialize)]
pub struct CensusDataset {
    pub dataset_name: String,
    pub title: String,
    pub description: String,
    pub vintage: String,
    pub is_timeseries: bool,
    pub temporal: Option<String>,
}

/// Census series information structure
#[derive(Debug, Clone)]
pub struct CensusSeriesInfo {
    pub series_id: String,
    pub title: String,
    pub description: Option<String>,
    pub frequency: String,
    pub units: String,
    pub dataset: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// Discover Census series using the Census Data API
pub async fn discover_census_series(
    client: &Client,
    census_api_key: &Option<String>,
    pool: &DatabasePool,
) -> AppResult<Vec<String>> {
    let census_source = DataSource::get_or_create(pool, DataSource::census()).await?;
    let mut discovered_series = Vec::new();

    // Get economic datasets from Census API
    let datasets = fetch_census_economic_datasets(client, census_api_key).await?;
    println!("Found {} Census economic datasets", datasets.len());

    // For each dataset, discover series
    for dataset in datasets {
        println!(
            "Discovering series for dataset: {} ({})",
            dataset.title, dataset.dataset_name
        );

        let dataset_series = get_known_census_series_by_dataset(&dataset.dataset_name);

        for series_info in dataset_series {
            // Store series metadata in database
            store_census_series(pool, &census_source.id, &series_info).await?;
            discovered_series.push(series_info.series_id);
        }
    }

    println!("Discovered {} Census series total", discovered_series.len());
    Ok(discovered_series)
}

/// Fetch economic datasets from Census Data API
async fn fetch_census_economic_datasets(
    client: &Client,
    census_api_key: &Option<String>,
) -> AppResult<Vec<CensusDataset>> {
    let base_url = "https://api.census.gov/data";

    // Census API doesn't require authentication for most endpoints
    let url = format!("{}/timeseries", base_url);

    let response = client.get(&url).send().await.map_err(|e| {
        AppError::ExternalApiError(format!("Census datasets request failed: {}", e))
    })?;

    if !response.status().is_success() {
        return Err(AppError::ExternalApiError(format!(
            "Census API returned status: {}",
            response.status()
        )));
    }

    let datasets_response: CensusDatasetsResponse = response.json().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse Census datasets response: {}", e))
    })?;

    // Filter for economic datasets
    let economic_datasets: Vec<CensusDataset> = datasets_response
        .dataset
        .into_iter()
        .filter(|dataset| {
            let name_lower = dataset.dataset_name.to_lowercase();
            let title_lower = dataset.title.to_lowercase();

            // Filter for key economic datasets
            name_lower.contains("economic")
                || name_lower.contains("business")
                || name_lower.contains("trade")
                || name_lower.contains("retail")
                || name_lower.contains("manufacturing")
                || title_lower.contains("economic")
                || title_lower.contains("business")
                || title_lower.contains("trade")
                || title_lower.contains("retail")
                || title_lower.contains("manufacturing")
        })
        .collect();

    Ok(economic_datasets)
}

/// Get known Census series organized by dataset
fn get_known_census_series_by_dataset(dataset_name: &str) -> Vec<CensusSeriesInfo> {
    match dataset_name {
        "timeseries/economic" => vec![
            CensusSeriesInfo {
                series_id: "CENSUS_ECON_RETAIL_SALES".to_string(),
                title: "Retail Sales".to_string(),
                description: Some("Monthly retail sales data from Census Bureau".to_string()),
                frequency: "Monthly".to_string(),
                units: "Millions of Dollars".to_string(),
                dataset: "timeseries/economic".to_string(),
                start_date: Some("1992-01-01".to_string()),
                end_date: None,
            },
            CensusSeriesInfo {
                series_id: "CENSUS_ECON_MANUFACTURING".to_string(),
                title: "Manufacturing Shipments".to_string(),
                description: Some(
                    "Monthly manufacturing shipments data from Census Bureau".to_string(),
                ),
                frequency: "Monthly".to_string(),
                units: "Millions of Dollars".to_string(),
                dataset: "timeseries/economic".to_string(),
                start_date: Some("1992-01-01".to_string()),
                end_date: None,
            },
        ],
        "timeseries/business" => vec![CensusSeriesInfo {
            series_id: "CENSUS_BUS_NEW_ORDERS".to_string(),
            title: "New Orders for Durable Goods".to_string(),
            description: Some("New orders for durable goods from Census Bureau".to_string()),
            frequency: "Monthly".to_string(),
            units: "Millions of Dollars".to_string(),
            dataset: "timeseries/business".to_string(),
            start_date: Some("1992-01-01".to_string()),
            end_date: None,
        }],
        // For other datasets, return empty vector for now
        // This can be expanded as we discover more series patterns
        _ => vec![],
    }
}

/// Store Census series metadata in database
async fn store_census_series(
    pool: &DatabasePool,
    source_id: &Uuid,
    series_info: &CensusSeriesInfo,
) -> AppResult<()> {
    let new_series = NewEconomicSeries {
        source_id: *source_id,
        external_id: series_info.series_id.clone(),
        title: series_info.title.clone(),
        description: series_info.description.clone(),
        units: Some(series_info.units.clone()),
        frequency: series_info.frequency.clone(),
        seasonal_adjustment: None, // Census data varies by series
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
