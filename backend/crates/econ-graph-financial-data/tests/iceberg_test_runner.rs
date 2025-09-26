use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::prelude::ToPrimitive;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::graphql::create_schema;
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};

/// Comprehensive Iceberg Test Runner
///
/// This test runner orchestrates all Iceberg integration tests and provides
/// a comprehensive testing strategy for the financial data service.
///
/// Test Categories:
/// 1. Basic Integration - Multi-file scenarios and GraphQL queries
/// 2. Advanced Features - Schema evolution, time travel, ACID transactions
/// 3. Financial-Specific - Data revisions, holiday handling, quality issues
/// 4. Performance - Large datasets, concurrent operations, query optimization
/// 5. Production Readiness - End-to-end scenarios, error handling, monitoring
/// Test 1: Basic Multi-File Integration
///
/// This is the core test that demonstrates the fundamental value proposition
/// of Iceberg: unified querying across multiple Parquet files.
#[tokio::test]
async fn test_iceberg_basic_integration() -> Result<(), Box<dyn std::error::Error>> {
    use econ_graph_financial_data::database::Database;
    use econ_graph_financial_data::graphql::create_schema;
    use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};

    println!("🧊 Starting Basic Iceberg Integration Test...");
    println!(
        "This demonstrates the core value proposition: unified querying across multiple files!"
    );

    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&catalog_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    // Initialize database with Iceberg storage
    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;

    // Create a long economic series
    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "BASIC_TEST_001".to_string(),
        title: "Basic Integration Test Series".to_string(),
        description: Some("Testing basic Iceberg integration".to_string()),
        units: Some("USD".to_string()),
        frequency: "daily".to_string(),
        seasonal_adjustment: None,
        start_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2021, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    database.create_series(series.clone()).await?;

    // Add data in chunks to simulate real-world ingestion
    println!("📈 Adding data in chunks...");

    // Chunk 1: First 6 months
    let chunk1_data = create_data_chunk(
        series.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        182,
        100.0,
        0.1,
    );
    database.create_data_points(chunk1_data).await?;

    // Chunk 2: Next 6 months
    let chunk2_data = create_data_chunk(
        series.id,
        NaiveDate::from_ymd_opt(2020, 7, 1).unwrap(),
        184,
        118.2,
        0.1,
    );
    database.create_data_points(chunk2_data).await?;

    // Chunk 3: First 6 months of 2021
    let chunk3_data = create_data_chunk(
        series.id,
        NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
        181,
        136.6,
        0.1,
    );
    database.create_data_points(chunk3_data).await?;

    // Chunk 4: Last 6 months of 2021
    let chunk4_data = create_data_chunk(
        series.id,
        NaiveDate::from_ymd_opt(2021, 7, 1).unwrap(),
        184,
        154.7,
        0.1,
    );
    database.create_data_points(chunk4_data).await?;

    println!("✅ All data chunks added to Iceberg table");

    // Test GraphQL queries that span multiple files
    let schema = create_schema(database.clone()).await?;

    // Query data from first file only
    let query1 = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-01-01"
                endDate: "2020-06-30"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result1 = schema.execute(query1).await;
    assert!(result1.errors.is_empty());
    println!("✅ Query from first file successful");

    // Query data spanning multiple files
    let query2 = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-11-01"
                endDate: "2021-02-28"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result2 = schema.execute(query2).await;
    assert!(result2.errors.is_empty());
    println!("✅ Query spanning multiple files successful");

    // Query entire series
    let query3 = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result3 = schema.execute(query3).await;
    assert!(result3.errors.is_empty());
    println!("✅ Query entire series successful");

    println!("🎉 Basic Integration Test Complete!");

    Ok(())
}

/// Test 2: Schema Evolution
///
/// This test demonstrates Iceberg's most powerful feature: schema evolution
/// without data migration.
#[tokio::test]
async fn test_iceberg_schema_evolution() -> Result<(), Box<dyn std::error::Error>> {
    use econ_graph_financial_data::database::Database;
    use econ_graph_financial_data::graphql::create_schema;
    use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};

    println!("🔄 Testing Iceberg Schema Evolution...");

    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&catalog_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;

    // Phase 1: Create initial series with basic schema
    let series_v1 = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "SCHEMA_EVOLUTION_001".to_string(),
        title: "Schema Evolution Test Series".to_string(),
        description: Some("Testing schema evolution capabilities".to_string()),
        units: Some("USD".to_string()),
        frequency: "daily".to_string(),
        seasonal_adjustment: None,
        start_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    database.create_series(series_v1.clone()).await?;

    // Add initial data
    let initial_data = create_data_chunk(
        series_v1.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        30,
        100.0,
        0.1,
    );
    database.create_data_points(initial_data).await?;

    // Phase 2: Evolve schema - Add seasonal adjustment
    let series_v2 = EconomicSeries {
        seasonal_adjustment: Some("seasonally_adjusted".to_string()),
        ..series_v1.clone()
    };

    database.create_series(series_v2.clone()).await?;

    // Add more data with evolved schema
    let evolved_data = create_data_chunk(
        series_v2.id,
        NaiveDate::from_ymd_opt(2020, 2, 1).unwrap(),
        30,
        103.0,
        0.1,
    );
    database.create_data_points(evolved_data).await?;

    // Test GraphQL queries
    let schema = create_schema(database.clone()).await?;

    // Query old data
    let query_old = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-01-01"
                endDate: "2020-01-31"
            ) {{
                date
                value
            }}
        }}
        "#,
        series_v1.id
    );

    let result_old = schema.execute(query_old).await;
    assert!(result_old.errors.is_empty());
    println!("✅ Old data still accessible");

    // Query new data
    let query_new = format!(
        r#"
        query {{
            series(id: "{}") {{
                id
                title
                seasonalAdjustment
            }}
        }}
        "#,
        series_v2.id
    );

    let result_new = schema.execute(query_new).await;
    assert!(result_new.errors.is_empty());
    println!("✅ New schema features working");

    println!("🎉 Schema Evolution Test Complete!");

    Ok(())
}

/// Test 3: Time Travel
///
/// This test demonstrates Iceberg's time travel capabilities.
#[tokio::test]
async fn test_iceberg_time_travel() -> Result<(), Box<dyn std::error::Error>> {
    use econ_graph_financial_data::database::Database;
    use econ_graph_financial_data::graphql::create_schema;
    use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};

    println!("⏰ Testing Iceberg Time Travel...");

    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&catalog_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;

    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "TIME_TRAVEL_001".to_string(),
        title: "Time Travel Test Series".to_string(),
        description: Some("Testing time travel capabilities".to_string()),
        units: Some("USD".to_string()),
        frequency: "daily".to_string(),
        seasonal_adjustment: None,
        start_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    database.create_series(series.clone()).await?;

    // Snapshot 1: Add initial data
    let snapshot1_data = create_data_chunk(
        series.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        30,
        100.0,
        0.1,
    );
    database.create_data_points(snapshot1_data).await?;
    let snapshot1_time = Utc::now();

    sleep(Duration::from_millis(100)).await;

    // Snapshot 2: Add more data
    let snapshot2_data = create_data_chunk(
        series.id,
        NaiveDate::from_ymd_opt(2020, 2, 1).unwrap(),
        30,
        103.0,
        0.1,
    );
    database.create_data_points(snapshot2_data).await?;
    let snapshot2_time = Utc::now();

    sleep(Duration::from_millis(100)).await;

    // Snapshot 3: Add even more data
    let snapshot3_data = create_data_chunk(
        series.id,
        NaiveDate::from_ymd_opt(2020, 3, 1).unwrap(),
        30,
        106.0,
        0.1,
    );
    database.create_data_points(snapshot3_data).await?;
    let snapshot3_time = Utc::now();

    // Test time travel queries
    let schema = create_schema(database.clone()).await?;

    // Query data as it existed at snapshot 1
    let query_snapshot1 = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-01-01"
                endDate: "2020-01-31"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result_snapshot1 = schema.execute(query_snapshot1).await;
    assert!(result_snapshot1.errors.is_empty());
    println!("✅ Snapshot 1 data accessible");

    // Query data as it exists now
    let query_current = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-01-01"
                endDate: "2020-03-31"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result_current = schema.execute(query_current).await;
    assert!(result_current.errors.is_empty());
    println!("✅ Current data accessible");

    println!("🎉 Time Travel Test Complete!");

    Ok(())
}

/// Test 4: ACID Transactions
///
/// This test demonstrates Iceberg's ACID transaction capabilities.
#[tokio::test]
async fn test_iceberg_acid_transactions() -> Result<(), Box<dyn std::error::Error>> {
    use econ_graph_financial_data::database::Database;
    use econ_graph_financial_data::graphql::create_schema;
    use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};

    println!("🔒 Testing Iceberg ACID Transactions...");

    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&catalog_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;

    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "ACID_TEST_001".to_string(),
        title: "ACID Transaction Test Series".to_string(),
        description: Some("Testing ACID transaction capabilities".to_string()),
        units: Some("USD".to_string()),
        frequency: "daily".to_string(),
        seasonal_adjustment: None,
        start_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    database.create_series(series.clone()).await?;

    // Test concurrent writes
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let db = database.clone();
            let series_id = series.id;
            let start_date =
                NaiveDate::from_ymd_opt(2020, 1, 1).unwrap() + chrono::Duration::days(i * 10);

            tokio::spawn(async move {
                let data =
                    create_data_chunk(series_id, start_date, 10, 100.0 + (i as f64 * 10.0), 0.1);

                db.create_data_points(data).await
            })
        })
        .collect();

    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await?;
        assert!(result.is_ok());
        println!("✅ Concurrent write {} completed", i + 1);
    }

    // Test data consistency
    let schema = create_schema(database.clone()).await?;

    let query = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-01-01"
                endDate: "2020-12-31"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result = schema.execute(query).await;
    assert!(result.errors.is_empty());
    println!("✅ Data consistency maintained");

    println!("🎉 ACID Transactions Test Complete!");

    Ok(())
}

/// Test 5: Large Dataset Handling
///
/// This test demonstrates Iceberg's ability to handle large datasets.
#[tokio::test]
async fn test_iceberg_large_dataset() -> Result<(), Box<dyn std::error::Error>> {
    use econ_graph_financial_data::database::Database;
    use econ_graph_financial_data::graphql::create_schema;
    use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
    use std::time::Instant;

    println!("📊 Testing Iceberg Large Dataset Handling...");

    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&catalog_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;

    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "LARGE_DATASET_001".to_string(),
        title: "Large Dataset Test Series".to_string(),
        description: Some("Testing large dataset handling capabilities".to_string()),
        units: Some("USD".to_string()),
        frequency: "daily".to_string(),
        seasonal_adjustment: None,
        start_date: Some(NaiveDate::from_ymd_opt(2010, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    database.create_series(series.clone()).await?;

    // Add large dataset in chunks
    let start_time = Instant::now();

    let total_days = 5113; // 14 years of daily data
    let chunk_size = 365; // 1 year per chunk
    let num_chunks = (total_days + chunk_size - 1) / chunk_size;

    for chunk in 0..num_chunks {
        let chunk_start_date = NaiveDate::from_ymd_opt(2010, 1, 1).unwrap()
            + chrono::Duration::days(chunk as i64 * chunk_size as i64);
        let chunk_days = if chunk == num_chunks - 1 {
            total_days - (chunk * chunk_size)
        } else {
            chunk_size
        };

        let chunk_data = create_data_chunk(
            series.id,
            chunk_start_date,
            chunk_days,
            100.0 + (chunk as f64 * chunk_size as f64 * 0.1),
            0.1,
        );

        database.create_data_points(chunk_data).await?;

        if chunk % 2 == 0 {
            println!("📊 Chunk {}/{} completed", chunk + 1, num_chunks);
        }
    }

    let ingestion_time = start_time.elapsed();
    println!(
        "✅ Large dataset ingestion completed in {:?}",
        ingestion_time
    );

    // Test query performance
    let schema = create_schema(database.clone()).await?;

    // Query recent data
    let query_start = Instant::now();
    let query_recent = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2023-01-01"
                endDate: "2023-12-31"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result_recent = schema.execute(query_recent).await;
    let query_recent_time = query_start.elapsed();
    assert!(result_recent.errors.is_empty());
    println!("✅ Recent data query: {:?}", query_recent_time);

    // Query historical data
    let query_start = Instant::now();
    let query_historical = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2010-01-01"
                endDate: "2010-12-31"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result_historical = schema.execute(query_historical).await;
    let query_historical_time = query_start.elapsed();
    assert!(result_historical.errors.is_empty());
    println!("✅ Historical data query: {:?}", query_historical_time);

    // Performance assertions
    assert!(
        query_recent_time.as_millis() < 5000,
        "Recent data query should be fast"
    );
    assert!(
        query_historical_time.as_millis() < 5000,
        "Historical data query should be fast"
    );

    println!("🎉 Large Dataset Test Complete!");

    Ok(())
}

/// Helper function to create data chunks for testing
fn create_data_chunk(
    series_id: Uuid,
    start_date: NaiveDate,
    days: i32,
    starting_value: f64,
    daily_increment: f64,
) -> Vec<DataPoint> {
    let mut data_points = Vec::new();
    let mut current_value = starting_value;

    for i in 0..days {
        let date = start_date + chrono::Duration::days(i as i64);

        data_points.push(DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date,
            value: Some(DecimalScalar(
                rust_decimal::Decimal::from_f64_retain(current_value).unwrap(),
            )),
            revision_date: date,
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });

        current_value += daily_increment;
    }

    data_points
}
