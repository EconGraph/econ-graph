//! Unit tests for Census Bureau API integration, focusing on BDS dataset

use crate::error::AppResult;
use crate::models::DataSource;
use crate::services::series_discovery::census::{
    discover_census_series, fetch_bds_data, fetch_bds_sample_data, filter_economic_indicators,
    BdsDataPoint, BdsVariable, CensusQueryBuilder,
};
use crate::test_utils::TestContainer;
use reqwest::Client;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_census_data_source_config() -> AppResult<()> {
    let container = TestContainer::new().await;
    let pool = container.pool().clone();

    // Test that Census data source can be created
    let census_source = DataSource::get_or_create(&pool, DataSource::census()).await?;

    assert_eq!(census_source.name, "U.S. Census Bureau");
    assert!(census_source
        .description
        .as_ref()
        .unwrap()
        .contains("Demographic and economic data"));
    assert_eq!(census_source.base_url, "https://api.census.gov/data");
    assert!(!census_source.api_key_required); // Census doesn't require API key

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_filter_economic_indicators() -> AppResult<()> {
    // Create test variables
    let variables = vec![
        BdsVariable {
            name: "ESTAB".to_string(),
            label: "Establishments".to_string(),
            concept: "Business Counts".to_string(),
            predicate_type: "int".to_string(),
            group: "N/A".to_string(),
            limit: 0,
            predicate_only: Some(false),
        },
        BdsVariable {
            name: "FIRM".to_string(),
            label: "Firms".to_string(),
            concept: "Business Counts".to_string(),
            predicate_type: "int".to_string(),
            group: "N/A".to_string(),
            limit: 0,
            predicate_only: Some(false),
        },
        BdsVariable {
            name: "JOB_CREATION".to_string(),
            label: "Job Creation".to_string(),
            concept: "Employment".to_string(),
            predicate_type: "int".to_string(),
            group: "N/A".to_string(),
            limit: 0,
            predicate_only: Some(false),
        },
        BdsVariable {
            name: "YEAR".to_string(),
            label: "Year".to_string(),
            concept: "Time".to_string(),
            predicate_type: "int".to_string(),
            group: "N/A".to_string(),
            limit: 0,
            predicate_only: Some(true), // This is a predicate variable
        },
        BdsVariable {
            name: "for".to_string(),
            label: "Geography".to_string(),
            concept: "Geography".to_string(),
            predicate_type: "string".to_string(),
            group: "N/A".to_string(),
            limit: 0,
            predicate_only: Some(true), // This is a predicate variable
        },
        BdsVariable {
            name: "RANDOM_VAR".to_string(),
            label: "Random Variable".to_string(),
            concept: "Other".to_string(),
            predicate_type: "int".to_string(),
            group: "N/A".to_string(),
            limit: 0,
            predicate_only: Some(false),
        },
    ];

    // Filter economic indicators
    let economic_vars = filter_economic_indicators(&variables);

    // Should include economic indicators but exclude time/geography variables
    assert_eq!(economic_vars.len(), 3);

    let var_names: Vec<&String> = economic_vars.iter().map(|v| &v.name).collect();
    assert!(var_names.contains(&&"ESTAB".to_string()));
    assert!(var_names.contains(&&"FIRM".to_string()));
    assert!(var_names.contains(&&"JOB_CREATION".to_string()));

    // Should exclude time and geography variables
    assert!(!var_names.contains(&&"YEAR".to_string()));
    assert!(!var_names.contains(&&"for".to_string()));
    assert!(!var_names.contains(&&"RANDOM_VAR".to_string()));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_series_discovery_integration() -> AppResult<()> {
    let container = TestContainer::new().await;
    let pool = container.pool().clone();
    let client = Client::new();

    // Test Census series discovery (this will make real API calls)
    let discovered_series = discover_census_series(&client, &None, &pool).await?;

    // Should discover some series
    assert!(!discovered_series.is_empty());

    // Should include BDS series (they start with CENSUS_BDS_)
    let bds_series: Vec<&String> = discovered_series
        .iter()
        .filter(|s| s.starts_with("CENSUS_BDS_"))
        .collect();

    assert!(!bds_series.is_empty(), "Should discover BDS series");

    // Verify series are stored in database
    let census_source = DataSource::get_or_create(&pool, DataSource::census()).await?;
    let stored_series = crate::models::EconomicSeries::find_all(&pool).await?;

    let census_series: Vec<_> = stored_series
        .iter()
        .filter(|s| s.source_id == census_source.id)
        .collect();

    assert!(
        !census_series.is_empty(),
        "Should have stored Census series in database"
    );

    // Verify BDS series have correct metadata
    let bds_series_in_db: Vec<_> = census_series
        .iter()
        .filter(|s| s.external_id.starts_with("CENSUS_BDS_"))
        .collect();

    assert!(
        !bds_series_in_db.is_empty(),
        "Should have BDS series in database"
    );

    // Check that BDS series have expected metadata
    for series in bds_series_in_db {
        assert_eq!(series.frequency, "Annual");
        assert_eq!(series.units, Some("Count".to_string()));
        assert!(series.title.contains(" - ")); // Should have format "Variable - Geography"
        assert!(series.description.is_some());
        assert!(series
            .description
            .as_ref()
            .unwrap()
            .contains("Business Dynamics Statistics"));
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_series_metadata_storage() -> AppResult<()> {
    let container = TestContainer::new().await;
    let pool = container.pool().clone();

    // Create Census data source
    let census_source = DataSource::get_or_create(&pool, DataSource::census()).await?;

    // Verify data source properties
    assert_eq!(census_source.name, "U.S. Census Bureau");
    assert!(census_source
        .description
        .as_ref()
        .unwrap()
        .contains("Demographic and economic data"));
    assert_eq!(census_source.base_url, "https://api.census.gov/data");
    assert!(!census_source.api_key_required); // No API key required

    // Verify data source is enabled
    assert!(census_source.is_enabled);

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_query_builder_basic() -> AppResult<()> {
    let client = Client::new();

    // Test basic query builder functionality
    let query = CensusQueryBuilder::new()
        .variables(&["ESTAB".to_string(), "FIRM".to_string(), "YEAR".to_string()])
        .geography("us")
        .time_range(2020, 2022);

    let url = query.build_url()?;

    // Verify URL construction
    assert!(url.contains("https://api.census.gov/data/timeseries/bds"));
    assert!(url.contains("get=ESTAB,FIRM,YEAR"));
    assert!(url.contains("for=us"));
    assert!(url.contains("YEAR=2020:2022"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_query_builder_defaults() -> AppResult<()> {
    let client = Client::new();

    // Test query builder with minimal parameters (should use defaults)
    let query = CensusQueryBuilder::new().variables(&["ESTAB".to_string()]);

    let url = query.build_url()?;

    // Should use default geography (us) and default time range (2020:2022)
    assert!(url.contains("get=ESTAB"));
    assert!(url.contains("for=us"));
    assert!(url.contains("YEAR=2020:2022"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_query_builder_api_key() -> AppResult<()> {
    let client = Client::new();

    // Test query builder with API key
    let query = CensusQueryBuilder::new()
        .variables(&["ESTAB".to_string(), "YEAR".to_string()])
        .geography("us")
        .api_key("test_api_key");

    let url = query.build_url()?;

    // Should include API key parameter
    assert!(url.contains("key=test_api_key"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_query_builder_validation() -> AppResult<()> {
    let client = Client::new();

    // Test query builder validation - should fail with no variables
    let query = CensusQueryBuilder::new();
    let result = query.build_url();

    assert!(result.is_err());
    if let Err(error) = result {
        assert!(error
            .to_string()
            .contains("At least one variable must be specified"));
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_query_builder_different_geography() -> AppResult<()> {
    let client = Client::new();

    // Test different geography levels
    let geographies = vec!["us", "state", "county", "metro", "cbsa"];

    for geo in geographies {
        let query = CensusQueryBuilder::new()
            .variables(&["ESTAB".to_string(), "YEAR".to_string()])
            .geography(geo);

        let url = query.build_url()?;
        assert!(url.contains(&format!("for={}", geo)));
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_query_builder_time_ranges() -> AppResult<()> {
    let client = Client::new();

    // Test different time ranges
    let query = CensusQueryBuilder::new()
        .variables(&["ESTAB".to_string(), "YEAR".to_string()])
        .geography("us")
        .time_range(2015, 2020);

    let url = query.build_url()?;
    assert!(url.contains("YEAR=2015:2020"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_query_builder_fluent_interface() -> AppResult<()> {
    let client = Client::new();

    // Test fluent interface chaining
    let query = CensusQueryBuilder::new()
        .variable("ESTAB")
        .variable("FIRM")
        .variable("YEAR")
        .geography("state")
        .time_range(2018, 2021)
        .api_key("test_key");

    let url = query.build_url()?;

    // Verify all parameters are included
    assert!(url.contains("get=ESTAB,FIRM,YEAR"));
    assert!(url.contains("for=state"));
    assert!(url.contains("YEAR=2018:2021"));
    assert!(url.contains("key=test_key"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_fetch_bds_sample_data() -> AppResult<()> {
    let client = Client::new();

    // Test fetching sample BDS data (this makes a real API call)
    let data_points = fetch_bds_sample_data(&client).await?;

    // Should get some data points
    assert!(!data_points.is_empty());

    // Verify data point structure
    for point in &data_points {
        assert!(point.year >= 2020 && point.year <= 2022);
        assert_eq!(point.geography, "us");
        assert!(point.variable == "ESTAB" || point.variable == "FIRM");
        // Values can be None or Some(f64)
    }

    // Should have data for both ESTAB and FIRM variables
    let variables: std::collections::HashSet<String> =
        data_points.iter().map(|p| p.variable.clone()).collect();

    assert!(variables.contains("ESTAB"));
    assert!(variables.contains("FIRM"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_fetch_bds_data_custom_params() -> AppResult<()> {
    let client = Client::new();

    // Test fetching BDS data with custom parameters
    let variables = vec!["ESTAB".to_string(), "FIRM".to_string(), "YEAR".to_string()];

    let data_points = fetch_bds_data(&client, &variables, "us", 2021, 2022, &None).await?;

    // Should get some data points
    assert!(!data_points.is_empty());

    // Verify data point structure
    for point in &data_points {
        assert!(point.year >= 2021 && point.year <= 2022);
        assert_eq!(point.geography, "us");
        assert!(point.variable == "ESTAB" || point.variable == "FIRM");
    }

    // Should have data for both variables
    let variables_found: std::collections::HashSet<String> =
        data_points.iter().map(|p| p.variable.clone()).collect();

    assert!(variables_found.contains("ESTAB"));
    assert!(variables_found.contains("FIRM"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_query_execution() -> AppResult<()> {
    let client = Client::new();

    // Test executing a query and parsing the response
    let query = CensusQueryBuilder::new()
        .variables(&["ESTAB".to_string(), "YEAR".to_string()])
        .geography("us")
        .time_range(2022, 2022); // Single year for smaller response

    let raw_data = query.execute(&client).await?;

    // Should have at least header and one data row
    assert!(raw_data.len() >= 2);

    // First row should be headers
    let headers = &raw_data[0];
    assert!(headers.contains(&"ESTAB".to_string()));
    assert!(headers.contains(&"YEAR".to_string()));

    // Data rows should have correct number of columns
    for row in &raw_data[1..] {
        assert_eq!(row.len(), headers.len());
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_census_query_structured_execution() -> AppResult<()> {
    let client = Client::new();

    // Test executing a query and getting structured data points
    let query = CensusQueryBuilder::new()
        .variables(&["ESTAB".to_string(), "YEAR".to_string()])
        .geography("us")
        .time_range(2022, 2022); // Single year for smaller response

    let data_points = query.execute_structured(&client).await?;

    // Should get some data points
    assert!(!data_points.is_empty());

    // Verify data point structure
    for point in &data_points {
        assert_eq!(point.year, 2022);
        assert_eq!(point.geography, "us");
        assert_eq!(point.variable, "ESTAB");
        // Value can be None or Some(f64)
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_bds_data_point_structure() -> AppResult<()> {
    // Test BdsDataPoint struct creation and field access
    let data_point = BdsDataPoint {
        year: 2022,
        value: Some(12345.0),
        geography: "us".to_string(),
        variable: "ESTAB".to_string(),
    };

    assert_eq!(data_point.year, 2022);
    assert_eq!(data_point.value, Some(12345.0));
    assert_eq!(data_point.geography, "us");
    assert_eq!(data_point.variable, "ESTAB");

    // Test with None value
    let data_point_none = BdsDataPoint {
        year: 2021,
        value: None,
        geography: "state".to_string(),
        variable: "FIRM".to_string(),
    };

    assert_eq!(data_point_none.year, 2021);
    assert_eq!(data_point_none.value, None);
    assert_eq!(data_point_none.geography, "state");
    assert_eq!(data_point_none.variable, "FIRM");

    Ok(())
}
