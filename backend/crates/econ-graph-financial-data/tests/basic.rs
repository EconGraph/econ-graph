use chrono::{NaiveDate, Utc};
use econ_graph_financial_data::crawler::CrawlerWriteApi;
use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
use rust_decimal::Decimal;
use uuid::Uuid;

#[tokio::test]
async fn test_crate_initialization() {
    // Test that we can create the basic components
    let database = Database::new_in_memory()
        .await
        .expect("Database should initialize");
    let crawler_api = CrawlerWriteApi::new();

    // Test that we can create data models
    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "TEST_SERIES_001".to_string(),
        title: "Test Economic Series".to_string(),
        description: Some("A test series for unit testing".to_string()),
        units: Some("USD".to_string()),
        frequency: "monthly".to_string(),
        seasonal_adjustment: Some("seasonally_adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let data_point = DataPoint {
        id: Uuid::new_v4(),
        series_id: series.id,
        date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        value: Some(DecimalScalar(Decimal::new(1000, 0))),
        revision_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        is_original_release: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Test serialization/deserialization
    let series_json = serde_json::to_string(&series).expect("Should serialize");
    let _: EconomicSeries = serde_json::from_str(&series_json).expect("Should deserialize");

    let point_json = serde_json::to_string(&data_point).expect("Should serialize");
    let _: DataPoint = serde_json::from_str(&point_json).expect("Should deserialize");

    println!("✅ Crate initialization test passed");
}

#[tokio::test]
async fn test_database_operations() {
    let mut database = Database::new_in_memory()
        .await
        .expect("Database should initialize");

    let series = EconomicSeries {
        id: Uuid::new_v4(),
        source_id: Uuid::new_v4(),
        external_id: "TEST_SERIES_002".to_string(),
        title: "Test Database Series".to_string(),
        description: None,
        units: Some("USD".to_string()),
        frequency: "daily".to_string(),
        seasonal_adjustment: None,
        start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        end_date: None,
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Test creating a series
    database
        .create_series(series.clone())
        .await
        .expect("Should create series");

    // Test retrieving the series
    let retrieved = database
        .get_series(series.id)
        .await
        .expect("Should retrieve series");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().title, "Test Database Series");

    println!("✅ Database operations test passed");
}

#[tokio::test]
async fn test_crawler_api() {
    let crawler_api = CrawlerWriteApi::new();

    let payload = econ_graph_financial_data::crawler::CleanDataPayload {
        series_id: Uuid::new_v4(),
        data_points: vec![DataPoint {
            id: Uuid::new_v4(),
            series_id: Uuid::new_v4(),
            date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            value: Some(DecimalScalar(Decimal::new(1000, 0))),
            revision_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }],
        metadata: Some(serde_json::json!({"source": "test"})),
    };

    let response = crawler_api
        .process_clean_data(payload)
        .await
        .expect("Should process clean data");
    assert!(response.success);
    assert_eq!(response.data_points_processed, 1);

    println!("✅ Crawler API test passed");
}
