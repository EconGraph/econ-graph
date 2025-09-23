//! BLS (Bureau of Labor Statistics) API integration for series discovery

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::{DataSource, EconomicSeries, NewEconomicSeries};
use econ_graph_metrics::crawler::CRAWLER_METRICS;
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

/// BLS API v2 response for surveys
#[derive(Debug, Deserialize)]
pub struct BlsSurveysResponse {
    pub status: String,
    #[serde(rename = "responseTime")]
    pub response_time: i32,
    pub message: Vec<String>,
    #[serde(rename = "Results")]
    pub results: BlsSurveysResults,
}

#[derive(Debug, Deserialize)]
pub struct BlsSurveysResults {
    pub survey: Vec<BlsSurvey>,
}

#[derive(Debug, Deserialize)]
pub struct BlsSurvey {
    #[serde(rename = "survey_abbreviation")]
    pub survey_abbreviation: String,
    #[serde(rename = "survey_name")]
    pub survey_name: String,
}

/// BLS series information structure
#[derive(Debug, Clone)]
pub struct BlsSeriesInfo {
    pub series_id: String,
    pub title: String,
    pub description: Option<String>,
    pub frequency: String,
    pub units: String,
    pub survey: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// Discover BLS series using the BLS API v2 surveys endpoint
pub async fn discover_bls_series(
    client: &Client,
    bls_api_key: &Option<String>,
    pool: &DatabasePool,
) -> AppResult<Vec<String>> {
    let api_key = bls_api_key
        .as_ref()
        .ok_or_else(|| AppError::ExternalApiError("BLS API key not configured".to_string()))?;

    let bls_source = DataSource::get_or_create(pool, DataSource::bls()).await?;
    let mut discovered_series = Vec::new();

    // Get all available surveys from BLS API v2
    let surveys = fetch_bls_surveys(client, api_key).await?;
    println!("Found {} BLS surveys", surveys.len());

    // For each survey, discover series
    for survey in surveys {
        println!(
            "Discovering series for survey: {} ({})",
            survey.survey_name, survey.survey_abbreviation
        );

        let survey_series = get_known_bls_series_by_survey(&survey.survey_abbreviation);

        for series_info in survey_series {
            // Store series metadata in database
            store_bls_series(pool, &bls_source.id, &series_info).await?;
            discovered_series.push(series_info.series_id);
        }
    }

    println!("Discovered {} BLS series total", discovered_series.len());
    Ok(discovered_series)
}

/// Fetch all surveys from BLS API v2
async fn fetch_bls_surveys(client: &Client, api_key: &str) -> AppResult<Vec<BlsSurvey>> {
    let url = "https://api.bls.gov/publicAPI/v2/surveys";

    let start = std::time::Instant::now();
    let response =
        client.get(url).send().await.map_err(|e| {
            AppError::ExternalApiError(format!("BLS surveys request failed: {}", e))
        })?;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status();
    CRAWLER_METRICS.record_request(
        "economic",
        "bls",
        "/publicAPI/v2/surveys",
        status.as_str(),
        duration,
    );

    if !status.is_success() {
        return Err(AppError::ExternalApiError(format!(
            "BLS surveys API returned status: {}",
            status
        )));
    }

    let surveys_response: BlsSurveysResponse = response.json().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse BLS surveys response: {}", e))
    })?;

    Ok(surveys_response.results.survey)
}

/// Get known BLS series organized by survey
fn get_known_bls_series_by_survey(survey_abbreviation: &str) -> Vec<BlsSeriesInfo> {
    match survey_abbreviation {
        "CU" => vec![
            BlsSeriesInfo {
                series_id: "CUUR0000SA0".to_string(),
                title: "Consumer Price Index for All Urban Consumers: All Items in U.S. City Average".to_string(),
                description: Some("Consumer Price Index for All Urban Consumers: All Items in U.S. City Average".to_string()),
                frequency: "Monthly".to_string(),
                units: "Index 1982-1984=100".to_string(),
                survey: "Consumer Price Index".to_string(),
                start_date: Some("1913-01-01".to_string()),
                end_date: None,
            },
            BlsSeriesInfo {
                series_id: "CUUR0000SA0L1E".to_string(),
                title: "Consumer Price Index for All Urban Consumers: All Items Less Food and Energy in U.S. City Average".to_string(),
                description: Some("Consumer Price Index for All Urban Consumers: All Items Less Food and Energy in U.S. City Average".to_string()),
                frequency: "Monthly".to_string(),
                units: "Index 1982-1984=100".to_string(),
                survey: "Consumer Price Index".to_string(),
                start_date: Some("1957-01-01".to_string()),
                end_date: None,
            },
        ],
        "CE" => vec![
            BlsSeriesInfo {
                series_id: "CES0000000001".to_string(),
                title: "All Employees, Total Nonfarm".to_string(),
                description: Some("All Employees, Total Nonfarm".to_string()),
                frequency: "Monthly".to_string(),
                units: "Thousands of Persons".to_string(),
                survey: "Current Employment Statistics".to_string(),
                start_date: Some("1939-01-01".to_string()),
                end_date: None,
            },
        ],
        "LA" => vec![
            BlsSeriesInfo {
                series_id: "LNS14000000".to_string(),
                title: "Unemployment Rate".to_string(),
                description: Some("Unemployment Rate".to_string()),
                frequency: "Monthly".to_string(),
                units: "Percent".to_string(),
                survey: "Local Area Unemployment Statistics".to_string(),
                start_date: Some("1948-01-01".to_string()),
                end_date: None,
            },
        ],
        // For other surveys, return empty vector for now
        // This can be expanded as we discover more series patterns
        _ => vec![],
    }
}

/// Store BLS series metadata in database
async fn store_bls_series(
    pool: &DatabasePool,
    source_id: &Uuid,
    series_info: &BlsSeriesInfo,
) -> AppResult<()> {
    let new_series = NewEconomicSeries {
        source_id: *source_id,
        external_id: series_info.series_id.clone(),
        title: series_info.title.clone(),
        description: series_info.description.clone(),
        units: Some(series_info.units.clone()),
        frequency: series_info.frequency.clone(),
        seasonal_adjustment: Some("Not Seasonally Adjusted".to_string()),
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
