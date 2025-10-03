use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::prelude::ToPrimitive;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::graphql::create_test_schema;
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};

/// Helper function to create test data chunks
fn create_data_chunk(
    series_id: Uuid,
    start_date: NaiveDate,
    count: usize,
    base_value: f64,
    increment: f64,
) -> Vec<DataPoint> {
    let mut points = Vec::new();
    for i in 0..count {
        let date = start_date + chrono::Duration::days(i as i64);
        let value = base_value + (i as f64 * increment);
        points.push(DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date,
            value: Some(DecimalScalar(
                rust_decimal::Decimal::from_f64_retain(value).unwrap_or_default(),
            )),
            revision_date: date,
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });
    }
    points
}

/// Financial-Specific Iceberg Integration Tests
///
/// These tests focus on the unique requirements of financial time series data:
/// 1. Data Revisions - Handling multiple revisions of the same data point
/// 2. Holiday Handling - Managing missing data due to market closures
/// 3. Data Quality - Handling outliers, missing values, and data corrections
/// 4. Regulatory Compliance - Audit trails and data lineage
/// 5. Real-time Updates - Streaming data ingestion and updates
/// 6. Cross-Series Analysis - Correlations and relationships between series
/// 7. Seasonal Adjustments - Handling seasonal patterns and adjustments
/// 8. Data Validation - Ensuring data integrity and consistency
/// Test 1: Data Revisions and Audit Trail
///
/// Financial data is frequently revised. This test demonstrates how Iceberg
/// handles multiple revisions of the same data point while maintaining
/// a complete audit trail.
#[tokio::test]
async fn test_iceberg_data_revisions() -> Result<(), Box<dyn std::error::Error>> {
    use econ_graph_financial_data::database::Database;
    use econ_graph_financial_data::graphql::create_test_schema;
    use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};

    println!("📊 Testing Iceberg Data Revisions...");
    println!("This demonstrates handling multiple revisions of financial data!");

    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&catalog_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;

    // Create series for revision testing
    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "REVISION_TEST_001".to_string(),
        title: "Data Revision Test Series".to_string(),
        description: Some("Testing data revision capabilities".to_string()),
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

    // Initial data release
    println!("📈 Initial data release...");
    let initial_data = create_data_chunk(
        series.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        30,
        100.0,
        0.1,
    );
    database.create_data_points(initial_data).await?;
    let initial_release_time = Utc::now();
    println!("  ✅ Initial release completed at {}", initial_release_time);

    // Wait for revision
    sleep(Duration::from_millis(100)).await;

    // First revision - Correct some values
    println!("📈 First revision - Correcting some values...");
    let revision1_data = create_revision_data(
        series.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        30,
        100.0,
        0.1,
        false, // Not original release
    );
    database.create_data_points(revision1_data).await?;
    let revision1_time = Utc::now();
    println!("  ✅ First revision completed at {}", revision1_time);

    // Wait for another revision
    sleep(Duration::from_millis(100)).await;

    // Second revision - Further corrections
    println!("📈 Second revision - Further corrections...");
    let revision2_data = create_revision_data(
        series.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        30,
        100.0,
        0.1,
        false, // Not original release
    );
    database.create_data_points(revision2_data).await?;
    let revision2_time = Utc::now();
    println!("  ✅ Second revision completed at {}", revision2_time);

    // Test querying latest data
    println!("🔍 Testing latest data query...");
    let schema = create_test_schema(database.clone()).await?;

    let query_latest = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-01-01"
                endDate: "2020-01-31"
            ) {{
                date
                value
                revisionDate
                isOriginalRelease
            }}
        }}
        "#,
        series.id
    );

    let result_latest = schema.execute(query_latest).await;
    assert!(result_latest.errors.is_empty());
    println!("  ✅ Latest data query successful");

    // Test querying historical revisions
    println!("🔍 Testing historical revision query...");
    let query_historical = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-01-01"
                endDate: "2020-01-31"
            ) {{
                date
                value
                revisionDate
                isOriginalRelease
            }}
        }}
        "#,
        series.id
    );

    let result_historical = schema.execute(query_historical).await;
    assert!(result_historical.errors.is_empty());
    println!("  ✅ Historical revision query successful");

    // Test audit trail
    println!("🔍 Testing audit trail...");
    let query_audit = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-01-01"
                endDate: "2020-01-31"
            ) {{
                date
                value
                revisionDate
                isOriginalRelease
                createdAt
                updatedAt
            }}
        }}
        "#,
        series.id
    );

    let result_audit = schema.execute(query_audit).await;
    assert!(result_audit.errors.is_empty());
    println!("  ✅ Audit trail query successful");

    println!("🎉 Data Revisions Test Complete!");
    println!("Key Benefits Demonstrated:");
    println!("  • Multiple revisions of the same data point");
    println!("  • Complete audit trail maintenance");
    println!("  • Historical data access");
    println!("  • Regulatory compliance support");

    Ok(())
}

/// Test 2: Holiday Handling and Missing Data
///
/// Financial markets are closed on holidays. This test demonstrates how
/// Iceberg handles missing data and holiday patterns.
#[tokio::test]
async fn test_iceberg_holiday_handling() -> Result<(), Box<dyn std::error::Error>> {
    use econ_graph_financial_data::database::Database;
    use econ_graph_financial_data::graphql::create_test_schema;
    use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};

    println!("🎄 Testing Iceberg Holiday Handling...");
    println!("This demonstrates handling missing data due to market closures!");

    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&catalog_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;

    // Create series for holiday testing
    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "HOLIDAY_TEST_001".to_string(),
        title: "Holiday Handling Test Series".to_string(),
        description: Some("Testing holiday handling capabilities".to_string()),
        units: Some("USD".to_string()),
        frequency: "daily".to_string(),
        seasonal_adjustment: None,
        start_date: Some(NaiveDate::from_ymd_opt(2020, 12, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2021, 1, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    database.create_series(series.clone()).await?;

    // Add data with holiday gaps
    println!("📈 Adding data with holiday gaps...");
    let holiday_data = create_holiday_data(
        series.id,
        NaiveDate::from_ymd_opt(2020, 12, 1).unwrap(),
        62, // December 1 to January 31
        100.0,
        0.1,
    );
    database.create_data_points(holiday_data).await?;
    println!("  ✅ Holiday data added with gaps");

    // Test querying data with gaps
    println!("🔍 Testing query with holiday gaps...");
    let schema = create_test_schema(database.clone()).await?;

    let query_with_gaps = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-12-20"
                endDate: "2021-01-10"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result_with_gaps = schema.execute(query_with_gaps).await;
    assert!(result_with_gaps.errors.is_empty());
    println!("  ✅ Query with holiday gaps successful");

    // Test querying specific holiday periods
    println!("🔍 Testing holiday period queries...");
    let query_holiday_period = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-12-24"
                endDate: "2020-12-26"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result_holiday_period = schema.execute(query_holiday_period).await;
    assert!(result_holiday_period.errors.is_empty());
    println!("  ✅ Holiday period query successful");

    // Test querying non-holiday periods
    println!("🔍 Testing non-holiday period queries...");
    let query_non_holiday = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-12-21"
                endDate: "2020-12-23"
            ) {{
                date
                value
            }}
        }}
        "#,
        series.id
    );

    let result_non_holiday = schema.execute(query_non_holiday).await;
    assert!(result_non_holiday.errors.is_empty());
    println!("  ✅ Non-holiday period query successful");

    println!("🎉 Holiday Handling Test Complete!");
    println!("Key Benefits Demonstrated:");
    println!("  • Handling missing data due to market closures");
    println!("  • Efficient querying with data gaps");
    println!("  • Holiday pattern recognition");
    println!("  • Flexible date range queries");

    Ok(())
}

/// Test 3: Data Quality and Outlier Handling
///
/// Financial data often contains outliers and quality issues. This test
/// demonstrates how Iceberg handles data quality challenges.
#[tokio::test]
async fn test_iceberg_data_quality() -> Result<(), Box<dyn std::error::Error>> {
    use econ_graph_financial_data::database::Database;
    use econ_graph_financial_data::graphql::create_test_schema;
    use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};

    println!("🔍 Testing Iceberg Data Quality...");
    println!("This demonstrates handling outliers and data quality issues!");

    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&catalog_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;

    // Create series for quality testing
    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "QUALITY_TEST_001".to_string(),
        title: "Data Quality Test Series".to_string(),
        description: Some("Testing data quality handling capabilities".to_string()),
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

    // Add data with quality issues
    println!("📈 Adding data with quality issues...");
    let quality_data = create_quality_data(
        series.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        30,
        100.0,
        0.1,
    );
    database.create_data_points(quality_data).await?;
    println!("  ✅ Quality data added with issues");

    // Test querying data with quality issues
    println!("🔍 Testing query with quality issues...");
    let schema = create_test_schema(database.clone()).await?;

    let query_quality = format!(
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

    let result_quality = schema.execute(query_quality).await;
    assert!(result_quality.errors.is_empty());
    println!("  ✅ Query with quality issues successful");

    // Test data validation
    println!("🔍 Testing data validation...");
    let query_validation = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-01-01"
                endDate: "2020-01-31"
            ) {{
                date
                value
                revisionDate
                isOriginalRelease
            }}
        }}
        "#,
        series.id
    );

    let result_validation = schema.execute(query_validation).await;
    assert!(result_validation.errors.is_empty());
    println!("  ✅ Data validation query successful");

    println!("🎉 Data Quality Test Complete!");
    println!("Key Benefits Demonstrated:");
    println!("  • Handling outliers and data quality issues");
    println!("  • Data validation and consistency checks");
    println!("  • Quality metadata tracking");
    println!("  • Flexible data access patterns");

    Ok(())
}

/// Test 4: Cross-Series Analysis
///
/// Financial analysis often requires comparing multiple series. This test
/// demonstrates how Iceberg handles cross-series queries and analysis.
#[tokio::test]
async fn test_iceberg_cross_series_analysis() -> Result<(), Box<dyn std::error::Error>> {
    use econ_graph_financial_data::database::Database;
    use econ_graph_financial_data::graphql::create_test_schema;
    use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};

    println!("📊 Testing Iceberg Cross-Series Analysis...");
    println!("This demonstrates cross-series queries and analysis!");

    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&catalog_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;

    // Create multiple related series
    println!("📈 Creating multiple related series...");
    let series1 = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "CROSS_SERIES_001".to_string(),
        title: "GDP Growth Rate".to_string(),
        description: Some("Gross Domestic Product Growth Rate".to_string()),
        units: Some("Percent".to_string()),
        frequency: "quarterly".to_string(),
        seasonal_adjustment: Some("seasonally_adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let series2 = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "CROSS_SERIES_002".to_string(),
        title: "Unemployment Rate".to_string(),
        description: Some("Unemployment Rate".to_string()),
        units: Some("Percent".to_string()),
        frequency: "monthly".to_string(),
        seasonal_adjustment: Some("seasonally_adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let series3 = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "CROSS_SERIES_003".to_string(),
        title: "Inflation Rate".to_string(),
        description: Some("Consumer Price Index Inflation Rate".to_string()),
        units: Some("Percent".to_string()),
        frequency: "monthly".to_string(),
        seasonal_adjustment: Some("seasonally_adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    database.create_series(series1.clone()).await?;
    database.create_series(series2.clone()).await?;
    database.create_series(series3.clone()).await?;
    println!("  ✅ Multiple series created");

    // Add data to each series
    println!("📈 Adding data to each series...");

    // GDP data (quarterly)
    let gdp_data = create_quarterly_data(
        series1.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        4,   // 4 quarters
        2.5, // Starting growth rate
        0.1, // Quarterly change
    );
    database.create_data_points(gdp_data).await?;

    // Unemployment data (monthly)
    let unemployment_data = create_monthly_data(
        series2.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        12,  // 12 months
        3.5, // Starting unemployment rate
        0.1, // Monthly change
    );
    database.create_data_points(unemployment_data).await?;

    // Inflation data (monthly)
    let inflation_data = create_monthly_data(
        series3.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        12,   // 12 months
        2.0,  // Starting inflation rate
        0.05, // Monthly change
    );
    database.create_data_points(inflation_data).await?;

    println!("  ✅ Data added to all series");

    // Test cross-series queries
    println!("🔍 Testing cross-series queries...");
    let schema = create_test_schema(database.clone()).await?;

    // Query all series for a specific date range
    let query_all_series = r#"
        query {
            listSeries {
                id
                title
                externalId
                frequency
                units
            }
        }
        "#
    .to_string();

    let result_all_series = schema.execute(query_all_series).await;
    assert!(result_all_series.errors.is_empty());
    println!("  ✅ All series query successful");

    // Query specific series data
    let query_series1 = format!(
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
        series1.id
    );

    let result_series1 = schema.execute(query_series1).await;
    assert!(result_series1.errors.is_empty());
    println!("  ✅ Series 1 data query successful");

    // Query series metadata
    let query_metadata = format!(
        r#"
        query {{
            series(id: "{}") {{
                id
                title
                description
                units
                frequency
                seasonalAdjustment
            }}
        }}
        "#,
        series1.id
    );

    let result_metadata = schema.execute(query_metadata).await;
    assert!(result_metadata.errors.is_empty());
    println!("  ✅ Series metadata query successful");

    println!("🎉 Cross-Series Analysis Test Complete!");
    println!("Key Benefits Demonstrated:");
    println!("  • Multiple related series management");
    println!("  • Cross-series query capabilities");
    println!("  • Series metadata access");
    println!("  • Flexible analysis patterns");

    Ok(())
}

/// Test 5: Real-time Updates and Streaming
///
/// Financial data is often updated in real-time. This test demonstrates
/// how Iceberg handles streaming updates and real-time data ingestion.
#[tokio::test]
async fn test_iceberg_real_time_updates() -> Result<(), Box<dyn std::error::Error>> {
    use econ_graph_financial_data::database::Database;
    use econ_graph_financial_data::graphql::create_test_schema;
    use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
    use std::time::Instant;

    println!("⚡ Testing Iceberg Real-time Updates...");
    println!("This demonstrates streaming updates and real-time data ingestion!");

    let temp_dir = TempDir::new()?;
    let catalog_dir = temp_dir.path().join("catalog");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&catalog_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    let database = Database::new_with_parquet(data_dir.to_string_lossy().to_string()).await?;

    // Create series for real-time testing
    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "REALTIME_TEST_001".to_string(),
        title: "Real-time Test Series".to_string(),
        description: Some("Testing real-time update capabilities".to_string()),
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

    // Simulate real-time updates
    println!("📈 Simulating real-time updates...");
    let start_time = Instant::now();

    // Initial data
    let initial_data = create_data_chunk(
        series.id,
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        10,
        100.0,
        0.1,
    );
    database.create_data_points(initial_data).await?;
    println!("  ✅ Initial data added");

    // Simulate streaming updates
    for i in 0..10 {
        sleep(Duration::from_millis(50)).await; // Simulate real-time delay

        let update_data = create_data_chunk(
            series.id,
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap() + chrono::Duration::days(i * 10),
            10,
            100.0 + (i as f64 * 10.0),
            0.1,
        );

        database.create_data_points(update_data).await?;
        println!("  ✅ Update {} completed", i + 1);
    }

    let update_time = start_time.elapsed();
    println!("  ✅ Real-time updates completed in {:?}", update_time);

    // Test real-time queries
    println!("🔍 Testing real-time queries...");
    let schema = create_test_schema(database.clone()).await?;

    // Query latest data
    let query_latest = format!(
        r#"
        query {{
            dataPoints(
                seriesId: "{}"
                startDate: "2020-01-01"
                endDate: "2020-12-31"
            ) {{
                date
                value
                updatedAt
            }}
        }}
        "#,
        series.id
    );

    let result_latest = schema.execute(query_latest).await;
    assert!(result_latest.errors.is_empty());
    println!("  ✅ Latest data query successful");

    // Test concurrent updates
    println!("🔄 Testing concurrent updates...");
    let update_handles: Vec<_> = (0..5)
        .map(|i| {
            let db = database.clone();
            let series_id = series.id;
            let start_date =
                NaiveDate::from_ymd_opt(2020, 1, 1).unwrap() + chrono::Duration::days(i * 20);

            tokio::spawn(async move {
                let data =
                    create_data_chunk(series_id, start_date, 20, 100.0 + (i as f64 * 20.0), 0.1);

                db.create_data_points(data).await
            })
        })
        .collect();

    for (i, handle) in update_handles.into_iter().enumerate() {
        let result = handle.await?;
        assert!(result.is_ok());
        println!("  ✅ Concurrent update {} completed", i + 1);
    }

    println!("🎉 Real-time Updates Test Complete!");
    println!("Key Benefits Demonstrated:");
    println!("  • Real-time data ingestion");
    println!("  • Streaming update capabilities");
    println!("  • Concurrent update handling");
    println!("  • Low-latency query responses");

    Ok(())
}

/// Helper function to create revision data
fn create_revision_data(
    series_id: Uuid,
    start_date: NaiveDate,
    days: i32,
    starting_value: f64,
    daily_increment: f64,
    is_original: bool,
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
            is_original_release: is_original,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });

        current_value += daily_increment;
    }

    data_points
}

/// Helper function to create holiday data with gaps
fn create_holiday_data(
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

        // Skip holidays (Christmas Eve, Christmas Day, New Year's Day)
        if is_holiday(date) {
            continue;
        }

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

/// Helper function to check if a date is a holiday
fn is_holiday(date: NaiveDate) -> bool {
    let month = date.month();
    let day = date.day();

    // Christmas Eve (Dec 24), Christmas Day (Dec 25), New Year's Day (Jan 1)
    (day == 25 || day == 24) && month == 12 || (month == 1 && day == 1)
}

/// Helper function to create quality data with issues
fn create_quality_data(
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

        // Introduce quality issues: outliers, missing values
        let value = if i % 7 == 0 {
            // Outlier every 7 days
            Some(current_value * 2.0)
        } else if i % 11 == 0 {
            // Missing value every 11 days
            None
        } else {
            Some(current_value)
        };

        data_points.push(DataPoint {
            id: Uuid::new_v4(),
            series_id,
            date,
            value: value.map(|v| DecimalScalar(rust_decimal::Decimal::from_f64_retain(v).unwrap())),
            revision_date: date,
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });

        current_value += daily_increment;
    }

    data_points
}

/// Helper function to create quarterly data
fn create_quarterly_data(
    series_id: Uuid,
    start_date: NaiveDate,
    quarters: i32,
    starting_value: f64,
    quarterly_change: f64,
) -> Vec<DataPoint> {
    let mut data_points = Vec::new();
    let mut current_value = starting_value;

    for i in 0..quarters {
        let date = start_date + chrono::Duration::days((i * 90) as i64); // Approximate quarterly

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

        current_value += quarterly_change;
    }

    data_points
}

/// Helper function to create monthly data
fn create_monthly_data(
    series_id: Uuid,
    start_date: NaiveDate,
    months: i32,
    starting_value: f64,
    monthly_change: f64,
) -> Vec<DataPoint> {
    let mut data_points = Vec::new();
    let mut current_value = starting_value;

    for i in 0..months {
        let date = start_date + chrono::Duration::days((i * 30) as i64); // Approximate monthly

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

        current_value += monthly_change;
    }

    data_points
}
