//! Performance testing examples for the Financial Data Service
//!
//! This example demonstrates how to test the performance characteristics of the service
//! including throughput, latency, and memory usage.

use anyhow::Result;
use chrono::NaiveDate;
use econ_graph_financial_data::{
    database::Database,
    models::input::{CreateDataPointInput, CreateEconomicSeriesInput},
    models::{DataPoint, DecimalScalar, EconomicSeries},
    storage::{FinancialDataStorage, ParquetStorage},
};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::time::Instant;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Financial Data Service - Performance Testing Examples");
    println!("======================================================\n");

    // Example 1: Single operation performance
    println!("⚡ Example 1: Single Operation Performance");
    example_single_operations().await?;

    // Example 2: Concurrent operations
    println!("\n🔄 Example 2: Concurrent Operations");
    example_concurrent_operations().await?;

    // Example 3: Bulk data operations
    println!("\n📊 Example 3: Bulk Data Operations");
    example_bulk_operations().await?;

    // Example 4: Memory usage analysis
    println!("\n💾 Example 4: Memory Usage Analysis");
    example_memory_usage().await?;

    // Example 5: Parquet performance
    println!("\n🗄️ Example 5: Parquet Storage Performance");
    example_parquet_performance().await?;

    println!("\n✅ All performance tests completed successfully!");
    Ok(())
}

async fn example_single_operations() -> Result<()> {
    let database = Database::new_in_memory().await?;
    let source_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();

    // Test series creation performance
    let start = Instant::now();
    let series_input = CreateEconomicSeriesInput {
        source_id,
        external_id: "PERF_TEST".to_string(),
        title: "Performance Test Series".to_string(),
        description: Some("Series for performance testing".to_string()),
        units: Some("Units".to_string()),
        frequency: "DAILY".to_string(),
        seasonal_adjustment: Some("SA".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
    };

    let series = econ_graph_financial_data::models::EconomicSeries {
        id: series_id,
        source_id: series_input.source_id,
        external_id: series_input.external_id,
        title: series_input.title,
        description: series_input.description,
        units: series_input.units,
        frequency: series_input.frequency,
        seasonal_adjustment: series_input.seasonal_adjustment,
        start_date: series_input.start_date,
        end_date: series_input.end_date,
        is_active: series_input.is_active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    database.create_series(series).await?;
    let series_creation_time = start.elapsed();
    println!("✅ Series creation: {:?}", series_creation_time);

    // Test data point creation performance
    let start = Instant::now();
    let data_points = create_test_data_points(series_id, 1000);
    let data_points_input: Vec<CreateDataPointInput> = data_points
        .iter()
        .map(|dp| CreateDataPointInput {
            series_id: dp.series_id,
            date: dp.date,
            value: dp.value.clone(),
            revision_date: dp.revision_date,
            is_original_release: dp.is_original_release,
        })
        .collect();

    let data_points_models: Vec<DataPoint> = data_points_input
        .into_iter()
        .map(|input| DataPoint {
            id: Uuid::new_v4(),
            series_id: input.series_id,
            date: input.date,
            value: input.value,
            revision_date: input.revision_date,
            is_original_release: input.is_original_release,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .collect();
    database.create_data_points(data_points_models).await?;
    let data_points_creation_time = start.elapsed();
    println!(
        "✅ 1000 data points creation: {:?}",
        data_points_creation_time
    );

    // Test series retrieval performance
    let start = Instant::now();
    let retrieved_series = database.get_series(series_id).await?;
    let series_retrieval_time = start.elapsed();
    println!("✅ Series retrieval: {:?}", series_retrieval_time);
    assert!(retrieved_series.is_some());

    // Test data points retrieval performance
    let start = Instant::now();
    let retrieved_points = database.get_data_points(series_id, None, None).await?;
    let data_points_retrieval_time = start.elapsed();
    println!(
        "✅ 1000 data points retrieval: {:?}",
        data_points_retrieval_time
    );
    assert_eq!(retrieved_points.len(), 1000);

    // Test date range query performance
    let start = Instant::now();
    let start_date = NaiveDate::from_ymd_opt(2023, 6, 1).unwrap();
    let end_date = NaiveDate::from_ymd_opt(2023, 6, 30).unwrap();
    let filtered_points = database
        .get_data_points(series_id, Some(start_date), Some(end_date))
        .await?;
    let date_range_query_time = start.elapsed();
    println!("✅ Date range query (30 days): {:?}", date_range_query_time);
    println!("   Found {} points in date range", filtered_points.len());

    Ok(())
}

async fn example_concurrent_operations() -> Result<()> {
    let database = Database::new_in_memory().await?;
    let concurrency_levels = [1, 5, 10, 20];

    for concurrency in concurrency_levels {
        println!("Testing {} concurrent operations...", concurrency);

        // Test concurrent writes
        let start = Instant::now();
        let handles: Vec<_> = (0..concurrency)
            .map(|i| {
                let database = database.clone();
                let series_id = Uuid::new_v4();
                let data_points = create_test_data_points(series_id, 100);
                tokio::spawn(async move { database.create_data_points(data_points).await })
            })
            .collect();

        for handle in handles {
            handle.await??;
        }
        let concurrent_write_time = start.elapsed();
        println!(
            "✅ {} concurrent writes (100 points each): {:?}",
            concurrency, concurrent_write_time
        );

        // Test concurrent reads
        let start = Instant::now();
        let handles: Vec<_> = (0..concurrency)
            .map(|_| {
                let database = database.clone();
                let series_id = Uuid::new_v4();
                tokio::spawn(async move { database.get_series(series_id).await })
            })
            .collect();

        for handle in handles {
            handle.await??;
        }
        let concurrent_read_time = start.elapsed();
        println!(
            "✅ {} concurrent reads: {:?}",
            concurrency, concurrent_read_time
        );
    }

    Ok(())
}

async fn example_bulk_operations() -> Result<()> {
    let database = Database::new_in_memory().await?;
    let source_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();

    // Create a series first
    let series_input = CreateEconomicSeriesInput {
        source_id,
        external_id: "BULK_TEST".to_string(),
        title: "Bulk Test Series".to_string(),
        description: Some("Series for bulk operations testing".to_string()),
        units: Some("Units".to_string()),
        frequency: "DAILY".to_string(),
        seasonal_adjustment: Some("SA".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
    };

    let series = econ_graph_financial_data::models::EconomicSeries {
        id: series_id,
        source_id: series_input.source_id,
        external_id: series_input.external_id,
        title: series_input.title,
        description: series_input.description,
        units: series_input.units,
        frequency: series_input.frequency,
        seasonal_adjustment: series_input.seasonal_adjustment,
        start_date: series_input.start_date,
        end_date: series_input.end_date,
        is_active: series_input.is_active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    database.create_series(series).await?;

    // Test different batch sizes
    let batch_sizes = [100, 1000, 10000, 50000];

    for batch_size in batch_sizes {
        println!("Testing batch size: {}", batch_size);

        let start = Instant::now();
        let data_points = create_test_data_points(series_id, batch_size);
        let data_points_input: Vec<CreateDataPointInput> = data_points
            .iter()
            .map(|dp| CreateDataPointInput {
                series_id: dp.series_id,
                date: dp.date,
                value: dp.value.clone(),
                revision_date: dp.revision_date,
                is_original_release: dp.is_original_release,
            })
            .collect();

        let data_points_models: Vec<DataPoint> = data_points_input
            .into_iter()
            .map(|input| DataPoint {
                id: Uuid::new_v4(),
                series_id: input.series_id,
                date: input.date,
                value: input.value,
                revision_date: input.revision_date,
                is_original_release: input.is_original_release,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .collect();
        database.create_data_points(data_points_models).await?;
        let bulk_write_time = start.elapsed();

        let throughput = batch_size as f64 / bulk_write_time.as_secs_f64();
        println!(
            "✅ {} data points: {:?} ({:.0} points/sec)",
            batch_size, bulk_write_time, throughput
        );

        // Test bulk read
        let start = Instant::now();
        let retrieved_points = database.get_data_points(series_id, None, None).await?;
        let bulk_read_time = start.elapsed();

        let read_throughput = retrieved_points.len() as f64 / bulk_read_time.as_secs_f64();
        println!(
            "✅ Read {} data points: {:?} ({:.0} points/sec)",
            retrieved_points.len(),
            bulk_read_time,
            read_throughput
        );
    }

    Ok(())
}

async fn example_memory_usage() -> Result<()> {
    println!("Memory usage analysis (approximate)...");

    let database = Database::new_in_memory().await?;
    let source_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();

    // Create a series
    let series_input = CreateEconomicSeriesInput {
        source_id,
        external_id: "MEMORY_TEST".to_string(),
        title: "Memory Test Series".to_string(),
        description: Some("Series for memory usage testing".to_string()),
        units: Some("Units".to_string()),
        frequency: "DAILY".to_string(),
        seasonal_adjustment: Some("SA".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
    };

    let series = econ_graph_financial_data::models::EconomicSeries {
        id: series_id,
        source_id: series_input.source_id,
        external_id: series_input.external_id,
        title: series_input.title,
        description: series_input.description,
        units: series_input.units,
        frequency: series_input.frequency,
        seasonal_adjustment: series_input.seasonal_adjustment,
        start_date: series_input.start_date,
        end_date: series_input.end_date,
        is_active: series_input.is_active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    database.create_series(series).await?;

    // Test memory usage with different data sizes
    let data_sizes = [1000, 10000, 100000];

    for size in data_sizes {
        println!("Testing memory usage with {} data points...", size);

        let start = Instant::now();
        let data_points = create_test_data_points(series_id, size);
        let data_points_input: Vec<CreateDataPointInput> = data_points
            .iter()
            .map(|dp| CreateDataPointInput {
                series_id: dp.series_id,
                date: dp.date,
                value: dp.value.clone(),
                revision_date: dp.revision_date,
                is_original_release: dp.is_original_release,
            })
            .collect();

        let data_points_models: Vec<DataPoint> = data_points_input
            .into_iter()
            .map(|input| DataPoint {
                id: Uuid::new_v4(),
                series_id: input.series_id,
                date: input.date,
                value: input.value,
                revision_date: input.revision_date,
                is_original_release: input.is_original_release,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .collect();
        database.create_data_points(data_points_models).await?;
        let creation_time = start.elapsed();

        // Estimate memory usage (rough approximation)
        let estimated_memory_mb = (size * 200) / 1024 / 1024; // ~200 bytes per data point
        println!(
            "✅ {} data points: {:?} (estimated {} MB)",
            size, creation_time, estimated_memory_mb
        );
    }

    Ok(())
}

async fn example_parquet_performance() -> Result<()> {
    // Create a temporary directory for Parquet files
    let temp_dir = std::env::temp_dir().join("econ_graph_performance_test");
    std::fs::create_dir_all(&temp_dir)?;

    let storage = ParquetStorage::new(temp_dir.clone());
    let source_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();

    // Create a series
    let series = EconomicSeries {
        id: series_id,
        source_id,
        external_id: "PARQUET_TEST".to_string(),
        title: "Parquet Test Series".to_string(),
        description: Some("Series for Parquet performance testing".to_string()),
        units: Some("Units".to_string()),
        frequency: "DAILY".to_string(),
        seasonal_adjustment: Some("SA".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Test series write performance
    let start = Instant::now();
    storage.write_series(&series).await?;
    let series_write_time = start.elapsed();
    println!("✅ Series write to Parquet: {:?}", series_write_time);

    // Test data points write performance
    let data_sizes = [1000, 10000, 100000];

    for size in data_sizes {
        println!("Testing Parquet performance with {} data points...", size);

        let start = Instant::now();
        let data_points = create_test_data_points(series_id, size);
        storage.write_data_points(series_id, &data_points).await?;
        let write_time = start.elapsed();

        let throughput = size as f64 / write_time.as_secs_f64();
        println!(
            "✅ Write {} data points: {:?} ({:.0} points/sec)",
            size, write_time, throughput
        );

        // Test read performance
        let start = Instant::now();
        let retrieved_points = storage.read_data_points(series_id, None, None).await?;
        let read_time = start.elapsed();

        let read_throughput = retrieved_points.len() as f64 / read_time.as_secs_f64();
        println!(
            "✅ Read {} data points: {:?} ({:.0} points/sec)",
            retrieved_points.len(),
            read_time,
            read_throughput
        );
    }

    // Test Arrow schema creation performance
    let start = Instant::now();
    for _ in 0..1000 {
        let _series_schema = ParquetStorage::series_arrow_schema();
        let _data_points_schema = ParquetStorage::data_points_arrow_schema();
    }
    let schema_creation_time = start.elapsed();
    println!("✅ 1000 Arrow schema creations: {:?}", schema_creation_time);

    // Test RecordBatch conversion performance
    let start = Instant::now();
    let data_points = create_test_data_points(series_id, 10000);
    let _batch = ParquetStorage::data_points_to_record_batch(&data_points)?;
    let batch_conversion_time = start.elapsed();
    println!(
        "✅ RecordBatch conversion (10k points): {:?}",
        batch_conversion_time
    );

    // Clean up
    std::fs::remove_dir_all(&temp_dir)?;
    println!("✅ Cleaned up temporary files");

    Ok(())
}

fn create_test_data_points(series_id: Uuid, count: usize) -> Vec<DataPoint> {
    let mut data_points = Vec::with_capacity(count);
    let base_date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();

    for i in 0..count {
        let date = base_date + chrono::Duration::days(i as i64);
        let value = 1000.0 + (i as f64 * 0.1) + (i as f64 * 0.01).sin();

        data_points.push(DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date,
            value: Some(DecimalScalar(
                Decimal::from_f64_retain(value).unwrap_or_default(),
            )),
            revision_date: date + chrono::Duration::days(15),
            is_original_release: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        });
    }

    data_points
}
