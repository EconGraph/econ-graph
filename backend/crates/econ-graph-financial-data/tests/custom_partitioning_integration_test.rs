use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
use rust_decimal::Decimal;
use std::fs;
use tempfile::TempDir;
use uuid::Uuid;

/// Test custom partitioning with real financial data scenarios
#[tokio::test]
async fn test_custom_partitioning_basic_operations() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    // Initialize database with custom partitioning
    let db = Database::new_with_custom_partitioning(&data_dir).await?;

    // Create a test economic series
    let series_id = Uuid::new_v4();
    let series = EconomicSeries {
        id: series_id,
        source_id: Uuid::new_v4(),
        external_id: "TEST_SERIES_001".to_string(),
        title: "Test GDP Series".to_string(),
        description: Some("Test series for custom partitioning".to_string()),
        units: Some("Billions of USD".to_string()),
        frequency: "quarterly".to_string(),
        seasonal_adjustment: Some("seasonally_adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Write series metadata
    db.create_series(series.clone()).await?;

    // Create data points across multiple partitions (different days)
    let mut data_points = Vec::new();
    let base_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();

    for i in 0..30 {
        let date = base_date + Duration::days(i);
        let point = DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date,
            value: Some(DecimalScalar(Decimal::from(1000 + i * 10))),
            revision_date: date,
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        data_points.push(point);
    }

    // Write data points (should create multiple partition files)
    db.create_data_points(data_points.clone()).await?;

    // Verify directory structure was created
    let data_path = temp_dir.path();
    assert!(data_path.exists());

    // Check that multiple partition directories were created
    let partition_dirs: Vec<_> = fs::read_dir(data_path)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().is_dir() && entry.file_name().to_string_lossy().starts_with("year=")
        })
        .collect();

    assert!(
        !partition_dirs.is_empty(),
        "Should have created partition directories"
    );
    println!("Created {} partition directories", partition_dirs.len());

    // Test reading all data points
    let retrieved_points = db.get_data_points(series_id, None, None).await?;
    assert_eq!(retrieved_points.len(), 30);

    // Test reading data points for a specific date range
    let start_date = NaiveDate::from_ymd_opt(2024, 1, 5).unwrap();
    let end_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    let range_points = db
        .get_data_points(series_id, Some(start_date), Some(end_date))
        .await?;
    assert_eq!(range_points.len(), 11); // 5th through 15th inclusive

    // Verify data integrity
    for point in &range_points {
        assert!(point.date >= start_date);
        assert!(point.date <= end_date);
        assert_eq!(point.series_id, series_id);
    }

    // Test reading series metadata
    let retrieved_series = db.get_series(series_id).await?;
    assert!(retrieved_series.is_some());
    let retrieved_series = retrieved_series.unwrap();
    assert_eq!(retrieved_series.external_id, "TEST_SERIES_001");
    assert_eq!(retrieved_series.title, "Test GDP Series");

    println!("✅ Custom partitioning basic operations test passed");
    Ok(())
}

/// Test partition file organization and discovery
#[tokio::test]
async fn test_partition_file_organization() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    let db = Database::new_with_custom_partitioning(&data_dir).await?;

    // Create multiple series with data in different partitions
    let series1_id = Uuid::new_v4();
    let series2_id = Uuid::new_v4();

    // Series 1: January data
    let jan_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    let series1_point = DataPoint {
        id: Uuid::new_v4(),
        series_id: series1_id,
        date: jan_date,
        value: Some(DecimalScalar(Decimal::from(1000))),
        revision_date: jan_date,
        is_original_release: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Series 2: February data
    let feb_date = NaiveDate::from_ymd_opt(2024, 2, 15).unwrap();
    let series2_point = DataPoint {
        id: Uuid::new_v4(),
        series_id: series2_id,
        date: feb_date,
        value: Some(DecimalScalar(Decimal::from(2000))),
        revision_date: feb_date,
        is_original_release: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Write the data
    db.create_data_points(vec![series1_point]).await?;
    db.create_data_points(vec![series2_point]).await?;

    // Verify file organization
    let data_path = temp_dir.path();

    // Check January partition
    let jan_partition = data_path.join("year=2024/month=01/day=15");
    assert!(jan_partition.exists());
    let jan_files: Vec<_> = fs::read_dir(jan_partition)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".parquet"))
        .collect();
    assert_eq!(jan_files.len(), 1);

    // Check February partition
    let feb_partition = data_path.join("year=2024/month=02/day=15");
    assert!(feb_partition.exists());
    let feb_files: Vec<_> = fs::read_dir(feb_partition)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".parquet"))
        .collect();
    assert_eq!(feb_files.len(), 1);

    // Test that we can read from specific partitions
    let jan_data = db
        .get_data_points(series1_id, Some(jan_date), Some(jan_date))
        .await?;
    assert_eq!(jan_data.len(), 1);
    assert_eq!(jan_data[0].value.as_ref().unwrap().0, Decimal::from(1000));

    let feb_data = db
        .get_data_points(series2_id, Some(feb_date), Some(feb_date))
        .await?;
    assert_eq!(feb_data.len(), 1);
    assert_eq!(feb_data[0].value.as_ref().unwrap().0, Decimal::from(2000));

    println!("✅ Partition file organization test passed");
    Ok(())
}

/// Test zero-copy performance characteristics
#[tokio::test]
async fn test_zero_copy_performance() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    let db = Database::new_with_custom_partitioning(&data_dir).await?;

    let series_id = Uuid::new_v4();

    // Create a large dataset
    let mut data_points = Vec::new();
    let base_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();

    for i in 0..1000 {
        let date = base_date + Duration::days(i % 30); // Spread across first 30 days
        let point = DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date,
            value: Some(DecimalScalar(Decimal::from(1000 + i))),
            revision_date: date,
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        data_points.push(point);
    }

    // Measure write performance
    let start = std::time::Instant::now();
    db.create_data_points(data_points.clone()).await?;
    let write_duration = start.elapsed();

    // Measure read performance
    let start = std::time::Instant::now();
    let retrieved_points = db.get_data_points(series_id, None, None).await?;
    let read_duration = start.elapsed();

    assert_eq!(retrieved_points.len(), 1000);

    println!("Write performance: {:?} for 1000 points", write_duration);
    println!("Read performance: {:?} for 1000 points", read_duration);
    println!("✅ Zero-copy performance test passed");

    Ok(())
}

/// Test range queries and file discovery
#[tokio::test]
async fn test_range_queries() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    let db = Database::new_with_custom_partitioning(&data_dir).await?;

    let series_id = Uuid::new_v4();

    // Create data spanning multiple months
    let mut data_points = Vec::new();
    let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();

    // Create data for every 5th day across 3 months
    for month in 1..=3 {
        for day in (1..=31).step_by(5) {
            if let Some(date) = NaiveDate::from_ymd_opt(2024, month, day) {
                let point = DataPoint {
                    id: Uuid::new_v4(),
                    series_id,
                    date,
                    value: Some(DecimalScalar(Decimal::from(month * 1000 + day))),
                    revision_date: date,
                    is_original_release: true,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                data_points.push(point);
            }
        }
    }

    db.create_data_points(data_points).await?;

    // Test range queries that span multiple partitions
    let jan_data = db
        .get_data_points(
            series_id,
            Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2024, 1, 31).unwrap()),
        )
        .await?;

    let feb_data = db
        .get_data_points(
            series_id,
            Some(NaiveDate::from_ymd_opt(2024, 2, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2024, 2, 28).unwrap()),
        )
        .await?;

    let cross_month_data = db
        .get_data_points(
            series_id,
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            Some(NaiveDate::from_ymd_opt(2024, 2, 15).unwrap()),
        )
        .await?;

    // Verify we get data from multiple partitions
    assert!(!jan_data.is_empty());
    assert!(!feb_data.is_empty());
    assert!(!cross_month_data.is_empty());

    // Verify date ranges
    for point in &jan_data {
        assert_eq!(point.date.year(), 2024);
        assert_eq!(point.date.month(), 1);
    }

    for point in &feb_data {
        assert_eq!(point.date.year(), 2024);
        assert_eq!(point.date.month(), 2);
    }

    for point in &cross_month_data {
        assert!(point.date >= NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        assert!(point.date <= NaiveDate::from_ymd_opt(2024, 2, 15).unwrap());
    }

    println!("✅ Range queries test passed");
    Ok(())
}
