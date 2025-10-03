use chrono::{NaiveDate, Utc};
use econ_graph_financial_data::crawler::CrawlerWriteApi;
use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::models::input::{CreateDataPointInput, CreateEconomicSeriesInput};
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
use rust_decimal::Decimal;
use uuid::Uuid;

#[tokio::test]
async fn test_complete_service_workflow() {
    // Initialize the service components
    let database = Database::new_in_memory()
        .await
        .expect("Database should initialize");
    let crawler_api = CrawlerWriteApi::new();

    // Test 1: Create a series through the database
    let series_id = Uuid::new_v4();
    let source_id = Uuid::new_v4();

    let series = EconomicSeries {
        id: series_id,
        source_id,
        external_id: "INTEGRATION_TEST_001".to_string(),
        title: "Integration Test Series".to_string(),
        description: Some("A series for integration testing".to_string()),
        units: Some("USD".to_string()),
        frequency: "monthly".to_string(),
        seasonal_adjustment: Some("seasonally_adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Store the series
    database
        .create_series(series.clone())
        .await
        .expect("Should create series");

    // Test 2: Retrieve the series
    let retrieved_series = database
        .get_series(series_id)
        .await
        .expect("Should retrieve series");
    assert!(retrieved_series.is_some());
    let retrieved = retrieved_series.unwrap();
    assert_eq!(retrieved.title, "Integration Test Series");
    assert_eq!(retrieved.external_id, "INTEGRATION_TEST_001");

    // Test 3: Create data points
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
            value: Some(DecimalScalar(Decimal::new(1100, 0))),
            revision_date: NaiveDate::from_ymd_opt(2023, 2, 1).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    database
        .create_data_points(data_points.clone())
        .await
        .expect("Should create data points");

    // Test 4: Retrieve data points with date filtering
    let all_points = database
        .get_data_points(series_id, None, None)
        .await
        .expect("Should retrieve all points");
    assert_eq!(all_points.len(), 2);

    let filtered_points = database
        .get_data_points(
            series_id,
            Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2023, 1, 31).unwrap()),
        )
        .await
        .expect("Should retrieve filtered points");
    assert_eq!(filtered_points.len(), 1);
    assert_eq!(
        filtered_points[0].date,
        NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()
    );

    // Test 5: Test crawler API
    let crawler_payload = econ_graph_financial_data::crawler::CleanDataPayload {
        series_id,
        data_points: data_points.clone(),
        metadata: Some(serde_json::json!({
            "source": "integration_test",
            "crawler_version": "1.0.0"
        })),
    };

    let crawler_response = crawler_api
        .process_clean_data(crawler_payload)
        .await
        .expect("Should process clean data");
    assert!(crawler_response.success);
    assert_eq!(crawler_response.series_id, series_id);
    assert_eq!(crawler_response.data_points_processed, 2);

    println!("✅ Complete service workflow test passed");
}

#[tokio::test]
async fn test_data_validation() {
    let database = Database::new_in_memory()
        .await
        .expect("Database should initialize");

    // Test creating series with minimal required fields
    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "MINIMAL_TEST".to_string(),
        title: "Minimal Series".to_string(),
        description: None,
        units: None,
        frequency: "daily".to_string(),
        seasonal_adjustment: None,
        start_date: None,
        end_date: None,
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    database
        .create_series(series.clone())
        .await
        .expect("Should create minimal series");

    // Test data point with null value
    let data_point = DataPoint {
        id: Uuid::new_v4(),
        series_id: series.id,
        date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        value: None, // Null value
        revision_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        is_original_release: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    database
        .create_data_points(vec![data_point])
        .await
        .expect("Should create data point with null value");

    println!("✅ Data validation test passed");
}

#[tokio::test]
async fn test_concurrent_operations() {
    let database = Database::new_in_memory()
        .await
        .expect("Database should initialize");

    // Test concurrent series creation
    let mut handles = vec![];

    for i in 0..10 {
        let db = database.clone();
        let handle = tokio::spawn(async move {
            let series = EconomicSeries {
                id: Uuid::new_v4(),
                source_id: Uuid::new_v4(),
                external_id: format!("CONCURRENT_TEST_{}", i),
                title: format!("Concurrent Series {}", i),
                description: None,
                units: None,
                frequency: "monthly".to_string(),
                seasonal_adjustment: None,
                start_date: None,
                end_date: None,
                is_active: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            db.create_series(series).await
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    for handle in handles {
        handle
            .await
            .expect("Task should complete")
            .expect("Should create series");
    }

    println!("✅ Concurrent operations test passed");
}
