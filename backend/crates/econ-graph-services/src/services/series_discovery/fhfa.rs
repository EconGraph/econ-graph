//! FHFA (Federal Housing Finance Agency) data source integration
//!
//! This module provides integration with FHFA's House Price Index data,
//! which measures changes in single-family home values across the United States.

use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::AppResult;
use econ_graph_core::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// FHFA House Price Index data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhfaHpiData {
    /// Year of the data
    pub year: i32,
    /// Quarter (1-4)
    pub quarter: i32,
    /// State abbreviation (e.g., "CA", "NY")
    pub state: Option<String>,
    /// Metropolitan area name
    pub metro_area: Option<String>,
    /// House Price Index value
    pub hpi_value: f64,
    /// Year-over-year change percentage
    pub yoy_change: Option<f64>,
    /// Quarter-over-quarter change percentage
    pub qoq_change: Option<f64>,
}

/// FHFA API response structure
#[derive(Debug, Serialize, Deserialize)]
struct FhfaApiResponse {
    data: Vec<FhfaHpiData>,
    meta: FhfaMeta,
}

/// FHFA API metadata
#[derive(Debug, Serialize, Deserialize)]
struct FhfaMeta {
    total_count: u32,
    page: u32,
    per_page: u32,
}

/// FHFA data source configuration
pub struct FhfaDataSource {
    client: Client,
    base_url: String,
}

impl Default for FhfaDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl FhfaDataSource {
    /// Create a new FHFA data source
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api.fhfa.gov".to_string(),
        }
    }

    /// Discover available FHFA House Price Index series
    pub async fn discover_series(&self) -> AppResult<Vec<FhfaSeriesInfo>> {
        let mut series_list = Vec::new();

        // National House Price Index
        series_list.push(FhfaSeriesInfo {
            external_id: "USHPI".to_string(),
            title: "U.S. House Price Index".to_string(),
            description: "Quarterly house price index for the United States".to_string(),
            frequency: "Quarterly".to_string(),
            units: "Index (1991Q1 = 100)".to_string(),
            geographic_level: "National".to_string(),
            data_url: format!("{}/v1/house-price-index/national", self.base_url),
        });

        // State-level House Price Indexes
        let states = self.get_available_states().await?;
        for state in states {
            series_list.push(FhfaSeriesInfo {
                external_id: format!("{}HPI", state.code),
                title: format!("{} House Price Index", state.name),
                description: format!("Quarterly house price index for {}", state.name),
                frequency: "Quarterly".to_string(),
                units: "Index (1991Q1 = 100)".to_string(),
                geographic_level: "State".to_string(),
                data_url: format!(
                    "{}/v1/house-price-index/state/{}",
                    self.base_url, state.code
                ),
            });
        }

        // Metropolitan area House Price Indexes
        let metro_areas = self.get_available_metro_areas().await?;
        for metro in metro_areas {
            series_list.push(FhfaSeriesInfo {
                external_id: format!("{}HPI", metro.code),
                title: format!("{} House Price Index", metro.name),
                description: format!("Quarterly house price index for {}", metro.name),
                frequency: "Quarterly".to_string(),
                units: "Index (1991Q1 = 100)".to_string(),
                geographic_level: "Metropolitan Area".to_string(),
                data_url: format!(
                    "{}/v1/house-price-index/metro/{}",
                    self.base_url, metro.code
                ),
            });
        }

        Ok(series_list)
    }

    /// Fetch House Price Index data for a specific series
    pub async fn fetch_hpi_data(
        &self,
        series_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> AppResult<Vec<FhfaHpiData>> {
        let mut url = format!("{}/v1/house-price-index/{}", self.base_url, series_id);

        let mut params = Vec::new();
        if let Some(start) = start_date {
            params.push(format!("start_date={}", start));
        }
        if let Some(end) = end_date {
            params.push(format!("end_date={}", end));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "EconGraph/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(econ_graph_core::error::AppError::ExternalApiError(format!(
                "FHFA API error: {}",
                response.status()
            )));
        }

        let api_response: FhfaApiResponse = response.json().await?;
        Ok(api_response.data)
    }

    /// Get available states for House Price Index data
    async fn get_available_states(&self) -> AppResult<Vec<StateInfo>> {
        // For now, return a comprehensive list of US states
        // In a real implementation, this would be fetched from the API
        Ok(vec![
            StateInfo {
                code: "AL".to_string(),
                name: "Alabama".to_string(),
            },
            StateInfo {
                code: "AK".to_string(),
                name: "Alaska".to_string(),
            },
            StateInfo {
                code: "AZ".to_string(),
                name: "Arizona".to_string(),
            },
            StateInfo {
                code: "AR".to_string(),
                name: "Arkansas".to_string(),
            },
            StateInfo {
                code: "CA".to_string(),
                name: "California".to_string(),
            },
            StateInfo {
                code: "CO".to_string(),
                name: "Colorado".to_string(),
            },
            StateInfo {
                code: "CT".to_string(),
                name: "Connecticut".to_string(),
            },
            StateInfo {
                code: "DE".to_string(),
                name: "Delaware".to_string(),
            },
            StateInfo {
                code: "FL".to_string(),
                name: "Florida".to_string(),
            },
            StateInfo {
                code: "GA".to_string(),
                name: "Georgia".to_string(),
            },
            StateInfo {
                code: "HI".to_string(),
                name: "Hawaii".to_string(),
            },
            StateInfo {
                code: "ID".to_string(),
                name: "Idaho".to_string(),
            },
            StateInfo {
                code: "IL".to_string(),
                name: "Illinois".to_string(),
            },
            StateInfo {
                code: "IN".to_string(),
                name: "Indiana".to_string(),
            },
            StateInfo {
                code: "IA".to_string(),
                name: "Iowa".to_string(),
            },
            StateInfo {
                code: "KS".to_string(),
                name: "Kansas".to_string(),
            },
            StateInfo {
                code: "KY".to_string(),
                name: "Kentucky".to_string(),
            },
            StateInfo {
                code: "LA".to_string(),
                name: "Louisiana".to_string(),
            },
            StateInfo {
                code: "ME".to_string(),
                name: "Maine".to_string(),
            },
            StateInfo {
                code: "MD".to_string(),
                name: "Maryland".to_string(),
            },
            StateInfo {
                code: "MA".to_string(),
                name: "Massachusetts".to_string(),
            },
            StateInfo {
                code: "MI".to_string(),
                name: "Michigan".to_string(),
            },
            StateInfo {
                code: "MN".to_string(),
                name: "Minnesota".to_string(),
            },
            StateInfo {
                code: "MS".to_string(),
                name: "Mississippi".to_string(),
            },
            StateInfo {
                code: "MO".to_string(),
                name: "Missouri".to_string(),
            },
            StateInfo {
                code: "MT".to_string(),
                name: "Montana".to_string(),
            },
            StateInfo {
                code: "NE".to_string(),
                name: "Nebraska".to_string(),
            },
            StateInfo {
                code: "NV".to_string(),
                name: "Nevada".to_string(),
            },
            StateInfo {
                code: "NH".to_string(),
                name: "New Hampshire".to_string(),
            },
            StateInfo {
                code: "NJ".to_string(),
                name: "New Jersey".to_string(),
            },
            StateInfo {
                code: "NM".to_string(),
                name: "New Mexico".to_string(),
            },
            StateInfo {
                code: "NY".to_string(),
                name: "New York".to_string(),
            },
            StateInfo {
                code: "NC".to_string(),
                name: "North Carolina".to_string(),
            },
            StateInfo {
                code: "ND".to_string(),
                name: "North Dakota".to_string(),
            },
            StateInfo {
                code: "OH".to_string(),
                name: "Ohio".to_string(),
            },
            StateInfo {
                code: "OK".to_string(),
                name: "Oklahoma".to_string(),
            },
            StateInfo {
                code: "OR".to_string(),
                name: "Oregon".to_string(),
            },
            StateInfo {
                code: "PA".to_string(),
                name: "Pennsylvania".to_string(),
            },
            StateInfo {
                code: "RI".to_string(),
                name: "Rhode Island".to_string(),
            },
            StateInfo {
                code: "SC".to_string(),
                name: "South Carolina".to_string(),
            },
            StateInfo {
                code: "SD".to_string(),
                name: "South Dakota".to_string(),
            },
            StateInfo {
                code: "TN".to_string(),
                name: "Tennessee".to_string(),
            },
            StateInfo {
                code: "TX".to_string(),
                name: "Texas".to_string(),
            },
            StateInfo {
                code: "UT".to_string(),
                name: "Utah".to_string(),
            },
            StateInfo {
                code: "VT".to_string(),
                name: "Vermont".to_string(),
            },
            StateInfo {
                code: "VA".to_string(),
                name: "Virginia".to_string(),
            },
            StateInfo {
                code: "WA".to_string(),
                name: "Washington".to_string(),
            },
            StateInfo {
                code: "WV".to_string(),
                name: "West Virginia".to_string(),
            },
            StateInfo {
                code: "WI".to_string(),
                name: "Wisconsin".to_string(),
            },
            StateInfo {
                code: "WY".to_string(),
                name: "Wyoming".to_string(),
            },
            StateInfo {
                code: "DC".to_string(),
                name: "District of Columbia".to_string(),
            },
        ])
    }

    /// Get available metropolitan areas for House Price Index data
    async fn get_available_metro_areas(&self) -> AppResult<Vec<MetroAreaInfo>> {
        // For now, return a selection of major metropolitan areas
        // In a real implementation, this would be fetched from the API
        Ok(vec![
            MetroAreaInfo {
                code: "NYC".to_string(),
                name: "New York-Newark-Jersey City, NY-NJ-PA".to_string(),
            },
            MetroAreaInfo {
                code: "LA".to_string(),
                name: "Los Angeles-Long Beach-Anaheim, CA".to_string(),
            },
            MetroAreaInfo {
                code: "CHI".to_string(),
                name: "Chicago-Naperville-Elgin, IL-IN-WI".to_string(),
            },
            MetroAreaInfo {
                code: "DAL".to_string(),
                name: "Dallas-Fort Worth-Arlington, TX".to_string(),
            },
            MetroAreaInfo {
                code: "HOU".to_string(),
                name: "Houston-The Woodlands-Sugar Land, TX".to_string(),
            },
            MetroAreaInfo {
                code: "PHX".to_string(),
                name: "Phoenix-Mesa-Chandler, AZ".to_string(),
            },
            MetroAreaInfo {
                code: "PHI".to_string(),
                name: "Philadelphia-Camden-Wilmington, PA-NJ-DE-MD".to_string(),
            },
            MetroAreaInfo {
                code: "SAN".to_string(),
                name: "San Antonio-New Braunfels, TX".to_string(),
            },
            MetroAreaInfo {
                code: "SD".to_string(),
                name: "San Diego-Chula Vista-Carlsbad, CA".to_string(),
            },
            MetroAreaInfo {
                code: "AUS".to_string(),
                name: "Austin-Round Rock-Georgetown, TX".to_string(),
            },
            MetroAreaInfo {
                code: "JAX".to_string(),
                name: "Jacksonville, FL".to_string(),
            },
            MetroAreaInfo {
                code: "FTW".to_string(),
                name: "Fort Worth-Arlington, TX".to_string(),
            },
            MetroAreaInfo {
                code: "COL".to_string(),
                name: "Columbus, OH".to_string(),
            },
            MetroAreaInfo {
                code: "CHA".to_string(),
                name: "Charlotte-Concord-Gastonia, NC-SC".to_string(),
            },
            MetroAreaInfo {
                code: "SF".to_string(),
                name: "San Francisco-Oakland-Berkeley, CA".to_string(),
            },
            MetroAreaInfo {
                code: "IND".to_string(),
                name: "Indianapolis-Carmel-Anderson, IN".to_string(),
            },
            MetroAreaInfo {
                code: "SEA".to_string(),
                name: "Seattle-Tacoma-Bellevue, WA".to_string(),
            },
            MetroAreaInfo {
                code: "DEN".to_string(),
                name: "Denver-Aurora-Lakewood, CO".to_string(),
            },
            MetroAreaInfo {
                code: "WAS".to_string(),
                name: "Washington-Arlington-Alexandria, DC-VA-MD-WV".to_string(),
            },
            MetroAreaInfo {
                code: "BOS".to_string(),
                name: "Boston-Cambridge-Newton, MA-NH".to_string(),
            },
        ])
    }
}

/// Information about a FHFA series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhfaSeriesInfo {
    pub external_id: String,
    pub title: String,
    pub description: String,
    pub frequency: String,
    pub units: String,
    pub geographic_level: String,
    pub data_url: String,
}

/// State information
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateInfo {
    code: String,
    name: String,
}

/// Metropolitan area information
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetroAreaInfo {
    code: String,
    name: String,
}

/// Discover FHFA House Price Index series and catalog them
pub async fn discover_fhfa_series(_client: &Client, pool: &DatabasePool) -> AppResult<Vec<String>> {
    let fhfa_source = FhfaDataSource::new();
    let series_list = fhfa_source.discover_series().await?;

    // Get or create FHFA data source
    let fhfa_data_source = DataSource::get_or_create(pool, DataSource::fhfa()).await?;

    let mut discovered_series = Vec::new();

    for series_info in series_list {
        // Create new economic series
        let new_series = NewEconomicSeries {
            source_id: fhfa_data_source.id,
            external_id: series_info.external_id.clone(),
            title: series_info.title,
            description: Some(series_info.description),
            units: Some(series_info.units),
            frequency: series_info.frequency,
            seasonal_adjustment: None,
            start_date: None,
            end_date: None,
            is_active: true,
            first_discovered_at: Some(chrono::Utc::now()),
            last_crawled_at: None,
            first_missing_date: None,
            crawl_status: None,
            crawl_error_message: None,
        };

        // Get or create the series
        match EconomicSeries::get_or_create(
            pool,
            &series_info.external_id,
            fhfa_data_source.id,
            &new_series,
        )
        .await
        {
            Ok(_) => {
                discovered_series.push(series_info.external_id);
            }
            Err(e) => {
                eprintln!(
                    "Failed to create FHFA series {}: {}",
                    series_info.external_id, e
                );
            }
        }
    }

    Ok(discovered_series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fhfa_data_source_creation() {
        let fhfa = FhfaDataSource::new();
        assert_eq!(fhfa.base_url, "https://api.fhfa.gov");
    }

    #[tokio::test]
    async fn test_discover_series() {
        let fhfa = FhfaDataSource::new();
        let series = fhfa.discover_series().await;

        // Should return series for national, state, and metro levels
        assert!(series.is_ok());
        let series_list = series.unwrap();
        assert!(!series_list.is_empty());

        // Should include national series
        assert!(series_list.iter().any(|s| s.external_id == "USHPI"));

        // Should include state series
        assert!(series_list.iter().any(|s| s.external_id == "CAHPI"));

        // Should include metro series
        assert!(series_list.iter().any(|s| s.external_id == "NYCHPI"));
    }

    #[test]
    fn test_fhfa_hpi_data_structure() {
        let hpi_data = FhfaHpiData {
            year: 2023,
            quarter: 4,
            state: Some("CA".to_string()),
            metro_area: None,
            hpi_value: 350.5,
            yoy_change: Some(5.2),
            qoq_change: Some(1.1),
        };

        assert_eq!(hpi_data.year, 2023);
        assert_eq!(hpi_data.quarter, 4);
        assert_eq!(hpi_data.hpi_value, 350.5);
        assert_eq!(hpi_data.yoy_change, Some(5.2));
    }
}
