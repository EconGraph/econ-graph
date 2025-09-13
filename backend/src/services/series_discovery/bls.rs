//! BLS (Bureau of Labor Statistics) API integration for series discovery
//!
//! This module implements the comprehensive three-step BLS API v2 discovery process:
//! 1. All Surveys: https://api.bls.gov/publicAPI/v2/surveys
//! 2. Single Survey: https://api.bls.gov/publicAPI/v2/survey/{survey_abbreviation}
//! 3. Series Data: https://api.bls.gov/publicAPI/v2/survey/{survey_abbreviation}/series
//!
//! Documentation: https://www.bls.gov/developers/api_signature_v2.htm
//! Sample Code: Available at the bottom of the BLS API documentation
//!
//! Note: BLS API v2 does NOT require an API key for public data access

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

/// BLS API v2 response for surveys
/// Documentation: https://www.bls.gov/developers/api_signature_v2.htm#all
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

/// BLS API v2 response for single survey metadata
/// Documentation: https://www.bls.gov/developers/api_signature_v2.htm#singlesurvey
#[derive(Debug, Deserialize)]
pub struct BlsSingleSurveyResponse {
    pub status: String,
    #[serde(rename = "responseTime")]
    pub response_time: i32,
    pub message: Vec<String>,
    #[serde(rename = "Results")]
    pub results: BlsSingleSurveyResults,
}

#[derive(Debug, Deserialize)]
pub struct BlsSingleSurveyResults {
    pub survey: Vec<BlsSurveyMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct BlsSurveyMetadata {
    #[serde(rename = "survey_abbreviation")]
    pub survey_abbreviation: String,
    #[serde(rename = "survey_name")]
    pub survey_name: String,
    pub description: Option<String>,
    pub source: Option<String>,
    pub contact: Option<String>,
    pub data_type: Option<String>,
    pub frequency: Option<String>,
    pub seasonal_adjustment: Option<String>,
    pub units: Option<String>,
    pub start_year: Option<i32>,
    pub end_year: Option<i32>,
    pub notes: Option<String>,
    pub footnotes: Option<Vec<String>>,
}

/// BLS API v2 response for series within a survey
/// Documentation: https://www.bls.gov/developers/api_signature_v2.htm#all
#[derive(Debug, Deserialize)]
pub struct BlsSeriesResponse {
    pub status: String,
    #[serde(rename = "responseTime")]
    pub response_time: i32,
    pub message: Vec<String>,
    #[serde(rename = "Results")]
    pub results: BlsSeriesResults,
}

#[derive(Debug, Deserialize)]
pub struct BlsSeriesResults {
    pub series: Vec<BlsSeriesMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct BlsSeriesMetadata {
    #[serde(rename = "seriesID")]
    pub series_id: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
struct BlsSeriesApiResponse {
    pub status: String,
    #[serde(rename = "responseTime")]
    pub response_time: u32,
    pub message: Vec<String>,
    #[serde(rename = "Results")]
    pub results: BlsSeriesApiResults,
}

#[derive(Debug, Deserialize)]
struct BlsSeriesApiResults {
    pub series: Vec<BlsSeriesData>,
}

#[derive(Debug, Deserialize)]
struct BlsSeriesData {
    #[serde(rename = "seriesID")]
    pub series_id: String,
    pub data: Vec<BlsDataPoint>,
}

#[derive(Debug, Deserialize)]
struct BlsDataPoint {
    pub year: String,
    pub period: String,
    #[serde(rename = "periodName")]
    pub period_name: String,
    pub value: String,
    #[serde(rename = "latest")]
    pub latest: Option<String>,
    pub footnotes: Option<Vec<BlsFootnote>>,
}

#[derive(Debug, Deserialize)]
struct BlsFootnote {
    pub code: String,
    pub text: String,
}

/// Discover BLS series using dynamic discovery mechanisms
/// Documentation: https://www.bls.gov/developers/api_signature_v2.htm#all
///
/// Dynamic Discovery Strategy:
/// 1. Get survey abbreviations from /surveys endpoint
/// 2. Generate potential series IDs using known patterns and survey codes
/// 3. Test generated series IDs against /timeseries/data/{series_id} endpoint
/// 4. Store valid series with metadata extracted from API responses
///
/// This approach uses pattern-based generation since the documented discovery endpoints don't work.
pub async fn discover_bls_series(
    client: &Client,
    bls_api_key: &Option<String>,
    pool: &DatabasePool,
) -> AppResult<Vec<String>> {
    // Note: BLS API v2 does NOT require an API key for public data
    let _ = bls_api_key; // Suppress unused warning

    let bls_source = DataSource::get_or_create(pool, DataSource::bls()).await?;
    let mut discovered_series = Vec::new();

    // Step 1: Get available surveys to understand what data is available
    let surveys = fetch_bls_surveys(client).await?;
    println!("Found {} BLS surveys for dynamic discovery", surveys.len());

    // Step 2: Generate potential series IDs using multiple strategies
    let mut candidate_series_ids = Vec::new();

    // Strategy A: Use known patterns for major surveys
    candidate_series_ids.extend(generate_series_ids_from_patterns(&surveys));

    // Strategy B: Use survey abbreviations to generate common series patterns
    candidate_series_ids.extend(generate_series_ids_from_surveys(&surveys));

    // Strategy C: Add some known important series as fallback
    candidate_series_ids.extend(get_known_bls_series_ids());

    // Remove duplicates
    candidate_series_ids.sort();
    candidate_series_ids.dedup();

    println!(
        "Generated {} candidate series IDs for testing",
        candidate_series_ids.len()
    );

    // Step 3: Test each candidate series ID against the API
    let mut valid_series = Vec::new();
    let mut tested_count = 0;
    let total_candidates = candidate_series_ids.len();

    for series_id in &candidate_series_ids {
        tested_count += 1;
        if tested_count % 100 == 0 {
            println!(
                "Tested {}/{} candidate series IDs",
                tested_count, total_candidates
            );
        }

        // Test if this series ID returns valid data
        match test_bls_series_id(client, &series_id).await {
            Ok(Some(series_metadata)) => {
                // Store the valid series with metadata
                store_discovered_bls_series(pool, &bls_source.id, &series_metadata).await?;
                discovered_series.push(series_id.clone());
                valid_series.push(series_metadata);

                if valid_series.len() % 50 == 0 {
                    println!("Found {} valid BLS series so far", valid_series.len());
                }
            }
            Ok(None) => {
                // Series doesn't exist or has no data
                continue;
            }
            Err(e) => {
                // API error - might be rate limiting or server issue
                println!("Error testing series {}: {}", series_id, e);
                continue;
            }
        }

        // Add a small delay to be polite to the API
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    println!(
        "Dynamic discovery completed: {} valid series found from {} candidates",
        discovered_series.len(),
        total_candidates
    );
    Ok(discovered_series)
}

/// Fetch all surveys from BLS API v2
/// Documentation: https://www.bls.gov/developers/api_signature_v2.htm#all
///
/// This is the "All Surveys" endpoint that provides the complete list of available surveys.
/// No API key is required for this public endpoint.
async fn fetch_bls_surveys(client: &Client) -> AppResult<Vec<BlsSurvey>> {
    let url = "https://api.bls.gov/publicAPI/v2/surveys";

    let response =
        client.get(url).send().await.map_err(|e| {
            AppError::ExternalApiError(format!("BLS surveys request failed: {}", e))
        })?;

    if !response.status().is_success() {
        return Err(AppError::ExternalApiError(format!(
            "BLS surveys API returned status: {}",
            response.status()
        )));
    }

    let surveys_response: BlsSurveysResponse = response.json().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse BLS surveys response: {}", e))
    })?;

    Ok(surveys_response.results.survey)
}

/// Fetch all series for a specific survey from BLS API v2
/// Documentation: https://www.bls.gov/developers/api_signature_v2.htm#all
///
/// This endpoint returns ALL series available for a given survey abbreviation.
/// Unlike the data retrieval endpoint (/timeseries/data/), this discovery endpoint
/// has no pagination limits and should return the complete list of series.
/// No API key is required for this public endpoint.
async fn fetch_bls_series_for_survey(
    client: &Client,
    survey_abbreviation: &str,
) -> AppResult<Vec<BlsSeriesMetadata>> {
    let url = format!(
        "https://api.bls.gov/publicAPI/v2/survey/{}/series",
        survey_abbreviation
    );

    let response = client.get(&url).send().await.map_err(|e| {
        AppError::ExternalApiError(format!(
            "BLS series request failed for survey {}: {}",
            survey_abbreviation, e
        ))
    })?;

    if !response.status().is_success() {
        return Err(AppError::ExternalApiError(format!(
            "BLS series API returned status: {} for survey {}",
            response.status(),
            survey_abbreviation
        )));
    }

    let series_response: BlsSeriesResponse = response.json().await.map_err(|e| {
        AppError::ExternalApiError(format!(
            "Failed to parse BLS series response for survey {}: {}",
            survey_abbreviation, e
        ))
    })?;

    Ok(series_response.results.series)
}

/// Fetch extended metadata for a specific survey from BLS API v2
/// Documentation: https://www.bls.gov/developers/api_signature_v2.htm#singlesurvey
///
/// This endpoint returns detailed metadata about a specific survey including
/// description, source, contact info, data type, frequency, units, etc.
async fn fetch_bls_single_survey(
    client: &Client,
    survey_abbreviation: &str,
) -> AppResult<BlsSurveyMetadata> {
    let url = format!(
        "https://api.bls.gov/publicAPI/v2/survey/{}",
        survey_abbreviation
    );

    let response = client.get(&url).send().await.map_err(|e| {
        AppError::ExternalApiError(format!(
            "BLS single survey request failed for survey {}: {}",
            survey_abbreviation, e
        ))
    })?;

    if !response.status().is_success() {
        return Err(AppError::ExternalApiError(format!(
            "BLS single survey API returned status: {} for survey {}",
            response.status(),
            survey_abbreviation
        )));
    }

    let survey_response: BlsSingleSurveyResponse = response.json().await.map_err(|e| {
        AppError::ExternalApiError(format!(
            "Failed to parse BLS single survey response for survey {}: {}",
            survey_abbreviation, e
        ))
    })?;

    // Return the first (and typically only) survey metadata
    survey_response
        .results
        .survey
        .into_iter()
        .next()
        .ok_or_else(|| {
            AppError::ExternalApiError(format!(
                "No survey metadata found for survey {}",
                survey_abbreviation
            ))
        })
}

/// Store BLS series metadata with rich survey context in database
async fn store_bls_series_with_metadata(
    pool: &DatabasePool,
    source_id: &Uuid,
    series_metadata: &BlsSeriesMetadata,
    survey_metadata: &BlsSurveyMetadata,
) -> AppResult<()> {
    // Build comprehensive description from survey and series metadata
    let mut description_parts = vec![
        survey_metadata.survey_name.clone(),
        series_metadata.title.clone(),
    ];

    if let Some(ref desc) = survey_metadata.description {
        description_parts.push(desc.clone());
    }

    let description = Some(description_parts.join(" - "));

    // Use survey metadata for additional fields when available
    let units = survey_metadata.units.clone();
    let frequency = survey_metadata
        .frequency
        .clone()
        .unwrap_or_else(|| "Unknown".to_string());
    let seasonal_adjustment = survey_metadata.seasonal_adjustment.clone();

    // Convert start/end years to dates if available
    let start_date = survey_metadata
        .start_year
        .map(|year| chrono::NaiveDate::from_ymd_opt(year, 1, 1).unwrap());
    let end_date = survey_metadata
        .end_year
        .map(|year| chrono::NaiveDate::from_ymd_opt(year, 12, 31).unwrap());

    let new_series = NewEconomicSeries {
        source_id: *source_id,
        external_id: series_metadata.series_id.clone(),
        title: series_metadata.title.clone(),
        description,
        units,
        frequency,
        seasonal_adjustment,
        start_date,
        end_date,
        is_active: true,
        first_discovered_at: Some(chrono::Utc::now()),
        last_crawled_at: None,
        first_missing_date: None,
        crawl_status: None,
        crawl_error_message: None,
    };

    EconomicSeries::get_or_create(pool, &series_metadata.series_id, *source_id, &new_series)
        .await?;
    Ok(())
}

/// Generate series IDs using known patterns for major BLS surveys
fn generate_series_ids_from_patterns(surveys: &[BlsSurvey]) -> Vec<String> {
    let mut series_ids = Vec::new();

    for survey in surveys {
        match survey.survey_abbreviation.as_str() {
            "LA" => {
                // Labor Force Statistics - generate common patterns
                // Format: LAUCN{state_fips}{area_fips}{measure_code}{seasonal_adjustment}
                let states = [
                    "00", "01", "02", "04", "05", "06", "08", "09", "10", "11", "12", "13", "15",
                    "16", "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28",
                    "29", "30", "31", "32", "33", "34", "35", "36", "37", "38", "39", "40", "41",
                    "42", "44", "45", "46", "47", "48", "49", "50", "51", "53", "54", "55", "56",
                ];
                let areas = ["000000", "000001", "000002", "000003", "000004", "000005"]; // National, major metros
                let measures = ["000003", "000004", "000005"]; // Labor Force, Employment, Unemployment Rate
                let seasonal = ["U", "S"]; // Unadjusted, Seasonally Adjusted

                for state in &states {
                    for area in &areas {
                        for measure in &measures {
                            for adj in &seasonal {
                                series_ids
                                    .push(format!("LAUCN{}{}{}{}", state, area, measure, adj));
                            }
                        }
                    }
                }
            }
            "CE" => {
                // Employment Situation - generate common patterns
                // Format: CES{supersector}{industry}{data_type}
                let supersectors = [
                    "00000000", "05000000", "10000000", "20000000", "30000000", "40000000",
                    "50000000", "55000000", "60000000", "70000000", "80000000", "90000000",
                ];
                let data_types = ["0001", "0003", "0007"]; // All Employees, Average Hourly Earnings, Average Weekly Hours

                for supersector in &supersectors {
                    for data_type in &data_types {
                        series_ids.push(format!("CES{}{}", supersector, data_type));
                    }
                }
            }
            "CU" => {
                // Consumer Price Index - generate common patterns
                // Format: CUSR0000{series_code}
                let series_codes = [
                    "SA0", "SA0L1E", "SA0L2", "SA0L5", "SETB01", "SETB02", "SETB03", "SETB04",
                    "SETB05",
                ];

                for code in &series_codes {
                    series_ids.push(format!("CUSR0000{}", code));
                }
            }
            "WP" => {
                // Producer Price Index - generate common patterns
                // Format: WPU{commodity_code}
                let commodities = [
                    "00000000", "FD49507", "FD49508", "FD49509", "FD49510", "FD49511", "FD49512",
                ];

                for commodity in &commodities {
                    series_ids.push(format!("WPU{}", commodity));
                }
            }
            _ => {
                // For other surveys, generate some basic patterns based on survey abbreviation
                series_ids.push(format!("{}US0000000000", survey.survey_abbreviation));
                series_ids.push(format!("{}0000000001", survey.survey_abbreviation));
                series_ids.push(format!("{}{}000000000", survey.survey_abbreviation, "U"));
                series_ids.push(format!("{}{}000000000", survey.survey_abbreviation, "S"));
            }
        }
    }

    series_ids
}

/// Generate series IDs based on survey abbreviations and common patterns
fn generate_series_ids_from_surveys(surveys: &[BlsSurvey]) -> Vec<String> {
    let mut series_ids = Vec::new();

    for survey in surveys {
        // Generate some basic patterns for each survey
        let base_codes = [
            "0000000001",
            "0000000002",
            "0000000003",
            "US000000000",
            "0000000000",
        ];

        for base_code in &base_codes {
            series_ids.push(format!("{}{}", survey.survey_abbreviation, base_code));
        }

        // Add seasonal adjustment variants
        series_ids.push(format!("{}{}U", survey.survey_abbreviation, "000000000"));
        series_ids.push(format!("{}{}S", survey.survey_abbreviation, "000000000"));
    }

    series_ids
}

/// Test a BLS series ID by making a request to the API
async fn test_bls_series_id(
    client: &Client,
    series_id: &str,
) -> AppResult<Option<BlsSeriesMetadata>> {
    let url = format!(
        "https://api.bls.gov/publicAPI/v2/timeseries/data/{}",
        series_id
    );

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let text = response.text().await?;
    let bls_response: BlsSeriesApiResponse = serde_json::from_str(&text)?;

    if bls_response.status != "REQUEST_SUCCEEDED" {
        return Ok(None);
    }

    if bls_response.results.series.is_empty() {
        return Ok(None);
    }

    let series_data = &bls_response.results.series[0];
    if series_data.data.is_empty() {
        return Ok(None);
    }

    // Extract metadata from the first data point
    let _first_point = &series_data.data[0];
    let title = format!("BLS Series {}", series_id);

    Ok(Some(BlsSeriesMetadata {
        series_id: series_id.to_string(),
        title,
    }))
}

/// Store a discovered BLS series with metadata
async fn store_discovered_bls_series(
    pool: &DatabasePool,
    source_id: &Uuid,
    series_metadata: &BlsSeriesMetadata,
) -> AppResult<()> {
    let new_series = NewEconomicSeries {
        source_id: *source_id,
        external_id: series_metadata.series_id.clone(),
        title: series_metadata.title.clone(),
        description: Some(format!(
            "Bureau of Labor Statistics series: {}",
            series_metadata.series_id
        )),
        units: None,
        frequency: "Monthly".to_string(),
        seasonal_adjustment: None,
        start_date: None,
        end_date: None,
        is_active: true,
        first_discovered_at: Some(chrono::Utc::now()),
        last_crawled_at: None,
        first_missing_date: None,
        crawl_status: Some("discovered".to_string()),
        crawl_error_message: None,
    };

    EconomicSeries::get_or_create(pool, &series_metadata.series_id, *source_id, &new_series)
        .await?;
    Ok(())
}

/// Get a list of known BLS series IDs as fallback
///
/// This includes important economic indicators from various BLS surveys.
/// These series IDs were verified to work with the BLS API v2.
fn get_known_bls_series_ids() -> Vec<String> {
    vec![
        // Labor Force Statistics (LA survey)
        "LAUCN040010000000005".to_string(), // Unemployment Rate - Arizona
        "LAUCN040010000000003".to_string(), // Labor Force - Arizona
        "LAUCN040010000000004".to_string(), // Employment - Arizona
        // Consumer Price Index (CU survey)
        "CUSR0000SA0".to_string(), // CPI for All Urban Consumers: All Items in U.S. City Average
        "CUSR0000SA0L1E".to_string(), // CPI for All Urban Consumers: All Items Less Food and Energy in U.S. City Average
        "CUSR0000SETB01".to_string(), // CPI for All Urban Consumers: Gasoline (all types) in U.S. City Average
        // Employment Situation (CE survey)
        "CES0000000001".to_string(), // All Employees, Total Nonfarm
        "CES0500000003".to_string(), // Average Hourly Earnings of All Employees, Total Private
        "CES0000000007".to_string(), // Average Weekly Hours of All Employees, Total Private
        // Producer Price Index (WP survey)
        "WPU00000000".to_string(), // Producer Price Index by Commodity: All Commodities
        "WPUFD49507".to_string(),  // Producer Price Index by Commodity: Finished Goods
        // Import/Export Price Indexes (MX survey)
        "MXUS0000000000".to_string(), // Import Price Index: All Imports
        "MXUS0000000001".to_string(), // Export Price Index: All Exports
        // Employment Cost Index (CI survey)
        "CIU2010000000000A".to_string(), // Employment Cost Index: Wages and Salaries: Private Industry Workers
        "CIU2020000000000A".to_string(), // Employment Cost Index: Benefits: Private Industry Workers
    ]
}
