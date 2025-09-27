//! Basic usage examples for the Financial Data Service
//!
//! This example demonstrates the core functionality of the service including:
//! - Creating economic series
//! - Adding data points
//! - Querying data
//! - Error handling

use anyhow::Result;
use chrono::NaiveDate;
use econ_graph_financial_data::{
    crawler::CrawlerWriteApi,
    database::Database,
    models::input::{CreateDataPointInput, CreateEconomicSeriesInput},
    models::DecimalScalar,
    storage::{FinancialDataStorage, ParquetStorage},
};
use rust_decimal::Decimal;
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Financial Data Service - Basic Usage Examples");
    println!("================================================\n");

    // Example 1: In-memory database operations
    println!("📊 Example 1: In-Memory Database Operations");
    example_in_memory_operations().await?;

    // Example 2: Parquet storage operations
    println!("\n💾 Example 2: Parquet Storage Operations");
    example_parquet_operations().await?;

    // Example 3: Crawler API usage
    println!("\n🕷️ Example 3: Crawler API Usage");
    example_crawler_api().await?;

    // Example 4: Error handling
    println!("\n⚠️ Example 4: Error Handling");
    example_error_handling().await?;

    println!("\n✅ All examples completed successfully!");
    Ok(())
}

async fn example_in_memory_operations() -> Result<()> {
    println!("Creating in-memory database...");
    let database = Database::new_in_memory().await?;

    // Create a test series
    let source_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();

    let series_input = CreateEconomicSeriesInput {
        source_id,
        external_id: "GDP_US".to_string(),
        title: "US Gross Domestic Product".to_string(),
        description: Some("Quarterly GDP data for the United States".to_string()),
        units: Some("Billions of USD".to_string()),
        frequency: "QUARTERLY".to_string(),
        seasonal_adjustment: Some("SAAR".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(1947, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
    };

    println!("Creating economic series...");
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
    database.create_series(series.clone()).await?;
    println!("✅ Created series: {} ({})", series.title, series.id);

    // Create some data points
    let data_points = vec![
        CreateDataPointInput {
            series_id,
            date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::from_str("25000.5")?)),
            revision_date: NaiveDate::from_ymd_opt(2023, 1, 15).unwrap(),
            is_original_release: true,
        },
        CreateDataPointInput {
            series_id,
            date: NaiveDate::from_ymd_opt(2023, 4, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::from_str("25100.2")?)),
            revision_date: NaiveDate::from_ymd_opt(2023, 4, 15).unwrap(),
            is_original_release: true,
        },
        CreateDataPointInput {
            series_id,
            date: NaiveDate::from_ymd_opt(2023, 7, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::from_str("25200.8")?)),
            revision_date: NaiveDate::from_ymd_opt(2023, 7, 15).unwrap(),
            is_original_release: true,
        },
    ];

    println!("Creating data points...");
    let data_points_models: Vec<econ_graph_financial_data::models::DataPoint> = data_points
        .into_iter()
        .map(|input| econ_graph_financial_data::models::DataPoint {
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
    database
        .create_data_points(data_points_models.clone())
        .await?;
    println!("✅ Created {} data points", data_points_models.len());

    // Query the series
    println!("Querying series...");
    let retrieved_series = database.get_series(series_id).await?;
    if let Some(series) = retrieved_series {
        println!("✅ Retrieved series: {} ({})", series.title, series.id);
    }

    // Query data points
    println!("Querying data points...");
    let retrieved_points = database.get_data_points(series_id, None, None).await?;
    println!("✅ Retrieved {} data points", retrieved_points.len());

    // Query with date range
    let start_date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
    let end_date = NaiveDate::from_ymd_opt(2023, 6, 30).unwrap();
    let filtered_points = database
        .get_data_points(series_id, Some(start_date), Some(end_date))
        .await?;
    println!(
        "✅ Retrieved {} data points in date range",
        filtered_points.len()
    );

    Ok(())
}

async fn example_parquet_operations() -> Result<()> {
    // Create a temporary directory for Parquet files
    let temp_dir = std::env::temp_dir().join("econ_graph_example");
    std::fs::create_dir_all(&temp_dir)?;

    println!("Creating Parquet storage...");
    let storage = ParquetStorage::new(temp_dir.clone());

    // Create a test series
    let source_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();

    let series = econ_graph_financial_data::models::EconomicSeries {
        id: series_id,
        source_id,
        external_id: "UNEMPLOYMENT_US".to_string(),
        title: "US Unemployment Rate".to_string(),
        description: Some("Monthly unemployment rate for the United States".to_string()),
        units: Some("Percent".to_string()),
        frequency: "MONTHLY".to_string(),
        seasonal_adjustment: Some("SA".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(1948, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    println!("Writing series to Parquet...");
    storage.write_series(&series).await?;
    println!("✅ Series written to Parquet");

    // Create some data points
    let data_points = vec![
        econ_graph_financial_data::models::DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::from_str("3.4")?)),
            revision_date: NaiveDate::from_ymd_opt(2023, 1, 15).unwrap(),
            is_original_release: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        econ_graph_financial_data::models::DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date: NaiveDate::from_ymd_opt(2023, 2, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::from_str("3.6")?)),
            revision_date: NaiveDate::from_ymd_opt(2023, 2, 15).unwrap(),
            is_original_release: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
    ];

    println!("Writing data points to Parquet...");
    storage.write_data_points(series_id, &data_points).await?;
    println!("✅ Data points written to Parquet");

    // Read the series back
    println!("Reading series from Parquet...");
    let retrieved_series = storage.read_series(series_id).await?;
    if let Some(series) = retrieved_series {
        println!("✅ Retrieved series: {} ({})", series.title, series.id);
    }

    // Read data points
    println!("Reading data points from Parquet...");
    let retrieved_points = storage.read_data_points(series_id, None, None).await?;
    println!("✅ Retrieved {} data points", retrieved_points.len());

    // Demonstrate Arrow schema creation
    println!("Creating Arrow schemas...");
    let series_schema = ParquetStorage::series_arrow_schema();
    let data_points_schema = ParquetStorage::data_points_arrow_schema();
    println!("✅ Series schema: {} fields", series_schema.fields().len());
    println!(
        "✅ Data points schema: {} fields",
        data_points_schema.fields().len()
    );

    // Clean up
    std::fs::remove_dir_all(&temp_dir)?;
    println!("✅ Cleaned up temporary files");

    Ok(())
}

async fn example_crawler_api() -> Result<()> {
    println!("Creating crawler API...");
    let crawler_api = CrawlerWriteApi::new();

    // Create a test payload
    let series_id = Uuid::new_v4();
    let data_points = vec![
        econ_graph_financial_data::models::DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::from_str("100.5")?)),
            revision_date: NaiveDate::from_ymd_opt(2023, 1, 15).unwrap(),
            is_original_release: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        econ_graph_financial_data::models::DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date: NaiveDate::from_ymd_opt(2023, 2, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::from_str("101.2")?)),
            revision_date: NaiveDate::from_ymd_opt(2023, 2, 15).unwrap(),
            is_original_release: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
    ];

    let payload = econ_graph_financial_data::crawler::CleanDataPayload {
        series_id,
        data_points,
        metadata: None,
    };

    println!("Processing clean data...");
    let response = crawler_api.process_clean_data(payload).await?;
    println!(
        "✅ Processed {} data points",
        response.data_points_processed
    );
    println!("✅ Success: {}", response.success);
    println!("✅ Message: {}", response.message);

    Ok(())
}

async fn example_error_handling() -> Result<()> {
    println!("Demonstrating error handling...");

    let database = Database::new_in_memory().await?;
    let non_existent_id = Uuid::new_v4();

    // Try to get a non-existent series
    match database.get_series(non_existent_id).await {
        Ok(series) => {
            if series.is_none() {
                println!("✅ Correctly returned None for non-existent series");
            } else {
                println!("❌ Unexpected series found");
            }
        }
        Err(e) => {
            println!("❌ Unexpected error: {}", e);
        }
    }

    // Try to get data points for a non-existent series
    match database.get_data_points(non_existent_id, None, None).await {
        Ok(points) => {
            if points.is_empty() {
                println!("✅ Correctly returned empty array for non-existent series");
            } else {
                println!("❌ Unexpected data points found");
            }
        }
        Err(e) => {
            println!("❌ Unexpected error: {}", e);
        }
    }

    // Demonstrate validation errors
    let invalid_input = CreateEconomicSeriesInput {
        source_id: Uuid::new_v4(),
        external_id: "".to_string(), // Empty external ID should be invalid
        title: "Test Series".to_string(),
        description: None,
        units: None,
        frequency: "INVALID".to_string(), // Invalid frequency
        seasonal_adjustment: None,
        start_date: None,
        end_date: None,
        is_active: true,
    };

    let invalid_series = econ_graph_financial_data::models::EconomicSeries {
        id: Uuid::new_v4(),
        source_id: invalid_input.source_id,
        external_id: invalid_input.external_id,
        title: invalid_input.title,
        description: invalid_input.description,
        units: invalid_input.units,
        frequency: invalid_input.frequency,
        seasonal_adjustment: invalid_input.seasonal_adjustment,
        start_date: invalid_input.start_date,
        end_date: invalid_input.end_date,
        is_active: invalid_input.is_active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    match database.create_series(invalid_series).await {
        Ok(_) => {
            println!("❌ Should have failed validation");
        }
        Err(e) => {
            println!("✅ Correctly caught validation error: {}", e);
        }
    }

    Ok(())
}
