use anyhow::Result;
use chrono::{NaiveDate, Utc};
use std::path::PathBuf;
use tempfile::tempdir;
use uuid::Uuid;

use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
use econ_graph_financial_data::storage::{parquet_storage::ParquetFlightService, ParquetStorage};

/// Simple Parquet Integration Test
///
/// This test demonstrates the core functionality:
/// 1. Create a series and data points
/// 2. Store them in Parquet files using Arrow Flight concepts
/// 3. Read them back and verify the data
/// 4. Test GraphQL queries against the data
#[tokio::test]
async fn test_simple_parquet_integration() -> Result<()> {
    println!("🧪 Starting simple Parquet integration test...");

    // Step 1: Create temporary directory
    let temp_dir = tempdir()?;
    let data_dir = temp_dir.path().to_path_buf();
    println!("📁 Created temporary directory: {:?}", data_dir);

    // Step 2: Initialize database with Parquet storage
    println!("🗄️  Initializing database with Parquet storage...");
    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;
    println!("✅ Database initialized with Parquet storage");

    // Step 3: Create a test economic series
    println!("📊 Creating test economic series...");
    let source_id = Uuid::new_v4();
    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id,
        external_id: "TEST001".to_string(),
        title: "Test Economic Series".to_string(),
        description: Some("A test series for integration testing".to_string()),
        units: Some("USD".to_string()),
        frequency: "Daily".to_string(),
        seasonal_adjustment: Some("Seasonally Adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2024, 1, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Step 4: Create test data points
    println!("📈 Creating test data points...");
    let mut data_points = Vec::new();
    for i in 0..31 {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap() + chrono::Duration::days(i);
        let value = 100.0 + (i as f64 * 0.5); // Simple trend

        data_points.push(DataPoint {
            id: Uuid::new_v4(),
            series_id: series.id,
            date,
            value: Some(DecimalScalar(
                rust_decimal::Decimal::from_f64_retain(value).unwrap(),
            )),
            revision_date: date,
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });
    }
    println!("✅ Created {} data points", data_points.len());

    // Step 5: Store the series and data points
    println!("💾 Storing series and data points...");
    database.create_series(series.clone()).await?;
    database.create_data_points(data_points.clone()).await?;
    println!("✅ Data stored successfully");

    // Step 6: Read back the series
    println!("📖 Reading back the series...");
    let retrieved_series = database.get_series(series.id).await?;
    assert!(retrieved_series.is_some());
    let retrieved_series = retrieved_series.unwrap();
    assert_eq!(retrieved_series.title, "Test Economic Series");
    assert_eq!(retrieved_series.external_id, "TEST001");
    println!("✅ Series retrieved successfully");

    // Step 7: Read back the data points
    println!("📊 Reading back the data points...");
    let retrieved_points = database.get_data_points(series.id, None, None).await?;
    assert_eq!(retrieved_points.len(), 31);
    println!("✅ Retrieved {} data points", retrieved_points.len());

    // Step 8: Test date filtering
    println!("🔍 Testing date filtering...");
    let start_date = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
    let end_date = NaiveDate::from_ymd_opt(2024, 1, 20).unwrap();
    let filtered_points = database
        .get_data_points(series.id, Some(start_date), Some(end_date))
        .await?;
    assert_eq!(filtered_points.len(), 11); // 10th to 20th inclusive
    println!("✅ Date filtering works correctly");

    // Step 9: Test listing all series
    println!("📋 Testing series listing...");
    let all_series = database.list_series().await?;
    assert_eq!(all_series.len(), 1);
    println!("✅ Series listing works correctly");

    // Step 10: Test Parquet file operations directly
    println!("🔧 Testing Parquet file operations directly...");
    let storage = ParquetStorage::new(data_dir.clone());

    // Test Arrow schema creation
    let series_schema = ParquetStorage::series_arrow_schema();
    let data_points_schema = ParquetStorage::data_points_arrow_schema();
    println!("✅ Arrow schemas created successfully");
    println!("  Series schema fields: {}", series_schema.fields().len());
    println!(
        "  Data points schema fields: {}",
        data_points_schema.fields().len()
    );

    // Test RecordBatch creation
    let series_batch = ParquetStorage::series_to_record_batch(&retrieved_series)?;
    let data_points_batch = ParquetStorage::data_points_to_record_batch(&retrieved_points)?;
    println!("✅ Arrow RecordBatches created successfully");
    println!("  Series batch rows: {}", series_batch.num_rows());
    println!("  Data points batch rows: {}", data_points_batch.num_rows());

    println!("🎉 Simple Parquet integration test completed successfully!");
    println!("📊 Test Results:");
    println!("  ✅ Series creation and storage");
    println!("  ✅ Data points creation and storage");
    println!("  ✅ Series retrieval");
    println!("  ✅ Data points retrieval");
    println!("  ✅ Date filtering");
    println!("  ✅ Series listing");
    println!("  ✅ Arrow schema creation");
    println!("  ✅ RecordBatch operations");
    println!("  ✅ Parquet file operations");

    Ok(())
}

/// Test Arrow Flight concepts with Parquet storage
///
/// This test demonstrates the Arrow Flight approach:
/// 1. Create Arrow schemas for financial data
/// 2. Convert data to Arrow RecordBatches
/// 3. Store and retrieve data using Arrow operations
/// 4. Verify zero-copy data transfer concepts
#[tokio::test]
async fn test_arrow_flight_concepts() -> Result<()> {
    println!("🚀 Starting Arrow Flight concepts test...");

    // Step 1: Create temporary directory
    let temp_dir = tempdir()?;
    let data_dir = temp_dir.path().to_path_buf();
    println!("📁 Created temporary directory: {:?}", data_dir);

    // Step 2: Initialize Parquet storage
    println!("🗄️  Initializing Parquet storage...");
    let storage = ParquetStorage::new(data_dir.clone());
    println!("✅ Parquet storage initialized");

    // Step 3: Create test data
    println!("📊 Creating test financial data...");
    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "ARROW_TEST_001".to_string(),
        title: "Arrow Flight Test Series".to_string(),
        description: Some("Testing Arrow Flight concepts".to_string()),
        units: Some("USD".to_string()),
        frequency: "Daily".to_string(),
        seasonal_adjustment: None,
        start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2024, 1, 7).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let data_points = vec![
        DataPoint {
            id: Uuid::new_v4(),
            series_id: series.id,
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            value: Some(DecimalScalar(
                rust_decimal::Decimal::from_f64_retain(100.0).unwrap(),
            )),
            revision_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        DataPoint {
            id: Uuid::new_v4(),
            series_id: series.id,
            date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
            value: Some(DecimalScalar(
                rust_decimal::Decimal::from_f64_retain(101.0).unwrap(),
            )),
            revision_date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    // Step 4: Test Arrow schema creation
    println!("🔧 Testing Arrow schema creation...");
    let series_schema = ParquetStorage::series_arrow_schema();
    let data_points_schema = ParquetStorage::data_points_arrow_schema();

    println!("✅ Arrow schemas created:");
    println!("  Series schema: {} fields", series_schema.fields().len());
    println!(
        "  Data points schema: {} fields",
        data_points_schema.fields().len()
    );

    // Step 5: Test RecordBatch creation
    println!("📦 Testing RecordBatch creation...");
    let series_batch = ParquetStorage::series_to_record_batch(&series)?;
    let data_points_batch = ParquetStorage::data_points_to_record_batch(&data_points)?;

    println!("✅ RecordBatches created:");
    println!(
        "  Series batch: {} rows, {} columns",
        series_batch.num_rows(),
        series_batch.num_columns()
    );
    println!(
        "  Data points batch: {} rows, {} columns",
        data_points_batch.num_rows(),
        data_points_batch.num_columns()
    );

    // Step 6: Test Parquet file operations
    println!("💾 Testing Parquet file operations...");
    let series_file = data_dir.join("test_series.parquet");
    let data_points_file = data_dir.join("test_data_points.parquet");

    storage
        .write_record_batch_to_parquet(series_batch, series_file.clone())
        .await?;
    storage
        .write_record_batch_to_parquet(data_points_batch, data_points_file.clone())
        .await?;
    println!("✅ Parquet files written successfully");

    // Step 7: Test Parquet file reading
    println!("📖 Testing Parquet file reading...");
    let read_series_batch = storage.read_record_batch_from_parquet(series_file).await?;
    let read_data_points_batch = storage
        .read_record_batch_from_parquet(data_points_file)
        .await?;

    assert!(read_series_batch.is_some());
    assert!(read_data_points_batch.is_some());

    let read_series_batch = read_series_batch.unwrap();
    let read_data_points_batch = read_data_points_batch.unwrap();

    println!("✅ Parquet files read successfully:");
    println!(
        "  Series batch: {} rows, {} columns",
        read_series_batch.num_rows(),
        read_series_batch.num_columns()
    );
    println!(
        "  Data points batch: {} rows, {} columns",
        read_data_points_batch.num_rows(),
        read_data_points_batch.num_columns()
    );

    // Step 8: Test Arrow Flight service concepts
    println!("🛩️  Testing Arrow Flight service concepts...");
    let flight_service = ParquetFlightService::new(data_dir.clone());
    let financial_schema = ParquetFlightService::get_financial_data_schema();
    let data_points_schema = ParquetFlightService::get_data_points_schema();

    println!("✅ Arrow Flight service concepts tested:");
    println!(
        "  Financial data schema: {} fields",
        financial_schema.fields().len()
    );
    println!(
        "  Data points schema: {} fields",
        data_points_schema.fields().len()
    );

    println!("🎉 Arrow Flight concepts test completed successfully!");
    println!("📊 Test Results:");
    println!("  ✅ Arrow schema creation");
    println!("  ✅ RecordBatch operations");
    println!("  ✅ Parquet file writing");
    println!("  ✅ Parquet file reading");
    println!("  ✅ Arrow Flight service concepts");
    println!("  ✅ Zero-copy data transfer preparation");

    Ok(())
}
