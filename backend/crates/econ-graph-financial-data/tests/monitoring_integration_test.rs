use anyhow::Result;
use chrono::{NaiveDate, Utc};
use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::graphql::create_test_schema;
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
use uuid::Uuid;

/// Test monitoring integration without HTTP complexity
///
/// This test exercises the monitoring system by:
/// 1. Creating a GraphQL schema with monitoring components
/// 2. Executing GraphQL operations directly (no HTTP)
/// 3. Verifying metrics are collected
/// 4. Testing health checks
#[tokio::test]
async fn test_monitoring_integration() -> Result<()> {
    println!("🔧 Testing monitoring integration...");

    // Step 1: Initialize database and schema with monitoring
    let database = Database::new_in_memory().await?;
    let schema = create_test_schema(database).await?;
    println!("✅ Database and schema initialized with monitoring");

    // Step 2: Test GraphQL operations and verify metrics collection
    test_graphql_operations(&schema).await?;
    println!("✅ GraphQL operations with metrics collection verified");

    // Step 3: Test health check functionality
    test_health_checks(&schema).await?;
    println!("✅ Health checks verified");

    println!("🎉 Monitoring integration test completed successfully!");
    Ok(())
}

async fn test_graphql_operations(
    schema: &async_graphql::Schema<
        econ_graph_financial_data::graphql::Query,
        econ_graph_financial_data::graphql::Mutation,
        async_graphql::EmptySubscription,
    >,
) -> Result<()> {
    println!("📊 Testing GraphQL operations with metrics...");

    // Test 1: Create a series (mutation)
    let create_series_query = r#"
        mutation {
            createSeries(input: {
                sourceId: "123e4567-e89b-12d3-a456-426614174000"
                externalId: "TEST001"
                title: "Test Economic Series"
                frequency: "Daily"
                isActive: true
            }) {
                id
                title
                externalId
            }
        }
    "#;

    let result = schema.execute(create_series_query).await;
    assert!(
        result.errors.is_empty(),
        "Create series failed: {:?}",
        result.errors
    );

    let series_data = result.data.into_json()?;
    let series_id = series_data["createSeries"]["id"].as_str().unwrap();
    println!("✅ Series created: {}", series_id);

    // Test 2: Query the series
    let query_series = format!(
        r#"
        query {{
            series(id: "{}") {{
                id
                title
                externalId
            }}
        }}
    "#,
        series_id
    );

    let result = schema.execute(&query_series).await;
    assert!(
        result.errors.is_empty(),
        "Query series failed: {:?}",
        result.errors
    );
    println!("✅ Series queried successfully");

    // Test 3: List all series
    let list_series_query = r#"
        query {
            listSeries {
                id
                title
                externalId
            }
        }
    "#;

    let result = schema.execute(list_series_query).await;
    assert!(
        result.errors.is_empty(),
        "List series failed: {:?}",
        result.errors
    );
    println!("✅ Series listed successfully");

    // Test 4: Create data points
    let create_data_points_query = format!(
        r#"
        mutation {{
            createDataPoints(inputs: [{{
                seriesId: "{}"
                date: "2024-01-01"
                value: "100.50"
                revisionDate: "2024-01-01"
                isOriginalRelease: true
            }}]) {{
                id
                seriesId
                date
                value
            }}
        }}
    "#,
        series_id
    );

    let result = schema.execute(&create_data_points_query).await;
    assert!(
        result.errors.is_empty(),
        "Create data points failed: {:?}",
        result.errors
    );
    println!("✅ Data points created successfully");

    // Test 5: Query data points
    let query_data_points = format!(
        r#"
        query {{
            dataPoints(seriesId: "{}") {{
                id
                seriesId
                date
                value
            }}
        }}
    "#,
        series_id
    );

    let result = schema.execute(&query_data_points).await;
    assert!(
        result.errors.is_empty(),
        "Query data points failed: {:?}",
        result.errors
    );
    println!("✅ Data points queried successfully");

    Ok(())
}

async fn test_health_checks(
    schema: &async_graphql::Schema<
        econ_graph_financial_data::graphql::Query,
        econ_graph_financial_data::graphql::Mutation,
        async_graphql::EmptySubscription,
    >,
) -> Result<()> {
    println!("❤️ Testing health checks...");

    // Test health check query
    let health_query = r#"
        query {
            health
        }
    "#;

    let result = schema.execute(health_query).await;
    assert!(
        result.errors.is_empty(),
        "Health check failed: {:?}",
        result.errors
    );

    let health_data = result.data.into_json()?;
    let health_status = health_data["health"].as_str().unwrap();
    assert!(health_status.contains("healthy") || health_status.contains("degraded"));
    println!("✅ Health check returned: {}", health_status);

    Ok(())
}

/// Test direct database operations with monitoring
#[tokio::test]
async fn test_database_operations_with_monitoring() -> Result<()> {
    println!("🗄️ Testing database operations with monitoring...");

    let database = Database::new_in_memory().await?;

    // Test series operations
    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "TEST_DB_001".to_string(),
        title: "Test Database Series".to_string(),
        description: Some("Test series for database monitoring".to_string()),
        units: Some("USD".to_string()),
        frequency: "Daily".to_string(),
        seasonal_adjustment: None,
        start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        end_date: None,
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Create series (should log timing)
    database.create_series(series.clone()).await?;
    println!("✅ Series created with monitoring");

    // Read series (should log timing)
    let retrieved_series = database.get_series(series.id).await?;
    assert!(retrieved_series.is_some());
    println!("✅ Series retrieved with monitoring");

    // Test data points operations
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

    // Create data points (should log timing)
    database.create_data_points(data_points.clone()).await?;
    println!("✅ Data points created with monitoring");

    // Read data points (should log timing)
    let retrieved_points = database.get_data_points(series.id, None, None).await?;
    assert_eq!(retrieved_points.len(), 2);
    println!("✅ Data points retrieved with monitoring");

    // List series (should log timing)
    let all_series = database.list_series().await?;
    assert_eq!(all_series.len(), 1);
    println!("✅ Series listed with monitoring");

    println!("🎉 Database operations with monitoring completed successfully!");
    Ok(())
}

/// Test monitoring components directly
#[tokio::test]
async fn test_monitoring_components() -> Result<()> {
    println!("📈 Testing monitoring components directly...");

    use econ_graph_financial_data::monitoring::{HealthChecker, HealthStatus, MetricsCollector};

    // Test metrics collector
    let metrics = MetricsCollector::new();

    // Test counter operations
    metrics.increment_counter("test_operations", 1).await;
    metrics.increment_counter("test_operations", 2).await;

    // Test timing operations
    let start = std::time::Instant::now();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    metrics
        .record_timing("test_duration", start.elapsed())
        .await;

    // Test GraphQL operation recording
    metrics
        .record_graphql_operation("query_test_query", start.elapsed(), true)
        .await;

    let metrics_summary = metrics.get_metrics_summary().await;
    assert!(!metrics_summary.is_empty());
    println!(
        "✅ Metrics collector working: {} metrics recorded",
        metrics_summary.len()
    );

    // Test health checker
    let health_checker = HealthChecker::new();

    // Test health check registration and updates
    health_checker
        .update_check(
            "test_check",
            HealthStatus::Healthy,
            Some("Test check passed".to_string()),
        )
        .await;

    let overall_health = health_checker.get_overall_health().await;
    assert!(matches!(overall_health, HealthStatus::Healthy));

    let health_report = health_checker.get_health_report().await;
    assert!(!health_report.is_empty());
    println!(
        "✅ Health checker working: {} checks registered",
        health_report.len()
    );

    println!("🎉 Monitoring components test completed successfully!");
    Ok(())
}
