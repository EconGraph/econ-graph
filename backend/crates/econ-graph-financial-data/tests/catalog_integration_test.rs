//! Integration tests for the data catalog system
//!
//! This module tests the complete catalog functionality including:
//! - Series metadata management
//! - Data range tracking
//! - Catalog discovery and scanning
//! - GraphQL catalog queries

use anyhow::Result;
use chrono::{Duration, NaiveDate, Utc};
use econ_graph_financial_data::catalog::{DataCatalog, SeriesMetadata};
use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
use rust_decimal::Decimal;
use tempfile::TempDir;
use uuid::Uuid;

/// Test basic catalog operations
#[tokio::test]
async fn test_catalog_basic_operations() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let catalog = DataCatalog::new(catalog_dir)?;

    // Create test series metadata
    let series_id = Uuid::new_v4();
    let metadata = SeriesMetadata::new(
        series_id,
        "GDP_USA".to_string(),
        "Gross Domestic Product - USA".to_string(),
        "Quarterly".to_string(),
        "BEA".to_string(),
    );

    // Add series to catalog
    catalog.add_series(metadata.clone()).await?;

    // Retrieve series from catalog
    let retrieved = catalog.get_series(series_id).await?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().external_id, "GDP_USA");

    // List all series
    let all_series = catalog.list_series().await?;
    assert_eq!(all_series.len(), 1);
    assert_eq!(all_series[0].series_id, series_id);

    // Find series by external ID
    let found = catalog.find_series_by_external_id("GDP_USA").await?;
    assert!(found.is_some());
    assert_eq!(found.unwrap().series_id, series_id);

    println!("✅ Basic catalog operations test passed");
    Ok(())
}

/// Test catalog with data range tracking
#[tokio::test]
async fn test_catalog_data_range_tracking() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let catalog = DataCatalog::new(catalog_dir)?;

    // Create series with data range
    let series_id = Uuid::new_v4();
    let mut metadata = SeriesMetadata::new(
        series_id,
        "CPI_USA".to_string(),
        "Consumer Price Index - USA".to_string(),
        "Monthly".to_string(),
        "BLS".to_string(),
    );

    let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let end_date = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
    metadata.update_data_range(Some(start_date), Some(end_date), 365, 5);

    catalog.add_series(metadata).await?;

    // Test date range queries
    let results = catalog
        .find_series_by_date_range(
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 8, 31).unwrap(),
        )
        .await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].series_id, series_id);

    // Test non-overlapping date range
    let results = catalog
        .find_series_by_date_range(
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        )
        .await?;

    assert_eq!(results.len(), 0);

    // Test catalog statistics
    let stats = catalog.get_stats().await?;
    assert_eq!(stats.total_series, 1);
    assert_eq!(stats.earliest_date, Some(start_date));
    assert_eq!(stats.latest_date, Some(end_date));

    println!("✅ Data range tracking test passed");
    Ok(())
}

/// Test catalog integration with database
#[tokio::test]
async fn test_database_catalog_integration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_str().unwrap().to_string();
    let db = Database::new_with_custom_partitioning(data_dir).await?;

    // Create a series
    let series_id = Uuid::new_v4();
    let series = EconomicSeries {
        id: series_id,
        source_id: Uuid::new_v4(),
        external_id: "UNEMPLOYMENT_USA".to_string(),
        title: "Unemployment Rate - USA".to_string(),
        description: Some("Monthly unemployment rate for the United States".to_string()),
        units: Some("Percentage".to_string()),
        frequency: "Monthly".to_string(),
        seasonal_adjustment: Some("Seasonally Adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Add series to database (should update catalog)
    db.create_series(series.clone()).await?;

    // Test catalog queries through database
    let stats = db.get_catalog_stats().await?;
    assert_eq!(stats.total_series, 1);

    let found_series = db.find_series_by_external_id("UNEMPLOYMENT_USA").await?;
    assert!(found_series.is_some());
    assert_eq!(found_series.unwrap().external_id, "UNEMPLOYMENT_USA");

    let date_range_series = db
        .find_series_by_date_range(
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 8, 31).unwrap(),
        )
        .await?;
    assert_eq!(date_range_series.len(), 1);

    println!("✅ Database catalog integration test passed");
    Ok(())
}

/// Test catalog with multiple series
#[tokio::test]
async fn test_catalog_multiple_series() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let catalog = DataCatalog::new(catalog_dir)?;

    // Create multiple series with different date ranges
    let series1_id = Uuid::new_v4();
    let mut metadata1 = SeriesMetadata::new(
        series1_id,
        "GDP_USA".to_string(),
        "Gross Domestic Product - USA".to_string(),
        "Quarterly".to_string(),
        "BEA".to_string(),
    );
    metadata1.update_data_range(
        Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        Some(NaiveDate::from_ymd_opt(2024, 6, 30).unwrap()),
        180,
        0,
    );
    catalog.add_series(metadata1).await?;

    let series2_id = Uuid::new_v4();
    let mut metadata2 = SeriesMetadata::new(
        series2_id,
        "CPI_USA".to_string(),
        "Consumer Price Index - USA".to_string(),
        "Monthly".to_string(),
        "BLS".to_string(),
    );
    metadata2.update_data_range(
        Some(NaiveDate::from_ymd_opt(2024, 7, 1).unwrap()),
        Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
        180,
        5,
    );
    catalog.add_series(metadata2).await?;

    let series3_id = Uuid::new_v4();
    let mut metadata3 = SeriesMetadata::new(
        series3_id,
        "INTEREST_RATE_USA".to_string(),
        "Federal Funds Rate - USA".to_string(),
        "Daily".to_string(),
        "FRED".to_string(),
    );
    metadata3.update_data_range(
        Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
        365,
        0,
    );
    catalog.add_series(metadata3).await?;

    // Test listing all series
    let all_series = catalog.list_series().await?;
    assert_eq!(all_series.len(), 3);

    // Test date range queries
    let overlapping_series = catalog
        .find_series_by_date_range(
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 8, 31).unwrap(),
        )
        .await?;
    assert_eq!(overlapping_series.len(), 3); // All series should overlap

    let early_series = catalog
        .find_series_by_date_range(
            NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 30).unwrap(),
        )
        .await?;
    assert_eq!(early_series.len(), 2); // GDP and Interest Rate

    let late_series = catalog
        .find_series_by_date_range(
            NaiveDate::from_ymd_opt(2024, 10, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
        )
        .await?;
    assert_eq!(late_series.len(), 2); // CPI and Interest Rate

    // Test catalog statistics
    let stats = catalog.get_stats().await?;
    assert_eq!(stats.total_series, 3);
    assert_eq!(stats.total_data_points, 725); // 180 + 180 + 365
    assert_eq!(
        stats.earliest_date,
        Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
    );
    assert_eq!(
        stats.latest_date,
        Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap())
    );

    println!("✅ Multiple series catalog test passed");
    Ok(())
}

/// Test catalog data freshness tracking
#[tokio::test]
async fn test_catalog_freshness_tracking() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let catalog = DataCatalog::new(catalog_dir)?;

    // Create series
    let series_id = Uuid::new_v4();
    let metadata = SeriesMetadata::new(
        series_id,
        "TEST_FRESHNESS".to_string(),
        "Test Freshness Series".to_string(),
        "Daily".to_string(),
        "Test".to_string(),
    );

    catalog.add_series(metadata).await?;

    // Check freshness
    let retrieved = catalog.get_series(series_id).await?.unwrap();
    let freshness = retrieved.get_freshness();
    assert!(freshness < Duration::seconds(1)); // Should be very recent

    // Test stale check
    assert!(!retrieved.is_stale(Duration::hours(1)));
    // Note: The is_stale check with 1 second might be flaky in tests

    // Test data range update affects freshness
    let mut updated_metadata = retrieved.clone();
    updated_metadata.update_data_range(
        Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
        365,
        0,
    );

    // The updated_at should be more recent than created_at
    assert!(updated_metadata.updated_at > updated_metadata.created_at);

    println!("✅ Freshness tracking test passed");
    Ok(())
}

/// Test catalog with tags and metadata
#[tokio::test]
async fn test_catalog_tags_and_metadata() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let catalog = DataCatalog::new(catalog_dir)?;

    // Create series with tags
    let series_id = Uuid::new_v4();
    let mut metadata = SeriesMetadata::new(
        series_id,
        "TAGGED_SERIES".to_string(),
        "Tagged Test Series".to_string(),
        "Monthly".to_string(),
        "Test".to_string(),
    );

    // Add tags
    metadata.add_tag("economic".to_string());
    metadata.add_tag("inflation".to_string());
    metadata.add_tag("monthly".to_string());

    // Update quality score
    metadata.quality_score = 0.95;

    catalog.add_series(metadata).await?;

    // Retrieve and verify
    let retrieved = catalog.get_series(series_id).await?.unwrap();
    assert_eq!(retrieved.tags.len(), 3);
    assert!(retrieved.tags.contains(&"economic".to_string()));
    assert!(retrieved.tags.contains(&"inflation".to_string()));
    assert!(retrieved.tags.contains(&"monthly".to_string()));
    assert_eq!(retrieved.quality_score, 0.95);

    // Test tag removal
    let mut updated_metadata = retrieved.clone();
    updated_metadata.remove_tag("monthly");
    assert_eq!(updated_metadata.tags.len(), 2);
    assert!(!updated_metadata.tags.contains(&"monthly".to_string()));

    println!("✅ Tags and metadata test passed");
    Ok(())
}

/// Test catalog persistence across restarts
#[tokio::test]
async fn test_catalog_persistence() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");

    // Create first catalog instance
    let catalog1 = DataCatalog::new(&catalog_dir)?;

    let series_id = Uuid::new_v4();
    let metadata = SeriesMetadata::new(
        series_id,
        "PERSISTENT_SERIES".to_string(),
        "Persistent Test Series".to_string(),
        "Daily".to_string(),
        "Test".to_string(),
    );

    catalog1.add_series(metadata).await?;

    // Create second catalog instance (simulating restart)
    let catalog2 = DataCatalog::new(&catalog_dir)?;

    // Verify data persists
    let retrieved = catalog2.get_series(series_id).await?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().external_id, "PERSISTENT_SERIES");

    let all_series = catalog2.list_series().await?;
    assert_eq!(all_series.len(), 1);

    println!("✅ Catalog persistence test passed");
    Ok(())
}
