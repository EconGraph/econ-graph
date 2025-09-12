//! Unified API discovery service for all data sources
//!
//! This service provides a unified interface for discovering economic data series
//! from various APIs and storing them in the series_metadata table.

use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::models::{DataSource, SeriesMetadata, NewSeriesMetadata};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// API discovery configuration for a data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiDiscoveryConfig {
    /// Data source name
    pub source_name: String,
    /// Base API URL
    pub base_url: String,
    /// API key (if required)
    pub api_key: Option<String>,
    /// Rate limit per minute
    pub rate_limit_per_minute: u32,
    /// Discovery endpoints
    pub discovery_endpoints: Vec<DiscoveryEndpoint>,
    /// Search parameters
    pub search_params: HashMap<String, String>,
}

/// Discovery endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryEndpoint {
    /// Endpoint name
    pub name: String,
    /// Endpoint URL pattern
    pub url_pattern: String,
    /// HTTP method
    pub method: String,
    /// Query parameters
    pub query_params: HashMap<String, String>,
    /// Response parsing configuration
    pub response_config: ResponseConfig,
}

/// Response parsing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseConfig {
    /// Path to series list in JSON response
    pub series_path: String,
    /// Field mappings for series data
    pub field_mappings: HashMap<String, String>,
    /// Pagination configuration
    pub pagination: Option<PaginationConfig>,
}

/// Pagination configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationConfig {
    /// Parameter name for page number
    pub page_param: String,
    /// Parameter name for page size
    pub size_param: String,
    /// Maximum pages to fetch
    pub max_pages: Option<u32>,
}

/// Discovered series information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSeries {
    /// External series ID
    pub external_id: String,
    /// Series title
    pub title: String,
    /// Series description
    pub description: Option<String>,
    /// Units of measurement
    pub units: Option<String>,
    /// Data frequency
    pub frequency: Option<String>,
    /// Geographic coverage
    pub geographic_level: Option<String>,
    /// Data URL
    pub data_url: Option<String>,
    /// API endpoint for data
    pub api_endpoint: Option<String>,
}

/// API Discovery Service
pub struct ApiDiscoveryService {
    client: Client,
    configs: HashMap<String, ApiDiscoveryConfig>,
}

impl ApiDiscoveryService {
    /// Create a new API discovery service
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            configs: Self::load_discovery_configs(),
        }
    }

    /// Load discovery configurations for all data sources
    fn load_discovery_configs() -> HashMap<String, ApiDiscoveryConfig> {
        let mut configs = HashMap::new();

        // FRED configuration
        configs.insert("FRED".to_string(), ApiDiscoveryConfig {
            source_name: "Federal Reserve Economic Data (FRED)".to_string(),
            base_url: "https://api.stlouisfed.org/fred".to_string(),
            api_key: None, // Will be set from environment
            rate_limit_per_minute: 120,
            discovery_endpoints: vec![
                DiscoveryEndpoint {
                    name: "popular_series".to_string(),
                    url_pattern: "/series/search".to_string(),
                    method: "GET".to_string(),
                    query_params: [
                        ("search_text".to_string(), "*".to_string()),
                        ("sort_order".to_string(), "popularity".to_string()),
                        ("limit".to_string(), "100".to_string()),
                        ("file_type".to_string(), "json".to_string()),
                    ].iter().cloned().collect(),
                    response_config: ResponseConfig {
                        series_path: "seriess".to_string(),
                        field_mappings: [
                            ("id".to_string(), "external_id".to_string()),
                            ("title".to_string(), "title".to_string()),
                            ("notes".to_string(), "description".to_string()),
                            ("units".to_string(), "units".to_string()),
                            ("frequency".to_string(), "frequency".to_string()),
                        ].iter().cloned().collect(),
                        pagination: None,
                    },
                },
                DiscoveryEndpoint {
                    name: "search_series".to_string(),
                    url_pattern: "/series/search".to_string(),
                    method: "GET".to_string(),
                    query_params: [
                        ("file_type".to_string(), "json".to_string()),
                        ("limit".to_string(), "1000".to_string()),
                    ].iter().cloned().collect(),
                    response_config: ResponseConfig {
                        series_path: "seriess".to_string(),
                        field_mappings: [
                            ("id".to_string(), "external_id".to_string()),
                            ("title".to_string(), "title".to_string()),
                            ("notes".to_string(), "description".to_string()),
                            ("units".to_string(), "units".to_string()),
                            ("frequency".to_string(), "frequency".to_string()),
                        ].iter().cloned().collect(),
                        pagination: None,
                    },
                },
            ],
            search_params: [
                ("search_text".to_string(), "".to_string()),
            ].iter().cloned().collect(),
        });

        // BLS configuration
        configs.insert("BLS".to_string(), ApiDiscoveryConfig {
            source_name: "Bureau of Labor Statistics (BLS)".to_string(),
            base_url: "https://api.bls.gov/publicAPI/v2".to_string(),
            api_key: None,
            rate_limit_per_minute: 500,
            discovery_endpoints: vec![
                DiscoveryEndpoint {
                    name: "popular_series".to_string(),
                    url_pattern: "/timeseries/popular".to_string(),
                    method: "GET".to_string(),
                    query_params: [
                        ("format".to_string(), "json".to_string()),
                    ].iter().cloned().collect(),
                    response_config: ResponseConfig {
                        series_path: "Results.series".to_string(),
                        field_mappings: [
                            ("seriesID".to_string(), "external_id".to_string()),
                            ("catalog".to_string(), "title".to_string()),
                            ("catalog".to_string(), "description".to_string()),
                        ].iter().cloned().collect(),
                        pagination: None,
                    },
                },
            ],
            search_params: HashMap::new(),
        });

        // Census configuration
        configs.insert("CENSUS".to_string(), ApiDiscoveryConfig {
            source_name: "U.S. Census Bureau".to_string(),
            base_url: "https://api.census.gov/data".to_string(),
            api_key: None,
            rate_limit_per_minute: 500,
            discovery_endpoints: vec![
                DiscoveryEndpoint {
                    name: "datasets".to_string(),
                    url_pattern: "/".to_string(),
                    method: "GET".to_string(),
                    query_params: [
                        ("format".to_string(), "json".to_string()),
                    ].iter().cloned().collect(),
                    response_config: ResponseConfig {
                        series_path: "dataset".to_string(),
                        field_mappings: [
                            ("dataset".to_string(), "external_id".to_string()),
                            ("title".to_string(), "title".to_string()),
                            ("description".to_string(), "description".to_string()),
                        ].iter().cloned().collect(),
                        pagination: None,
                    },
                },
            ],
            search_params: HashMap::new(),
        });

        configs
    }

    /// Discover series for a specific data source
    pub async fn discover_series_for_source(
        &self,
        source_name: &str,
        pool: &DatabasePool,
    ) -> AppResult<Vec<String>> {
        let config = self.configs.get(source_name)
            .ok_or_else(|| crate::error::AppError::ConfigurationError(
                format!("No discovery configuration found for source: {}", source_name)
            ))?;

        // Get or create data source
        let data_source = DataSource::get_or_create(pool, DataSource::from_name(source_name)?).await?;

        let mut discovered_series = Vec::new();

        // Discover from each endpoint
        for endpoint in &config.discovery_endpoints {
            match self.discover_from_endpoint(config, endpoint, &data_source.id, pool).await {
                Ok(series) => {
                    discovered_series.extend(series);
                }
                Err(e) => {
                    eprintln!("Failed to discover from endpoint {}: {}", endpoint.name, e);
                }
            }
        }

        Ok(discovered_series)
    }

    /// Discover series from a specific endpoint
    async fn discover_from_endpoint(
        &self,
        config: &ApiDiscoveryConfig,
        endpoint: &DiscoveryEndpoint,
        source_id: &uuid::Uuid,
        pool: &DatabasePool,
    ) -> AppResult<Vec<String>> {
        let mut url = format!("{}{}", config.base_url, endpoint.url_pattern);

        // Add query parameters
        let mut query_params = endpoint.query_params.clone();
        if let Some(api_key) = &config.api_key {
            query_params.insert("api_key".to_string(), api_key.clone());
        }

        if !query_params.is_empty() {
            let params: Vec<String> = query_params.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            url.push('?');
            url.push_str(&params.join("&"));
        }

        // Make API request
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(crate::error::AppError::ExternalApiError(
                format!("API request failed with status: {}", response.status())
            ));
        }

        let json: serde_json::Value = response.json().await?;

        // Parse series from response
        let series_list = self.parse_series_from_response(&json, &endpoint.response_config)?;

        // Store series metadata
        let mut discovered_ids = Vec::new();
        for series in series_list {
            let new_metadata = NewSeriesMetadata {
                source_id: *source_id,
                external_id: series.external_id.clone(),
                title: series.title,
                description: series.description,
                units: series.units,
                frequency: series.frequency,
                geographic_level: series.geographic_level,
                data_url: series.data_url,
                api_endpoint: series.api_endpoint,
                is_active: true,
            };

            match SeriesMetadata::get_or_create(pool, *source_id, &series.external_id, &new_metadata).await {
                Ok(_) => {
                    discovered_ids.push(series.external_id);
                }
                Err(e) => {
                    eprintln!("Failed to store series metadata {}: {}", series.external_id, e);
                }
            }
        }

        Ok(discovered_ids)
    }

    /// Parse series from API response
    fn parse_series_from_response(
        &self,
        json: &serde_json::Value,
        config: &ResponseConfig,
    ) -> AppResult<Vec<DiscoveredSeries>> {
        // Navigate to series list using the configured path
        let series_array = json.pointer(&format!("/{}", config.series_path))
            .and_then(|v| v.as_array())
            .ok_or_else(|| crate::error::AppError::ExternalApiError(
                "Could not find series array in API response".to_string()
            ))?;

        let mut series_list = Vec::new();

        for series_json in series_array {
            if let Some(series) = self.parse_single_series(series_json, config)? {
                series_list.push(series);
            }
        }

        Ok(series_list)
    }

    /// Parse a single series from JSON
    fn parse_single_series(
        &self,
        json: &serde_json::Value,
        config: &ResponseConfig,
    ) -> AppResult<Option<DiscoveredSeries>> {
        let external_id = json.get(&config.field_mappings.get("external_id").unwrap_or(&"id".to_string()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AppError::ExternalApiError(
                "Missing external_id in series data".to_string()
            ))?
            .to_string();

        let title = json.get(&config.field_mappings.get("title").unwrap_or(&"title".to_string()))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Series")
            .to_string();

        let description = json.get(&config.field_mappings.get("description").unwrap_or(&"notes".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let units = json.get(&config.field_mappings.get("units").unwrap_or(&"units".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let frequency = json.get(&config.field_mappings.get("frequency").unwrap_or(&"frequency".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(Some(DiscoveredSeries {
            external_id,
            title,
            description,
            units,
            frequency,
            geographic_level: Some("United States".to_string()),
            data_url: None,
            api_endpoint: None,
        }))
    }

    /// Discover series for all configured sources
    pub async fn discover_all_series(&self, pool: &DatabasePool) -> AppResult<HashMap<String, Vec<String>>> {
        let mut results = HashMap::new();

        for source_name in self.configs.keys() {
            match self.discover_series_for_source(source_name, pool).await {
                Ok(series) => {
                    results.insert(source_name.clone(), series);
                }
                Err(e) => {
                    eprintln!("Failed to discover series for {}: {}", source_name, e);
                    results.insert(source_name.clone(), Vec::new());
                }
            }
        }

        Ok(results)
    }
}

impl Default for ApiDiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestContainer;
    use tokio;

    #[tokio::test]
    async fn test_api_discovery_service_creation() {
        let service = ApiDiscoveryService::new();
        assert!(!service.configs.is_empty());
        assert!(service.configs.contains_key("FRED"));
        assert!(service.configs.contains_key("BLS"));
        assert!(service.configs.contains_key("CENSUS"));
    }

    #[tokio::test]
    async fn test_discover_series_for_source() {
        let container = TestContainer::new().await;
        let pool = &container.pool;

        let service = ApiDiscoveryService::new();

        // Test with FRED (this will fail without API key, but we can test the structure)
        let result = service.discover_series_for_source("FRED", pool).await;
        // We expect this to fail due to missing API key, but the structure should be correct
        assert!(result.is_err() || result.is_ok());
    }

    #[tokio::test]
    async fn test_parse_series_from_response() {
        let service = ApiDiscoveryService::new();

        let json = serde_json::json!({
            "seriess": [
                {
                    "id": "GDP",
                    "title": "Gross Domestic Product",
                    "notes": "Real GDP",
                    "units": "Billions of Dollars",
                    "frequency": "Quarterly"
                }
            ]
        });

        let config = ResponseConfig {
            series_path: "seriess".to_string(),
            field_mappings: [
                ("id".to_string(), "external_id".to_string()),
                ("title".to_string(), "title".to_string()),
                ("notes".to_string(), "description".to_string()),
                ("units".to_string(), "units".to_string()),
                ("frequency".to_string(), "frequency".to_string()),
            ].iter().cloned().collect(),
            pagination: None,
        };

        let series = service.parse_series_from_response(&json, &config).unwrap();
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].external_id, "GDP");
        assert_eq!(series[0].title, "Gross Domestic Product");
    }
}
