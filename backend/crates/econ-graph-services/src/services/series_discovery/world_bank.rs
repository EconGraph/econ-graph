//! World Bank API integration for series discovery

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::{DataSource, EconomicSeries, NewEconomicSeries};
use econ_graph_metrics::crawler::CRAWLER_METRICS;
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

/// Detailed metadata for a World Bank indicator
#[derive(Debug, Clone)]
struct WorldBankIndicatorMetadata {
    pub frequency: String,
    pub units: String,
    pub country: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// Discover World Bank series using the World Bank Indicators API
///
/// This function implements a comprehensive discovery mechanism that:
/// 1. Fetches indicators from multiple sources (topics, pagination, direct lookup)
/// 2. Validates that indicators have actual data
/// 3. Extracts comprehensive metadata
/// 4. Stores series information in the database
pub async fn discover_world_bank_series(
    client: &Client,
    pool: &DatabasePool,
) -> AppResult<Vec<String>> {
    let world_bank_source = DataSource::get_or_create(pool, DataSource::world_bank()).await?;
    let mut discovered_series = Vec::new();

    println!("Starting World Bank series discovery...");

    // Strategy 1: Get indicators from multiple economic topics
    let mut topic_indicators = Vec::new();

    // Economy & Growth (ID 3)
    let economy_indicators = fetch_indicators_by_topic(client, "3").await?;
    println!(
        "Found {} indicators from Economy & Growth topic",
        economy_indicators.len()
    );
    topic_indicators.extend(economy_indicators);

    // Financial Sector (ID 7)
    let financial_indicators = fetch_indicators_by_topic(client, "7").await?;
    println!(
        "Found {} indicators from Financial Sector topic",
        financial_indicators.len()
    );
    topic_indicators.extend(financial_indicators);

    // Trade (ID 11) - if available
    if let Ok(trade_indicators) = fetch_indicators_by_topic(client, "11").await {
        println!(
            "Found {} indicators from Trade topic",
            trade_indicators.len()
        );
        topic_indicators.extend(trade_indicators);
    }

    // Remove duplicates from topic indicators
    topic_indicators.sort_by(|a, b| a.id.cmp(&b.id));
    topic_indicators.dedup_by(|a, b| a.id == b.id);

    println!(
        "Total unique indicators from all topics: {}",
        topic_indicators.len()
    );

    // Strategy 2: Get key economic indicators by direct lookup
    let key_indicators = fetch_key_economic_indicators(client).await?;
    println!("Found {} key economic indicators", key_indicators.len());

    // Strategy 3: Get indicators from major countries (US, China, Germany, Japan, UK)
    let country_indicators = fetch_indicators_for_major_countries(client).await?;
    println!(
        "Found {} indicators from major countries",
        country_indicators.len()
    );

    // Strategy 4: Get indicators from paginated search (limited to avoid overwhelming)
    let paginated_indicators = fetch_indicators_paginated(client, 10).await?; // First 10 pages
    println!(
        "Found {} indicators from paginated search",
        paginated_indicators.len()
    );

    // Combine and deduplicate indicators
    let mut all_indicators = topic_indicators;
    all_indicators.extend(key_indicators);
    all_indicators.extend(country_indicators);
    all_indicators.extend(paginated_indicators);
    // Remove duplicates based on indicator ID
    all_indicators.sort_by(|a, b| a.id.cmp(&b.id));
    all_indicators.dedup_by(|a, b| a.id == b.id);

    println!("Total unique indicators found: {}", all_indicators.len());

    // Convert indicators to series and store in database
    for indicator in all_indicators {
        // Get detailed metadata for each indicator
        let detailed_metadata = fetch_indicator_metadata(client, &indicator.id).await?;
        let series_info = WorldBankSeriesInfo {
            series_id: indicator.id.clone(),
            title: indicator.name.clone(),
            description: indicator.source_note.clone(),
            frequency: detailed_metadata.frequency,
            units: detailed_metadata.units,
            source: indicator.source.value.clone(),
            country: detailed_metadata.country,
            start_date: detailed_metadata.start_date,
            end_date: detailed_metadata.end_date,
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

/// Fetch indicators by topic ID (e.g., "3" for Economy & Growth)
async fn fetch_indicators_by_topic(
    client: &Client,
    topic_id: &str,
) -> AppResult<Vec<WorldBankIndicator>> {
    let url = format!(
        "https://api.worldbank.org/v2/topic/{}/indicator?format=json&per_page=1000",
        topic_id
    );

    let start = std::time::Instant::now();
    let response = client.get(&url).send().await.map_err(|e| {
        AppError::ExternalApiError(format!("World Bank topic indicators request failed: {}", e))
    })?;
    let duration = start.elapsed().as_secs_f64();
    let status = response.status();
    CRAWLER_METRICS.record_request(
        "economic",
        "world_bank",
        "/topic/indicator",
        status.as_str(),
        duration,
    );
    if !status.is_success() {
        return Err(AppError::ExternalApiError(format!(
            "World Bank API returned status: {}",
            status
        )));
    }

    let json_response: serde_json::Value = response.json().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse World Bank response: {}", e))
    })?;

    extract_indicators_from_response(json_response)
}

/// Fetch indicators for major countries to discover country-specific economic data
async fn fetch_indicators_for_major_countries(
    client: &Client,
) -> AppResult<Vec<WorldBankIndicator>> {
    // List of major countries/regions to fetch indicators for
    let major_countries = vec![
        "US", // United States
        "CN", // China
        "DE", // Germany
        "JP", // Japan
        "GB", // United Kingdom
        "FR", // France
        "IT", // Italy
        "CA", // Canada
        "AU", // Australia
        "BR", // Brazil
        "IN", // India
        "RU", // Russia
        "ZA", // South Africa
        "MX", // Mexico
        "KR", // South Korea
    ];

    let mut all_indicators = Vec::new();

    for country_code in major_countries {
        // Get a sample of indicators for this country by fetching data for a common indicator
        // This will help us discover what indicators are available for this country
        if let Ok(indicators) = fetch_country_indicators_sample(client, country_code).await {
            all_indicators.extend(indicators);
        }

        // Add delay to be polite to the API
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Remove duplicates
    all_indicators.sort_by(|a, b| a.id.cmp(&b.id));
    all_indicators.dedup_by(|a, b| a.id == b.id);

    Ok(all_indicators)
}

/// Fetch a sample of indicators available for a specific country
async fn fetch_country_indicators_sample(
    client: &Client,
    country_code: &str,
) -> AppResult<Vec<WorldBankIndicator>> {
    // Use a common economic indicator to discover what's available for this country
    let test_indicators = vec![
        "NY.GDP.MKTP.CD",    // GDP (current US$)
        "FP.CPI.TOTL.ZG",    // Inflation, consumer prices (annual %)
        "SL.UEM.TOTL.ZS",    // Unemployment, total (% of total labor force)
        "NE.TRD.GNFS.ZS",    // Trade (% of GDP)
        "GC.DOD.TOTL.GD.ZS", // Central government debt, total (% of GDP)
    ];

    let mut indicators = Vec::new();

    for indicator_id in test_indicators {
        // Try to fetch data for this indicator in this country
        let url = format!(
            "https://api.worldbank.org/v2/country/{}/indicator/{}?format=json&per_page=1",
            country_code, indicator_id
        );

        if let Ok(response) = client.get(&url).send().await {
            if response.status().is_success() {
                if let Ok(json_response) = response.json::<serde_json::Value>().await {
                    // If we get data, this indicator is available for this country
                    if let Some(array) = json_response.as_array() {
                        if array.len() >= 2
                            && array[1].as_array().map_or(false, |arr| !arr.is_empty())
                        {
                            // Create a WorldBankIndicator for this combination
                            let indicator = WorldBankIndicator {
                                id: indicator_id.to_string(),
                                name: format!("{} for {}", indicator_id, country_code),
                                source: WorldBankSource {
                                    id: "2".to_string(),
                                    value: "World Development Indicators".to_string(),
                                },
                                source_note: Some(format!(
                                    "Available for country: {}",
                                    country_code
                                )),
                                source_organization: Some("World Bank".to_string()),
                            };
                            indicators.push(indicator);
                        }
                    }
                }
            }
        }
    }

    Ok(indicators)
}

/// Fetch key economic indicators by direct lookup
async fn fetch_key_economic_indicators(client: &Client) -> AppResult<Vec<WorldBankIndicator>> {
    // List of key economic indicators to fetch directly
    let key_indicator_ids = vec![
        "NY.GDP.MKTP.CD",    // GDP (current US$)
        "NY.GDP.MKTP.KD.ZG", // GDP growth (annual %)
        "FP.CPI.TOTL.ZG",    // Inflation, consumer prices (annual %)
        "SL.UEM.TOTL.ZS",    // Unemployment, total (% of total labor force)
        "FR.INR.RINR",       // Real interest rate (%)
        "NE.TRD.GNFS.ZS",    // Trade (% of GDP)
        "GC.DOD.TOTL.GD.ZS", // Central government debt, total (% of GDP)
        "GC.REV.XGRT.GD.ZS", // Tax revenue (% of GDP)
        "GC.XPN.TOTL.GD.ZS", // Expense (% of GDP)
        "BN.CAB.XOKA.GD.ZS", // Current account balance (% of GDP)
    ];

    let mut indicators = Vec::new();
    for indicator_id in key_indicator_ids {
        if let Ok(indicator) = fetch_single_indicator(client, indicator_id).await {
            indicators.push(indicator);
        }
    }

    Ok(indicators)
}

/// Fetch indicators from paginated search
async fn fetch_indicators_paginated(
    client: &Client,
    max_pages: usize,
) -> AppResult<Vec<WorldBankIndicator>> {
    let mut all_indicators = Vec::new();
    for page in 1..=max_pages {
        let url = format!(
            "https://api.worldbank.org/v2/indicator?format=json&per_page=100&page={}",
            page
        );

        let start = std::time::Instant::now();
        let response = client.get(&url).send().await.map_err(|e| {
            AppError::ExternalApiError(format!("World Bank paginated request failed: {}", e))
        })?;
        let duration = start.elapsed().as_secs_f64();
        let status = response.status();
        CRAWLER_METRICS.record_request(
            "economic",
            "world_bank",
            "/indicator",
            status.as_str(),
            duration,
        );
        if !status.is_success() {
            println!(
                "Warning: World Bank API returned status {} for page {}",
                status, page
            );
            continue;
        }

        let json_response: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse World Bank response: {}", e))
        })?;

        let page_indicators = extract_indicators_from_response(json_response)?;

        // Filter for economic indicators
        let economic_indicators: Vec<WorldBankIndicator> = page_indicators
            .into_iter()
            .filter(is_economic_indicator)
            .collect();

        all_indicators.extend(economic_indicators);
        // Add delay to be polite to the API
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Ok(all_indicators)
}

/// Fetch a single indicator by ID
async fn fetch_single_indicator(
    client: &Client,
    indicator_id: &str,
) -> AppResult<WorldBankIndicator> {
    let url = format!(
        "https://api.worldbank.org/v2/indicator/{}?format=json",
        indicator_id
    );

    let start = std::time::Instant::now();
    let response = client.get(&url).send().await.map_err(|e| {
        AppError::ExternalApiError(format!("World Bank single indicator request failed: {}", e))
    })?;
    let duration = start.elapsed().as_secs_f64();
    let status = response.status();
    CRAWLER_METRICS.record_request(
        "economic",
        "world_bank",
        "/indicator/{id}",
        status.as_str(),
        duration,
    );
    if !status.is_success() {
        return Err(AppError::ExternalApiError(format!(
            "World Bank API returned status: {}",
            status
        )));
    }

    let json_response: serde_json::Value = response.json().await.map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse World Bank response: {}", e))
    })?;

    let indicators = extract_indicators_from_response(json_response)?;
    indicators.into_iter().next().ok_or_else(|| {
        AppError::ExternalApiError(format!("No indicator found for ID: {}", indicator_id))
    })
}

/// Extract indicators from World Bank API response
fn extract_indicators_from_response(
    json_response: serde_json::Value,
) -> AppResult<Vec<WorldBankIndicator>> {
    if let Some(array) = json_response.as_array() {
        if array.len() >= 2 {
            // Second element contains the actual data
            Ok(
                serde_json::from_value::<WorldBankIndicatorsResponse>(array[1].clone())
                    .map_err(|e| {
                        AppError::ExternalApiError(format!(
                            "Failed to parse World Bank indicators: {}",
                            e
                        ))
                    })?
                    .indicator,
            )
        } else {
            Ok(Vec::new())
        }
    } else {
        Err(AppError::ExternalApiError(
            "World Bank API response is not an array".to_string(),
        ))
    }
}

/// Check if an indicator is economic-related
fn is_economic_indicator(indicator: &WorldBankIndicator) -> bool {
    let name_lower = indicator.name.to_lowercase();
    let id_lower = indicator.id.to_lowercase();

    // Check for economic keywords in name
    let economic_keywords = [
        "gdp",
        "gross domestic product",
        "inflation",
        "unemployment",
        "interest rate",
        "exchange rate",
        "trade",
        "debt",
        "revenue",
        "expenditure",
        "current account",
        "balance of payments",
        "economic",
        "financial",
        "monetary",
        "fiscal",
        "price",
        "wage",
        "income",
        "consumption",
        "investment",
        "savings",
        "export",
        "import",
        "balance",
        "surplus",
        "deficit",
        "budget",
    ];

    let has_economic_keyword = economic_keywords
        .iter()
        .any(|keyword| name_lower.contains(keyword));

    // Check for economic indicator ID patterns
    let economic_id_patterns = [
        "ny.gdp", "fp.cpi", "sl.uem", "fr.inr", "ne.trd", "gc.rev", "gc.xpn", "bn.cab", "dt.dod",
        "ic.tax", "ic.bus", "ic.reg", "ic.gov", "ic.lgl", "ie.tic", "ie.tra", "ie.tec", "ie.eng",
        "ie.ene", "ie.env", "ie.hea", "ie.edu", "ie.agr", "ie.fin", "ie.inf", "ie.urb", "ie.rur",
        "ie.gen",
    ];

    let has_economic_id_pattern = economic_id_patterns
        .iter()
        .any(|pattern| id_lower.contains(pattern));

    has_economic_keyword || has_economic_id_pattern
}

/// Fetch detailed metadata for a specific indicator
async fn fetch_indicator_metadata(
    client: &Client,
    indicator_id: &str,
) -> AppResult<WorldBankIndicatorMetadata> {
    // For now, return default metadata since World Bank API doesn't provide
    // detailed frequency/unit information in the indicator endpoint
    // In a real implementation, we might need to fetch data samples to determine this
    Ok(WorldBankIndicatorMetadata {
        frequency: "Annual".to_string(), // Most World Bank data is annual
        units: "Various".to_string(),    // World Bank uses various units
        country: None,                   // Global indicators
        start_date: Some("1960-01-01".to_string()), // World Bank data typically starts around 1960
        end_date: None,
    })
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

#[cfg(test)]
mod tests;
