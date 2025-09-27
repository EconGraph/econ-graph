use anyhow::Result;
use chrono::{NaiveDate, Utc};
use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
use econ_graph_financial_data::storage::parquet_storage::ParquetFlightService;
use econ_graph_financial_data::storage::{FinancialDataStorage, ParquetStorage};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::Value;
use tempfile::TempDir;
use uuid::Uuid;

/// Comprehensive integration test that:
/// 1. Creates Parquet files with Arrow structure
/// 2. Tests GraphQL schema execution with Parquet storage
/// 3. Verifies end-to-end data flow without HTTP complexity
/// 4. Demonstrates Arrow Flight concepts in action
#[tokio::test]
async fn test_graphql_parquet_integration() -> Result<()> {
    println!("🚀 Starting GraphQL + Parquet Integration Test");

    // Step 1: Create temporary directory for Parquet files
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_path_buf();
    println!("📁 Using data directory: {:?}", data_dir);

    // Step 2: Create test data and write to Parquet files
    let (series_id, source_id) = create_test_data_in_parquet(&data_dir).await?;
    println!("✅ Test data written to Parquet files");

    // Step 3: Create GraphQL schema with Parquet storage
    let database = Database::new_with_parquet(data_dir.to_string_lossy()).await?;
    let schema = econ_graph_financial_data::graphql::create_test_schema(database.clone()).await?;
    println!("🌐 GraphQL schema created with Parquet storage");

    // Step 4: Test GraphQL queries at payload level
    test_graphql_queries(&schema).await?;
    println!("✅ GraphQL queries executed successfully");

    // Step 5: Test GraphQL mutations at payload level
    test_graphql_mutations(&schema, series_id).await?;
    println!("✅ GraphQL mutations executed successfully");

    // Step 6: Verify Parquet files were created/updated
    verify_parquet_files_created(&data_dir, series_id).await?;
    println!("✅ Parquet files verified");

    // Step 7: Test Arrow Flight concepts
    test_arrow_flight_concepts(&data_dir).await?;
    println!("✅ Arrow Flight concepts validated");

    println!("🎉 GraphQL + Parquet Integration Test PASSED!");
    println!("   - Parquet files created with Arrow structure");
    println!("   - GraphQL schema executed with Parquet backend");
    println!("   - All GraphQL operations work with Arrow Flight storage");
    println!("   - Data persisted to Parquet files");
    println!("   - Arrow Flight concepts demonstrated");
    println!("   - Ready for Iceberg integration!");

    Ok(())
}

/// Create test data and write it to Parquet files using Arrow Flight storage
async fn create_test_data_in_parquet(data_dir: &std::path::Path) -> Result<(Uuid, Uuid)> {
    let storage = ParquetStorage::new(data_dir.to_path_buf());

    let source_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();

    // Create test series
    let series = EconomicSeries {
        id: series_id,
        source_id,
        external_id: "INTEGRATION_TEST_001".to_string(),
        title: "GraphQL Parquet Integration Test Series".to_string(),
        description: Some(
            "A comprehensive test series for GraphQL + Parquet integration".to_string(),
        ),
        units: Some("Index".to_string()),
        frequency: "monthly".to_string(),
        seasonal_adjustment: Some("Seasonally Adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Write series to Parquet
    storage.write_series(&series).await?;

    // Create test data points
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
        DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date: NaiveDate::from_ymd_opt(2023, 3, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::new(1100, 0))),
            revision_date: NaiveDate::from_ymd_opt(2023, 3, 1).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    // Write data points to Parquet
    storage.write_data_points(series_id, &data_points).await?;

    Ok((series_id, source_id))
}

/// Test GraphQL queries at payload level
async fn test_graphql_queries(
    schema: &async_graphql::Schema<
        econ_graph_financial_data::graphql::Query,
        econ_graph_financial_data::graphql::Mutation,
        async_graphql::EmptySubscription,
    >,
) -> Result<()> {
    // Test 1: Health check
    let health_query = r#"{ health }"#;
    let result = schema.execute(health_query).await;
    assert!(result.errors.is_empty());
    // GraphQL returns unquoted JSON, so we need to parse it differently
    let health_response = result.data.to_string();
    assert!(health_response.contains("Financial Data Service is healthy"));
    println!("   ✅ Health check query passed");

    // Test 2: List all series
    let list_query = r#"query { listSeries { id externalId title frequency isActive } }"#;
    let result = schema.execute(list_query).await;
    if !result.errors.is_empty() {
        println!("   Debug: Series query errors: {:?}", result.errors);
    }
    assert!(result.errors.is_empty());
    let list_response = result.data.to_string();
    // Check that we get a response with series data
    assert!(list_response.contains("INTEGRATION_TEST_001"));
    println!("   ✅ List series query passed");

    Ok(())
}

/// Test GraphQL mutations at payload level
async fn test_graphql_mutations(
    schema: &async_graphql::Schema<
        econ_graph_financial_data::graphql::Query,
        econ_graph_financial_data::graphql::Mutation,
        async_graphql::EmptySubscription,
    >,
    series_id: Uuid,
) -> Result<()> {
    // Test 1: Create new series
    let create_series_mutation = format!(
        r#"
        mutation CreateSeries {{
            createSeries(input: {{
                sourceId: "{}"
                externalId: "MUTATION_TEST_001"
                title: "GraphQL Mutation Test Series"
                description: "Created via GraphQL mutation"
                units: "Percentage"
                frequency: "quarterly"
                seasonalAdjustment: "Not Seasonally Adjusted"
                startDate: "2023-01-01"
                endDate: "2023-12-31"
                isActive: true
            }}) {{
                id
                externalId
                title
                frequency
                isActive
            }}
        }}
    "#,
        Uuid::new_v4()
    );

    let result = schema.execute(&create_series_mutation).await;
    assert!(result.errors.is_empty());
    let mutation_response = result.data.to_string();
    assert!(mutation_response.contains("MUTATION_TEST_001"));
    assert!(mutation_response.contains("GraphQL Mutation Test Series"));
    assert!(mutation_response.contains("quarterly"));
    println!("   ✅ Create series mutation passed");

    // Test 2: Create data points
    let create_data_points_mutation = format!(
        r#"
        mutation CreateDataPoints {{
            createDataPoints(inputs: [
                {{
                    seriesId: "{}"
                    date: "2023-04-01"
                    value: "1200"
                    revisionDate: "2023-04-01"
                    isOriginalRelease: true
                }},
                {{
                    seriesId: "{}"
                    date: "2023-05-01"
                    value: "1250"
                    revisionDate: "2023-05-01"
                    isOriginalRelease: true
                }}
            ]) {{
                id
                seriesId
                date
                value
                isOriginalRelease
            }}
        }}
    "#,
        series_id, series_id
    );

    let result = schema.execute(&create_data_points_mutation).await;
    assert!(result.errors.is_empty());
    let data_points_response = result.data.to_string();
    // Check that we get a response with data points
    assert!(data_points_response.contains("2023-04-01"));
    assert!(data_points_response.contains("1200"));
    println!("   ✅ Create data points mutation passed");

    Ok(())
}

/// Verify that Parquet files were created and contain the expected data
async fn verify_parquet_files_created(data_dir: &std::path::Path, series_id: Uuid) -> Result<()> {
    let storage = ParquetStorage::new(data_dir.to_path_buf());

    // Verify series file exists and can be read
    let series_file = data_dir.join(format!("series_{}.parquet", series_id));
    assert!(series_file.exists(), "Series Parquet file should exist");

    let retrieved_series = storage.read_series(series_id).await?;
    assert!(
        retrieved_series.is_some(),
        "Should be able to read series from Parquet"
    );

    let series = retrieved_series.unwrap();
    assert_eq!(series.external_id, "INTEGRATION_TEST_001");
    assert_eq!(series.title, "GraphQL Parquet Integration Test Series");

    // Verify data points file exists and can be read
    let data_points_file = data_dir.join(format!("data_points_{}.parquet", series_id));
    assert!(
        data_points_file.exists(),
        "Data points Parquet file should exist"
    );

    let retrieved_points = storage.read_data_points(series_id, None, None).await?;
    assert!(
        retrieved_points.len() >= 2,
        "Should have at least 2 data points"
    );

    // Verify the data points have the expected values
    let values: Vec<f64> = retrieved_points
        .iter()
        .filter_map(|p| p.value.as_ref().map(|v| v.0.to_f64().unwrap_or(0.0)))
        .collect();

    assert!(values.contains(&1200.0), "Should contain value 1200");
    assert!(values.contains(&1250.0), "Should contain value 1250");

    println!("✅ Parquet files verified:");
    println!("   - Series file: {:?}", series_file);
    println!("   - Data points file: {:?}", data_points_file);
    println!(
        "   - Series data: {} ({} points)",
        series.title,
        retrieved_points.len()
    );

    Ok(())
}

/// Test Arrow Flight concepts and zero-copy data transfer
async fn test_arrow_flight_concepts(data_dir: &std::path::Path) -> Result<()> {
    println!("🔍 Testing Arrow Flight Concepts");

    let storage = ParquetStorage::new(data_dir.to_path_buf());

    // Test 1: Arrow Schema Creation
    let series_schema = ParquetStorage::series_arrow_schema();
    let data_points_schema = ParquetStorage::data_points_arrow_schema();

    assert!(
        !series_schema.fields().is_empty(),
        "Series schema should have fields"
    );
    assert_eq!(
        series_schema.fields().len(),
        13,
        "Series schema should have 13 fields"
    );
    assert!(
        !data_points_schema.fields().is_empty(),
        "Data points schema should have fields"
    );
    assert_eq!(
        data_points_schema.fields().len(),
        8,
        "Data points schema should have 8 fields"
    );
    println!("   ✅ Arrow schemas created successfully");

    // Test 2: RecordBatch Operations (Arrow Flight core concept)
    let test_series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "ARROW_FLIGHT_TEST".to_string(),
        title: "Arrow Flight Test Series".to_string(),
        description: Some("Testing Arrow Flight concepts".to_string()),
        units: Some("Index".to_string()),
        frequency: "daily".to_string(),
        seasonal_adjustment: Some("Seasonally Adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Convert to Arrow RecordBatch (zero-copy concept)
    let series_batch = ParquetStorage::series_to_record_batch(&test_series)?;
    assert_eq!(series_batch.num_rows(), 1);
    assert_eq!(series_batch.num_columns(), 13);
    println!("   ✅ Series converted to Arrow RecordBatch");

    // Test 3: Data Points RecordBatch
    let test_data_points = vec![
        DataPoint {
            id: Uuid::new_v4(),
            series_id: test_series.id,
            date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::new(1000, 0))),
            revision_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        DataPoint {
            id: Uuid::new_v4(),
            series_id: test_series.id,
            date: NaiveDate::from_ymd_opt(2023, 1, 2).unwrap(),
            value: Some(DecimalScalar(Decimal::new(1010, 0))),
            revision_date: NaiveDate::from_ymd_opt(2023, 1, 2).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    let data_points_batch = ParquetStorage::data_points_to_record_batch(&test_data_points)?;
    assert_eq!(data_points_batch.num_rows(), 2);
    assert_eq!(data_points_batch.num_columns(), 8);
    println!("   ✅ Data points converted to Arrow RecordBatch");

    // Test 4: Parquet File Operations (Arrow Flight storage)
    let series_file = data_dir.join("arrow_flight_test_series.parquet");
    let data_points_file = data_dir.join("arrow_flight_test_data_points.parquet");

    // Write RecordBatches to Parquet (Arrow Flight storage concept)
    storage
        .write_record_batch_to_parquet(series_batch, series_file.clone())
        .await?;
    storage
        .write_record_batch_to_parquet(data_points_batch, data_points_file.clone())
        .await?;

    assert!(series_file.exists(), "Series Parquet file should exist");
    assert!(
        data_points_file.exists(),
        "Data points Parquet file should exist"
    );
    println!("   ✅ RecordBatches written to Parquet files");

    // Test 5: Read back from Parquet (Arrow Flight retrieval)
    let retrieved_series_batch = storage.read_record_batch_from_parquet(series_file).await?;
    let retrieved_data_points_batch = storage
        .read_record_batch_from_parquet(data_points_file)
        .await?;

    assert!(
        retrieved_series_batch.is_some(),
        "Should read series batch from Parquet"
    );
    assert!(
        retrieved_data_points_batch.is_some(),
        "Should read data points batch from Parquet"
    );

    let series_batch = retrieved_series_batch.unwrap();
    let data_points_batch = retrieved_data_points_batch.unwrap();

    assert_eq!(series_batch.num_rows(), 1);
    assert_eq!(data_points_batch.num_rows(), 2);
    println!("   ✅ RecordBatches read back from Parquet files");

    // Test 6: Arrow Flight Service Schema (V2 preparation)
    let _flight_service = ParquetFlightService::new(data_dir.to_path_buf());
    let financial_schema = ParquetFlightService::get_financial_data_schema();
    let data_points_schema = ParquetFlightService::get_data_points_schema();

    assert!(!financial_schema.fields().is_empty());
    assert!(!data_points_schema.fields().is_empty());
    println!("   ✅ Arrow Flight service schemas ready");

    println!("✅ Arrow Flight Concepts PASSED");
    println!("   - Arrow schemas created and validated");
    println!("   - RecordBatch operations (zero-copy data transfer)");
    println!("   - Parquet file read/write with Arrow");
    println!("   - Arrow Flight service preparation");
    println!("   - Ready for full Arrow Flight server implementation");

    Ok(())
}

/// Test Arrow schema creation and validation
#[tokio::test]
async fn test_arrow_schema_validation() -> Result<()> {
    println!("🔍 Testing Arrow Schema Validation");

    let series_schema = ParquetStorage::series_arrow_schema();
    let data_points_schema = ParquetStorage::data_points_arrow_schema();

    // Verify series schema
    assert!(
        !series_schema.fields().is_empty(),
        "Series schema should have fields"
    );
    assert_eq!(
        series_schema.fields().len(),
        13,
        "Series schema should have 13 fields"
    );

    let field_names: Vec<String> = series_schema
        .fields()
        .iter()
        .map(|f| f.name().to_string())
        .collect();
    assert!(
        field_names.contains(&"id".to_string()),
        "Series schema should have 'id' field"
    );
    assert!(
        field_names.contains(&"title".to_string()),
        "Series schema should have 'title' field"
    );
    assert!(
        field_names.contains(&"frequency".to_string()),
        "Series schema should have 'frequency' field"
    );

    // Verify data points schema
    assert!(
        !data_points_schema.fields().is_empty(),
        "Data points schema should have fields"
    );
    assert_eq!(
        data_points_schema.fields().len(),
        8,
        "Data points schema should have 8 fields"
    );

    let data_point_field_names: Vec<String> = data_points_schema
        .fields()
        .iter()
        .map(|f| f.name().to_string())
        .collect();
    assert!(
        data_point_field_names.contains(&"id".to_string()),
        "Data points schema should have 'id' field"
    );
    assert!(
        data_point_field_names.contains(&"series_id".to_string()),
        "Data points schema should have 'series_id' field"
    );
    assert!(
        data_point_field_names.contains(&"date".to_string()),
        "Data points schema should have 'date' field"
    );
    assert!(
        data_point_field_names.contains(&"value".to_string()),
        "Data points schema should have 'value' field"
    );

    println!("✅ Arrow Schema Validation PASSED");
    println!(
        "   - Series schema: {} fields",
        series_schema.fields().len()
    );
    println!(
        "   - Data points schema: {} fields",
        data_points_schema.fields().len()
    );
    println!("   - All required fields present");
    println!("   - Schema ready for Arrow Flight operations");

    Ok(())
}
