//! Comprehensive tests for World Bank API integration

use crate::services::series_discovery::world_bank::{
    is_economic_indicator, WorldBankIndicator, WorldBankSeriesInfo,
};
use econ_graph_core::models::{DataSource, EconomicSeries};
use econ_graph_core::test_utils::TestContainer;
use reqwest::Client;
use serial_test::serial;

/// Test World Bank data source configuration (unit test - no database)
#[test]
fn test_world_bank_data_source_config() {
    let world_bank_source = DataSource::world_bank();

    // Verify World Bank data source configuration
    assert_eq!(world_bank_source.name, "World Bank Open Data");
    assert!(!world_bank_source.api_key_required);
    assert_eq!(world_bank_source.rate_limit_per_minute, 1000);
    assert_eq!(world_bank_source.base_url, "https://api.worldbank.org/v2");
    assert!(world_bank_source.description.is_some());
    assert!(world_bank_source
        .description
        .unwrap()
        .contains("Global economic"));
}

/// Test economic indicator filtering logic
#[test]
fn test_is_economic_indicator() {
    // Test GDP indicators
    let gdp_indicator = WorldBankIndicator {
        id: "NY.GDP.MKTP.CD".to_string(),
        name: "GDP (current US$)".to_string(),
        source: crate::services::series_discovery::world_bank::WorldBankSource {
            id: "2".to_string(),
            value: "World Development Indicators".to_string(),
        },
        source_note: Some("GDP description".to_string()),
        source_organization: Some("World Bank".to_string()),
    };
    assert!(is_economic_indicator(&gdp_indicator));

    // Test inflation indicators
    let inflation_indicator = WorldBankIndicator {
        id: "FP.CPI.TOTL.ZG".to_string(),
        name: "Inflation, consumer prices (annual %)".to_string(),
        source: crate::services::series_discovery::world_bank::WorldBankSource {
            id: "2".to_string(),
            value: "World Development Indicators".to_string(),
        },
        source_note: Some("Inflation description".to_string()),
        source_organization: Some("IMF".to_string()),
    };
    assert!(is_economic_indicator(&inflation_indicator));

    // Test unemployment indicators
    let unemployment_indicator = WorldBankIndicator {
        id: "SL.UEM.TOTL.ZS".to_string(),
        name: "Unemployment, total (% of total labor force)".to_string(),
        source: crate::services::series_discovery::world_bank::WorldBankSource {
            id: "2".to_string(),
            value: "World Development Indicators".to_string(),
        },
        source_note: Some("Unemployment description".to_string()),
        source_organization: Some("ILO".to_string()),
    };
    assert!(is_economic_indicator(&unemployment_indicator));

    // Test non-economic indicators
    let non_economic_indicator = WorldBankIndicator {
        id: "SH.STA.ACSN".to_string(),
        name: "Improved sanitation facilities (% of population with access)".to_string(),
        source: crate::services::series_discovery::world_bank::WorldBankSource {
            id: "2".to_string(),
            value: "World Development Indicators".to_string(),
        },
        source_note: Some("Sanitation description".to_string()),
        source_organization: Some("WHO".to_string()),
    };
    assert!(!is_economic_indicator(&non_economic_indicator));

    // Test indicators with economic keywords in name
    let trade_indicator = WorldBankIndicator {
        id: "NE.TRD.GNFS.ZS".to_string(),
        name: "Trade (% of GDP)".to_string(),
        source: crate::services::series_discovery::world_bank::WorldBankSource {
            id: "2".to_string(),
            value: "World Development Indicators".to_string(),
        },
        source_note: Some("Trade description".to_string()),
        source_organization: Some("World Bank".to_string()),
    };
    assert!(is_economic_indicator(&trade_indicator));
}

/// Test World Bank series metadata storage
#[tokio::test]
#[serial]
async fn test_world_bank_series_metadata_storage() -> Result<(), Box<dyn std::error::Error>> {
    let container = TestContainer::new().await;
    let pool = container.pool().clone();

    // Create World Bank data source
    let world_bank_source = DataSource::get_or_create(&pool, DataSource::world_bank()).await?;

    // Create test series info
    let series_info = WorldBankSeriesInfo {
        series_id: "NY.GDP.MKTP.CD".to_string(),
        title: "GDP (current US$)".to_string(),
        description: Some("GDP is the sum of gross value added...".to_string()),
        frequency: "Annual".to_string(),
        units: "Current US$".to_string(),
        source: "World Development Indicators".to_string(),
        country: None,
        start_date: Some("1960-01-01".to_string()),
        end_date: None,
    };

    // Store series
    let new_series = econ_graph_core::models::NewEconomicSeries {
        source_id: world_bank_source.id,
        external_id: series_info.series_id.clone(),
        title: series_info.title.clone(),
        description: series_info.description.clone(),
        units: Some(series_info.units.clone()),
        frequency: series_info.frequency.clone(),
        seasonal_adjustment: None,
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

    let stored_series = EconomicSeries::get_or_create(
        &pool,
        &series_info.series_id,
        world_bank_source.id,
        &new_series,
    )
    .await?;

    // Verify stored data
    assert_eq!(stored_series.external_id, "NY.GDP.MKTP.CD");
    assert_eq!(stored_series.title, "GDP (current US$)");
    assert_eq!(stored_series.frequency, "Annual");
    assert_eq!(stored_series.units, Some("Current US$".to_string()));
    assert!(stored_series.is_active);

    Ok(())
}
