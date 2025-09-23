//! BEA (Bureau of Economic Analysis) API integration for series discovery

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::{DataSource, EconomicSeries, NewEconomicSeries};
use econ_graph_metrics::crawler::CRAWLER_METRICS;
use reqwest::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

/// BEA API response for datasets
#[derive(Debug, Deserialize)]
pub struct BeaDatasetsResponse {
    #[serde(rename = "BEAAPI")]
    pub beaapi: BeaApiResponse,
}

#[derive(Debug, Deserialize)]
pub struct BeaApiResponse {
    #[serde(rename = "Results")]
    pub results: BeaResults,
}

#[derive(Debug, Deserialize)]
pub struct BeaResults {
    #[serde(rename = "Dataset")]
    pub dataset: Vec<BeaDataset>,
}

#[derive(Debug, Deserialize)]
pub struct BeaDataset {
    #[serde(rename = "DatasetName")]
    pub dataset_name: String,
    #[serde(rename = "DatasetDescription")]
    pub dataset_description: String,
}

/// BEA series information structure
#[derive(Debug, Clone)]
pub struct BeaSeriesInfo {
    pub series_id: String,
    pub title: String,
    pub description: Option<String>,
    pub frequency: String,
    pub units: String,
    pub dataset: String,
    pub table_name: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// Discover BEA series using the BEA Data API
pub async fn discover_bea_series(
    client: &Client,
    bea_api_key: &Option<String>,
    pool: &DatabasePool,
) -> AppResult<Vec<String>> {
    let api_key = bea_api_key
        .as_ref()
        .ok_or_else(|| AppError::ExternalApiError("BEA API key not configured".to_string()))?;

    let bea_source = DataSource::get_or_create(pool, DataSource::bea()).await?;
    let mut discovered_series = Vec::new();

    // Get all available datasets from BEA API
    let datasets = fetch_bea_datasets(client, api_key).await?;
    println!("Found {} BEA datasets", datasets.len());

    // For each dataset, discover series
    for dataset in datasets {
        println!(
            "Discovering series for dataset: {} ({})",
            dataset.dataset_description, dataset.dataset_name
        );

        let dataset_series = get_known_bea_series_by_dataset(&dataset.dataset_name);

        for series_info in dataset_series {
            // Store series metadata in database
            store_bea_series(pool, &bea_source.id, &series_info).await?;
            discovered_series.push(series_info.series_id);
        }
    }

    println!("Discovered {} BEA series total", discovered_series.len());
    Ok(discovered_series)
}

/// Fetch all datasets from BEA Data API
async fn fetch_bea_datasets(client: &Client, api_key: &str) -> AppResult<Vec<BeaDataset>> {
    let url = format!(
        "https://apps.bea.gov/api/data/?&UserID={}&method=GetDatasetList&ResultFormat=JSON",
        api_key
    );

    let start = std::time::Instant::now();
    let response =
        client.get(&url).send().await.map_err(|e| {
            AppError::ExternalApiError(format!("BEA datasets request failed: {}", e))
        })?;
    let duration = start.elapsed().as_secs_f64();
    let status = response.status();
    CRAWLER_METRICS.record_request(
        "economic",
        "bea",
        "/api/data/GetDatasetList",
        status.as_str(),
        duration,
    );
    if status == StatusCode::TOO_MANY_REQUESTS {
        CRAWLER_METRICS.record_rate_limit_hit("economic", "bea");
    }

    if !status.is_success() {
        return Err(AppError::ExternalApiError(format!(
            "BEA datasets API returned status: {}",
            status
        )));
    }

    let datasets_response: BeaDatasetsResponse = response.json().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse BEA datasets response: {}", e))
    })?;

    Ok(datasets_response.beaapi.results.dataset)
}

/// Get known BEA series organized by dataset
fn get_known_bea_series_by_dataset(dataset_name: &str) -> Vec<BeaSeriesInfo> {
    match dataset_name {
        "NIPA" => vec![
            BeaSeriesInfo {
                series_id: "NIPA_GDP_TOTAL".to_string(),
                title: "Gross Domestic Product".to_string(),
                description: Some(
                    "Total GDP from National Income and Product Accounts".to_string(),
                ),
                frequency: "Quarterly".to_string(),
                units: "Billions of Dollars".to_string(),
                dataset: "NIPA".to_string(),
                table_name: "T10101".to_string(),
                start_date: Some("1947-01-01".to_string()),
                end_date: None,
            },
            BeaSeriesInfo {
                series_id: "NIPA_GDP_PC".to_string(),
                title: "Gross Domestic Product Per Capita".to_string(),
                description: Some(
                    "GDP per capita from National Income and Product Accounts".to_string(),
                ),
                frequency: "Quarterly".to_string(),
                units: "Dollars".to_string(),
                dataset: "NIPA".to_string(),
                table_name: "T10101".to_string(),
                start_date: Some("1947-01-01".to_string()),
                end_date: None,
            },
            BeaSeriesInfo {
                series_id: "NIPA_PCE_TOTAL".to_string(),
                title: "Personal Consumption Expenditures".to_string(),
                description: Some("Total personal consumption expenditures".to_string()),
                frequency: "Quarterly".to_string(),
                units: "Billions of Dollars".to_string(),
                dataset: "NIPA".to_string(),
                table_name: "T20100".to_string(),
                start_date: Some("1947-01-01".to_string()),
                end_date: None,
            },
        ],
        "FixedAssets" => vec![BeaSeriesInfo {
            series_id: "FA_NET_STOCK_TOTAL".to_string(),
            title: "Net Stock of Fixed Assets".to_string(),
            description: Some("Total net stock of fixed assets".to_string()),
            frequency: "Annual".to_string(),
            units: "Billions of Dollars".to_string(),
            dataset: "FixedAssets".to_string(),
            table_name: "T10101".to_string(),
            start_date: Some("1925-01-01".to_string()),
            end_date: None,
        }],
        "ITA" => vec![
            BeaSeriesInfo {
                series_id: "ITA_EXPORTS_TOTAL".to_string(),
                title: "Total Exports of Goods and Services".to_string(),
                description: Some(
                    "Total exports from International Transactions Accounts".to_string(),
                ),
                frequency: "Quarterly".to_string(),
                units: "Billions of Dollars".to_string(),
                dataset: "ITA".to_string(),
                table_name: "T10101".to_string(),
                start_date: Some("1999-01-01".to_string()),
                end_date: None,
            },
            BeaSeriesInfo {
                series_id: "ITA_IMPORTS_TOTAL".to_string(),
                title: "Total Imports of Goods and Services".to_string(),
                description: Some(
                    "Total imports from International Transactions Accounts".to_string(),
                ),
                frequency: "Quarterly".to_string(),
                units: "Billions of Dollars".to_string(),
                dataset: "ITA".to_string(),
                table_name: "T10101".to_string(),
                start_date: Some("1999-01-01".to_string()),
                end_date: None,
            },
        ],
        "RegionalData" => vec![BeaSeriesInfo {
            series_id: "REG_GDP_TOTAL".to_string(),
            title: "Gross Domestic Product by State".to_string(),
            description: Some("Total GDP by state".to_string()),
            frequency: "Annual".to_string(),
            units: "Millions of Dollars".to_string(),
            dataset: "RegionalData".to_string(),
            table_name: "T10101".to_string(),
            start_date: Some("1997-01-01".to_string()),
            end_date: None,
        }],
        // For other datasets, return empty vector for now
        // This can be expanded as we discover more series patterns
        _ => vec![],
    }
}

/// Store BEA series metadata in database
async fn store_bea_series(
    pool: &DatabasePool,
    source_id: &Uuid,
    series_info: &BeaSeriesInfo,
) -> AppResult<()> {
    let new_series = NewEconomicSeries {
        source_id: *source_id,
        external_id: series_info.series_id.clone(),
        title: series_info.title.clone(),
        description: series_info.description.clone(),
        units: Some(series_info.units.clone()),
        frequency: series_info.frequency.clone(),
        seasonal_adjustment: None, // BEA data varies by series
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
