//! Census Bureau API integration for economic data discovery
//!
//! This module provides integration with the U.S. Census Bureau Data API
//! for discovering and accessing economic time series data.

#[cfg(test)]
mod integration_tests;

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::models::data_source::DataSource;
use crate::models::economic_series::{EconomicSeries, NewEconomicSeries, SeriesFrequency};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Census API query builder for constructing API requests
#[derive(Debug, Clone)]
pub struct CensusQueryBuilder {
    variables: Vec<String>,
    for_geography: Option<String>,
    in_geography: Option<String>,
    year_start: Option<i32>,
    year_end: Option<i32>,
}

impl CensusQueryBuilder {
    pub fn new() -> Self {
        Self {
            variables: Vec::new(),
            for_geography: None,
            in_geography: None,
            year_start: None,
            year_end: None,
        }
    }

    pub fn variables(mut self, variables: &[String]) -> Self {
        self.variables = variables.to_vec();
        self
    }

    pub fn for_geography(mut self, geography: &str) -> Self {
        self.for_geography = Some(geography.to_string());
        self
    }

    pub fn in_geography(mut self, geography: &str) -> Self {
        self.in_geography = Some(geography.to_string());
        self
    }

    pub fn year_range(mut self, start: i32, end: i32) -> Self {
        self.year_start = Some(start);
        self.year_end = Some(end);
        self
    }

    pub fn build_url(&self) -> AppResult<String> {
        if self.variables.is_empty() {
            return Err(AppError::ValidationError(
                "At least one variable must be specified".to_string(),
            ));
        }

        let mut url = "https://api.census.gov/data/timeseries/bds".to_string();

        // Build get parameter
        let get_vars = self.variables.join(",");
        url.push_str(&format!("?get={}", get_vars));

        // Add geography parameters
        if let Some(for_geo) = &self.for_geography {
            url.push_str(&format!("&for={}", for_geo));
        }

        if let Some(in_geo) = &self.in_geography {
            url.push_str(&format!("&in={}", in_geo));
        }

        // Add year range
        if let (Some(start), Some(end)) = (self.year_start, self.year_end) {
            url.push_str(&format!("&YEAR={}", start));
            for year in (start + 1)..=end {
                url.push_str(&format!(",{}", year));
            }
        }

        Ok(url)
    }
}

impl Default for CensusQueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// BDS (Business Dynamics Statistics) variable information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BdsVariable {
    pub name: String,
    pub label: String,
    pub predicate_type: Option<String>,
    pub group: Option<String>,
    pub limit: Option<i32>,
    pub attributes: Option<HashMap<String, String>>,
}

/// BDS geography information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BdsGeography {
    pub name: String,
    pub geo_level_display: Option<String>,
    pub geo_level_id: Option<String>,
    pub requires: Option<HashMap<String, String>>,
    pub wildcard: Option<bool>,
}

/// BDS data point from Census API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BdsDataPoint {
    pub variable: String,
    pub year: i32,
    pub value: Option<i64>,
    pub geography: String,
}

/// Execute a Census API query and return raw response
pub async fn execute_query(client: &Client, query: &CensusQueryBuilder) -> AppResult<String> {
    let url = query.build_url()?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::ExternalApiError(format!("Census API request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::ExternalApiError(format!(
            "Census API returned error: {} - {}",
            response.status(),
            response
                .status()
                .canonical_reason()
                .unwrap_or("Unknown error")
        )));
    }

    // Handle 204 No Content responses (known API limitation for multi-year queries)
    if response.status() == 204 {
        return Err(AppError::ExternalApiError(
            "Census API returned 204 No Content - no data available for the requested parameters"
                .to_string(),
        ));
    }

    let text = response.text().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to read Census response: {}", e))
    })?;

    Ok(text)
}

/// Execute a Census API query and return parsed structured data
pub async fn execute_structured(
    client: &Client,
    query: &CensusQueryBuilder,
) -> AppResult<Vec<BdsDataPoint>> {
    let raw_data = execute_query(client, query).await?;
    parse_census_response(&raw_data)
}

/// Parse Census API response into structured data points
pub fn parse_census_response(response: &str) -> AppResult<Vec<BdsDataPoint>> {
    let raw_data: Vec<Vec<String>> = serde_json::from_str(response).map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse Census response: {}", e))
    })?;

    if raw_data.len() < 2 {
        return Err(AppError::ExternalApiError(
            "Invalid Census response: expected at least header and data rows".to_string(),
        ));
    }

    let headers = &raw_data[0];
    let data_rows = &raw_data[1..];

    // Find indices of key columns - handle Census API quirks
    let year_idx = headers
        .iter()
        .position(|h| h.to_lowercase() == "year")
        .ok_or_else(|| {
            AppError::ExternalApiError("YEAR column not found in response".to_string())
        })?;

    // Census API returns geography as the last column with numeric codes
    let geo_idx = headers.len() - 1;

    let mut data_points = Vec::new();

    for row in data_rows {
        if row.len() != headers.len() {
            continue; // Skip malformed rows
        }

        // Parse year
        let year: i32 = row[year_idx].parse().unwrap_or(0);
        if year == 0 {
            continue; // Skip invalid years
        }

        // Parse geography (last column)
        let geography = row[geo_idx].clone();

        // Parse each variable (excluding YEAR and geography columns)
        for (i, header) in headers.iter().enumerate() {
            if header.to_lowercase() == "year" || i == geo_idx {
                continue; // Skip YEAR and geography columns
            }

            let value = if row[i].is_empty() {
                None
            } else {
                row[i].parse().ok()
            };

            data_points.push(BdsDataPoint {
                variable: header.clone(),
                year,
                value,
                geography: geography.clone(),
            });
        }
    }

    Ok(data_points)
}

/// Fetch sample BDS data for testing and validation
pub async fn fetch_bds_sample_data(client: &Client) -> AppResult<Vec<BdsDataPoint>> {
    let variables = vec![
        "ESTAB".to_string(), // Establishments
        "FIRM".to_string(),  // Firms
        "YEAR".to_string(),  // Year (required)
    ];

    fetch_bds_data(client, &variables, "us", 2020, 2022, &None).await
}

/// Fetch BDS data with specified parameters
pub async fn fetch_bds_data(
    client: &Client,
    variables: &[String],
    geography: &str,
    year_start: i32,
    year_end: i32,
    _census_api_key: &Option<String>,
) -> AppResult<Vec<BdsDataPoint>> {
    let query = CensusQueryBuilder::new()
        .variables(variables)
        .for_geography(geography)
        .year_range(year_start, year_end);

    execute_structured(client, &query).await
}

/// Discover Census series using BDS dataset
pub async fn discover_census_series(pool: &DatabasePool) -> AppResult<Vec<EconomicSeries>> {
    println!("🔍 Starting Census BDS series discovery...");

    let client = Client::new();
    let census_source = DataSource::get_or_create(pool, DataSource::census()).await?;

    // Fetch BDS variables and geography
    let variables =
        fetch_bds_variables(&client, "https://api.census.gov/data/timeseries/bds").await?;
    let geography =
        fetch_bds_geography(&client, "https://api.census.gov/data/timeseries/bds").await?;

    println!(
        "📊 Found {} variables and {} geography levels",
        variables.len(),
        geography.len()
    );

    // Filter for economic indicators
    let economic_variables = filter_economic_indicators(&variables);
    println!("🔍 Filtering economic indicators...");
    let economic_variables = filter_economic_indicators(&variables);

    println!(
        "📈 Creating series for {} economic indicators across {} geographic levels",
        economic_variables.len(),
        geography.len()
    );

    let mut discovered_series = Vec::new();

    // Create series for each economic variable and geography combination
    for variable in &economic_variables {
        for geo in &geography {
            let external_id = format!("CENSUS_BDS_{}_{}", variable.name, geo.name);
            let title = format!(
                "{} - {}",
                variable.label,
                geo.geo_level_display.as_ref().unwrap_or(&geo.name)
            );
            let description = format!(
                "Business Dynamics Statistics: {} for {}",
                variable.label,
                geo.geo_level_display.as_ref().unwrap_or(&geo.name)
            );

            let new_series = NewEconomicSeries {
                source_id: census_source.id,
                external_id: external_id.clone(),
                title: title.clone(),
                description: Some(description),
                units: Some("Count".to_string()),
                frequency: SeriesFrequency::Annual.to_string(),
                seasonal_adjustment: None,
                start_date: Some(chrono::NaiveDate::from_ymd_opt(1978, 1, 1).unwrap()),
                end_date: Some(chrono::NaiveDate::from_ymd_opt(2022, 12, 31).unwrap()),
                is_active: true,
                first_discovered_at: Some(chrono::Utc::now()),
                last_crawled_at: None,
                first_missing_date: None,
                crawl_status: None,
                crawl_error_message: None,
            };

            let series = EconomicSeries::create(pool, &new_series).await?;
            discovered_series.push(series);

            if discovered_series.len() % 10 == 0 {
                println!("📝 Created {} series so far...", discovered_series.len());
            }
        }
    }

    println!(
        "✅ Discovered {} Census series total",
        discovered_series.len()
    );
    Ok(discovered_series)
}

/// Filter BDS variables to economic indicators
fn filter_economic_indicators(variables: &[BdsVariable]) -> Vec<BdsVariable> {
    let economic_keywords = [
        "estab",
        "firm",
        "job",
        "emp",
        "creation",
        "destruction",
        "net",
        "reallocation",
        "birth",
        "death",
        "entry",
        "exit",
        "rate",
        "employment",
        "establishment",
    ];

    variables
        .iter()
        .filter(|var| {
            let name_lower = var.name.to_lowercase();
            let label_lower = var.label.to_lowercase();

            // Skip geographic and time variables
            if name_lower.contains("for")
                || name_lower.contains("in")
                || name_lower.contains("year")
                || name_lower.contains("time")
                || name_lower.contains("geo")
                || name_lower.contains("state")
                || name_lower.contains("county")
                || name_lower.contains("metro")
                || name_lower.contains("cbsa")
                || name_lower.contains("nation")
            {
                return false;
            }

            // Skip variables with no meaningful data
            if name_lower.is_empty() || label_lower.is_empty() {
                return false;
            }

            // Check if it's an economic indicator
            economic_keywords
                .iter()
                .any(|keyword| name_lower.contains(keyword) || label_lower.contains(keyword))
        })
        .cloned()
        .collect()
}

/// Fetch BDS variables from Census API
async fn fetch_bds_variables(client: &Client, base_url: &str) -> AppResult<Vec<BdsVariable>> {
    let url = format!("{}/variables.json", base_url);

    let response =
        client.get(&url).send().await.map_err(|e| {
            AppError::ExternalApiError(format!("BDS variables request failed: {}", e))
        })?;

    if !response.status().is_success() {
        return Err(AppError::ExternalApiError(format!(
            "BDS variables request failed with status: {}",
            response.status()
        )));
    }

    let text = response.text().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to read BDS variables response: {}", e))
    })?;

    // Parse the response - it's a JSON object with a "variables" key
    let response: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse BDS variables response: {}", e))
    })?;

    let variables_obj = response.get("variables").ok_or_else(|| {
        AppError::ExternalApiError("No 'variables' key in BDS response".to_string())
    })?;

    let mut variables = Vec::new();
    if let Some(obj) = variables_obj.as_object() {
        for (name, var_data) in obj {
            if let Some(var_obj) = var_data.as_object() {
                let variable = BdsVariable {
                    name: name.clone(),
                    label: var_obj
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    predicate_type: var_obj
                        .get("predicateType")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    group: var_obj
                        .get("group")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    limit: var_obj
                        .get("limit")
                        .and_then(|v| v.as_i64())
                        .map(|i| i as i32),
                    attributes: None, // We can expand this later if needed
                };
                variables.push(variable);
            }
        }
    }

    Ok(variables)
}

/// Fetch BDS geography from Census API
async fn fetch_bds_geography(client: &Client, base_url: &str) -> AppResult<Vec<BdsGeography>> {
    let url = format!("{}/geography.json", base_url);

    let response =
        client.get(&url).send().await.map_err(|e| {
            AppError::ExternalApiError(format!("BDS geography request failed: {}", e))
        })?;

    if !response.status().is_success() {
        return Err(AppError::ExternalApiError(format!(
            "BDS geography request failed with status: {}",
            response.status()
        )));
    }

    let text = response.text().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to read BDS geography response: {}", e))
    })?;

    // Parse the response - it's a JSON object with a "fips" key
    let response: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse BDS geography response: {}", e))
    })?;

    let fips_array = response.get("fips").ok_or_else(|| {
        AppError::ExternalApiError("No 'fips' key in BDS geography response".to_string())
    })?;

    let mut geography = Vec::new();
    if let Some(array) = fips_array.as_array() {
        for geo_data in array {
            if let Some(geo_obj) = geo_data.as_object() {
                let geo = BdsGeography {
                    name: geo_obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    geo_level_display: geo_obj
                        .get("geoLevelDisplay")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    geo_level_id: geo_obj
                        .get("geoLevelDisplay")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    requires: None, // We can expand this later if needed
                    wildcard: geo_obj.get("wildcard").and_then(|v| v.as_bool()),
                };
                geography.push(geo);
            }
        }
    }

    Ok(geography)
}
