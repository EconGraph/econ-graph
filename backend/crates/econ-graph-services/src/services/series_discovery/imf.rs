//! IMF (International Monetary Fund) API integration for series discovery

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

/// IMF API response for datasets (SDMX format)
#[derive(Debug, Deserialize)]
pub struct ImfDatasetsResponse {
    #[serde(rename = "Structure")]
    pub structure: ImfStructure,
}

#[derive(Debug, Deserialize)]
pub struct ImfStructure {
    #[serde(rename = "Dataflows")]
    pub dataflows: ImfDataflows,
}

#[derive(Debug, Deserialize)]
pub struct ImfDataflows {
    #[serde(rename = "Dataflow")]
    pub dataflow: Vec<ImfDataflow>,
}

#[derive(Debug, Deserialize)]
pub struct ImfDataflow {
    #[serde(rename = "Name")]
    pub name: Vec<ImfName>,
    #[serde(rename = "KeyFamilyRef")]
    pub key_family_ref: ImfKeyFamilyRef,
}

#[derive(Debug, Deserialize)]
pub struct ImfName {
    #[serde(rename = "$")]
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct ImfKeyFamilyRef {
    #[serde(rename = "KeyFamilyID")]
    pub key_family_id: String,
}

/// IMF series information structure
#[derive(Debug, Clone)]
pub struct ImfSeriesInfo {
    pub series_id: String,
    pub title: String,
    pub description: Option<String>,
    pub frequency: String,
    pub units: String,
    pub dataset: String,
    pub country: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// Discover IMF series using the IMF Data API
pub async fn discover_imf_series(client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let imf_source = DataSource::get_or_create(pool, DataSource::imf()).await?;
    let mut discovered_series = Vec::new();

    // Get economic datasets from IMF API
    let datasets = fetch_imf_economic_datasets(client).await?;
    println!("Found {} IMF economic datasets", datasets.len());

    // For each dataset, discover series
    for dataset in datasets {
        println!(
            "Discovering series for dataset: {} ({})",
            dataset.name[0].value, dataset.key_family_ref.key_family_id
        );

        let dataset_series = get_known_imf_series_by_dataset(&dataset.key_family_ref.key_family_id);

        for series_info in dataset_series {
            // Store series metadata in database
            store_imf_series(pool, &imf_source.id, &series_info).await?;
            discovered_series.push(series_info.series_id);
        }
    }

    println!("Discovered {} IMF series total", discovered_series.len());
    Ok(discovered_series)
}

/// Fetch economic datasets from IMF API
async fn fetch_imf_economic_datasets(client: &Client) -> AppResult<Vec<ImfDataflow>> {
    // IMF API doesn't require authentication
    let url = "http://dataservices.imf.org/REST/SDMX_JSON.svc/Dataflow";

    let response =
        client.get(url).send().await.map_err(|e| {
            AppError::ExternalApiError(format!("IMF datasets request failed: {}", e))
        })?;

    if !response.status().is_success() {
        return Err(AppError::ExternalApiError(format!(
            "IMF API returned status: {}",
            response.status()
        )));
    }

    let datasets_response: ImfDatasetsResponse = response.json().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse IMF datasets response: {}", e))
    })?;

    // Filter for economic datasets
    let economic_datasets: Vec<ImfDataflow> = datasets_response
        .structure
        .dataflows
        .dataflow
        .into_iter()
        .filter(|dataflow| {
            let name = &dataflow.name[0].value.to_lowercase();
            let id = &dataflow.key_family_ref.key_family_id.to_lowercase();

            // Filter for key economic datasets
            name.contains("financial statistics")
                || name.contains("balance of payments")
                || name.contains("government finance")
                || name.contains("world economic outlook")
                || name.contains("direction of trade")
                || name.contains("international reserves")
                || name.contains("exchange rates")
                || id.contains("ifs")
                || id.contains("bop")
                || id.contains("gfs")
                || id.contains("weo")
                || id.contains("dot")
                || id.contains("ir")
        })
        .collect();

    Ok(economic_datasets)
}

/// Get known IMF series organized by dataset
fn get_known_imf_series_by_dataset(dataset_id: &str) -> Vec<ImfSeriesInfo> {
    match dataset_id.to_uppercase().as_str() {
        "IFS" => vec![
            ImfSeriesInfo {
                series_id: "IFS_US_PCPI_IX".to_string(),
                title: "Consumer Price Index - United States".to_string(),
                description: Some("Consumer Price Index from International Financial Statistics".to_string()),
                frequency: "Monthly".to_string(),
                units: "Index".to_string(),
                dataset: "IFS".to_string(),
                country: Some("US".to_string()),
                start_date: Some("1980-01-01".to_string()),
                end_date: None,
            },
            ImfSeriesInfo {
                series_id: "IFS_US_LP_IX".to_string(),
                title: "Labor Force Participation Rate - United States".to_string(),
                description: Some("Labor force participation rate from International Financial Statistics".to_string()),
                frequency: "Monthly".to_string(),
                units: "Percent".to_string(),
                dataset: "IFS".to_string(),
                country: Some("US".to_string()),
                start_date: Some("1980-01-01".to_string()),
                end_date: None,
            },
            ImfSeriesInfo {
                series_id: "IFS_US_EREER_IX".to_string(),
                title: "Real Effective Exchange Rate - United States".to_string(),
                description: Some("Real effective exchange rate from International Financial Statistics".to_string()),
                frequency: "Monthly".to_string(),
                units: "Index".to_string(),
                dataset: "IFS".to_string(),
                country: Some("US".to_string()),
                start_date: Some("1980-01-01".to_string()),
                end_date: None,
            },
        ],
        "BOP" => vec![
            ImfSeriesInfo {
                series_id: "BOP_US_CA_BP6_USD".to_string(),
                title: "Current Account Balance - United States".to_string(),
                description: Some("Current account balance from Balance of Payments".to_string()),
                frequency: "Quarterly".to_string(),
                units: "US Dollars".to_string(),
                dataset: "BOP".to_string(),
                country: Some("US".to_string()),
                start_date: Some("1980-01-01".to_string()),
                end_date: None,
            },
            ImfSeriesInfo {
                series_id: "BOP_US_FA_BP6_USD".to_string(),
                title: "Financial Account Balance - United States".to_string(),
                description: Some("Financial account balance from Balance of Payments".to_string()),
                frequency: "Quarterly".to_string(),
                units: "US Dollars".to_string(),
                dataset: "BOP".to_string(),
                country: Some("US".to_string()),
                start_date: Some("1980-01-01".to_string()),
                end_date: None,
            },
        ],
        "GFS" => vec![
            ImfSeriesInfo {
                series_id: "GFS_US_GGR_G01_GDP_PT".to_string(),
                title: "General Government Revenue - United States".to_string(),
                description: Some("General government revenue as percentage of GDP from Government Finance Statistics".to_string()),
                frequency: "Annual".to_string(),
                units: "Percent of GDP".to_string(),
                dataset: "GFS".to_string(),
                country: Some("US".to_string()),
                start_date: Some("1980-01-01".to_string()),
                end_date: None,
            },
            ImfSeriesInfo {
                series_id: "GFS_US_GGX_G01_GDP_PT".to_string(),
                title: "General Government Expenditure - United States".to_string(),
                description: Some("General government expenditure as percentage of GDP from Government Finance Statistics".to_string()),
                frequency: "Annual".to_string(),
                units: "Percent of GDP".to_string(),
                dataset: "GFS".to_string(),
                country: Some("US".to_string()),
                start_date: Some("1980-01-01".to_string()),
                end_date: None,
            },
        ],
        "WEO" => vec![
            ImfSeriesInfo {
                series_id: "WEO_US_NGDP_RPCH".to_string(),
                title: "Real GDP Growth - United States".to_string(),
                description: Some("Real GDP growth rate from World Economic Outlook".to_string()),
                frequency: "Annual".to_string(),
                units: "Percent".to_string(),
                dataset: "WEO".to_string(),
                country: Some("US".to_string()),
                start_date: Some("1980-01-01".to_string()),
                end_date: None,
            },
            ImfSeriesInfo {
                series_id: "WEO_US_PCPIPCH".to_string(),
                title: "Inflation Rate - United States".to_string(),
                description: Some("Inflation rate from World Economic Outlook".to_string()),
                frequency: "Annual".to_string(),
                units: "Percent".to_string(),
                dataset: "WEO".to_string(),
                country: Some("US".to_string()),
                start_date: Some("1980-01-01".to_string()),
                end_date: None,
            },
        ],
        // For other datasets, return empty vector for now
        // This can be expanded as we discover more series patterns
        _ => vec![],
    }
}

/// Store IMF series metadata in database
async fn store_imf_series(
    pool: &DatabasePool,
    source_id: &Uuid,
    series_info: &ImfSeriesInfo,
) -> AppResult<()> {
    let new_series = NewEconomicSeries {
        source_id: *source_id,
        external_id: series_info.series_id.clone(),
        title: series_info.title.clone(),
        description: series_info.description.clone(),
        units: Some(series_info.units.clone()),
        frequency: series_info.frequency.clone(),
        seasonal_adjustment: None, // IMF data varies by series
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
