use anyhow::Result;
use chrono::{NaiveDate, Utc};
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
use econ_graph_financial_data::storage::{FinancialDataStorage, ParquetStorage};
use rust_decimal::Decimal;
use std::fs;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn test_parquet_file_operations() -> Result<()> {
    // Create a temporary directory for test data
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_path_buf();

    // Create the Parquet storage
    let storage = ParquetStorage::new(data_dir.clone());

    // Create a test series
    let series_id = Uuid::new_v4();
    let source_id = Uuid::new_v4();

    let series = EconomicSeries {
        id: series_id,
        source_id,
        external_id: "TEST_PARQUET_001".to_string(),
        title: "Parquet Test Series".to_string(),
        description: Some("A test series for Parquet operations".to_string()),
        units: Some("Units".to_string()),
        frequency: "monthly".to_string(),
        seasonal_adjustment: Some("Not Seasonally Adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Test 1: Write series to Parquet file
    storage.write_series(&series).await?;

    // Verify the file was created
    let series_file = data_dir.join(format!("series_{}.parquet", series_id));
    assert!(series_file.exists(), "Series Parquet file should exist");

    // Test 2: Read series from Parquet file
    let retrieved_series = storage.read_series(series_id).await?;
    assert!(retrieved_series.is_some(), "Should retrieve the series");

    let retrieved = retrieved_series.unwrap();
    assert_eq!(retrieved.title, "Parquet Test Series");
    assert_eq!(retrieved.external_id, "TEST_PARQUET_001");
    assert_eq!(retrieved.frequency, "monthly");

    // Test 3: Create and write data points
    let data_points = vec![
        DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::new(1000, 0))),
            revision_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date: NaiveDate::from_ymd_opt(2023, 2, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::new(1050, 0))),
            revision_date: NaiveDate::from_ymd_opt(2023, 2, 1).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    storage.write_data_points(series_id, &data_points).await?;

    // Verify the data points file was created
    let data_points_file = data_dir.join(format!("data_points_{}.parquet", series_id));
    assert!(
        data_points_file.exists(),
        "Data points Parquet file should exist"
    );

    // Test 4: Read data points from Parquet file
    let retrieved_points = storage.read_data_points(series_id, None, None).await?;
    assert_eq!(retrieved_points.len(), 2, "Should retrieve 2 data points");

    // Test 5: Read data points with date filtering
    let filtered_points = storage
        .read_data_points(
            series_id,
            Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2023, 1, 31).unwrap()),
        )
        .await?;
    assert_eq!(
        filtered_points.len(),
        1,
        "Should retrieve 1 filtered data point"
    );

    // Test 6: List all series
    let all_series = storage.list_series().await?;
    assert_eq!(all_series.len(), 1, "Should list 1 series");
    assert_eq!(all_series[0].id, series_id);

    println!("✅ Parquet file operations test passed");
    println!("   - Series written to: {:?}", series_file);
    println!("   - Data points written to: {:?}", data_points_file);
    println!("   - Series retrieved successfully");
    println!("   - Data points retrieved successfully");
    println!("   - Date filtering works correctly");
    println!("   - List series works correctly");

    Ok(())
}

#[tokio::test]
async fn test_arrow_schema_creation() -> Result<()> {
    // Test that we can create Arrow schemas
    let series_schema = ParquetStorage::series_arrow_schema();
    let data_points_schema = ParquetStorage::data_points_arrow_schema();

    assert!(
        !series_schema.fields().is_empty(),
        "Series schema should have fields"
    );
    assert!(
        !data_points_schema.fields().is_empty(),
        "Data points schema should have fields"
    );

    println!("✅ Arrow schema creation test passed");
    println!(
        "   - Series schema has {} fields",
        series_schema.fields().len()
    );
    println!(
        "   - Data points schema has {} fields",
        data_points_schema.fields().len()
    );

    Ok(())
}
