use chrono::{NaiveDate, Utc};
use econ_graph_financial_data::catalog::IcebergCatalog;
use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::models::validation::Validate;
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// Test concurrent catalog operations
#[tokio::test]
async fn test_concurrent_catalog_updates() {
    let temp_dir = tempfile::tempdir().unwrap();
    let catalog = Arc::new(
        IcebergCatalog::new(temp_dir.path().to_path_buf())
            .await
            .unwrap(),
    );

    let num_tasks = 10;
    let series_per_task = 5;

    // Spawn concurrent tasks that add series to the catalog
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let catalog = Arc::clone(&catalog);
        let handle = tokio::spawn(async move {
            for series_idx in 0..series_per_task {
                let series_metadata = econ_graph_financial_data::catalog::SeriesMetadata {
                    series_id: Uuid::new_v4(),
                    external_id: format!("TASK_{}_SERIES_{}", task_id, series_idx),
                    title: format!("Test Series {} from Task {}", series_idx, task_id),
                    description: Some(format!(
                        "Description for series {} from task {}",
                        series_idx, task_id
                    )),
                    units: Some("Units".to_string()),
                    frequency: "monthly".to_string(),
                    seasonal_adjustment: Some("seasonally_adjusted".to_string()),
                    source: "Test Source".to_string(),
                    is_active: true,
                    start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                    end_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
                    total_points: 0,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };

                catalog.add_series(series_metadata).await.unwrap();

                // Small delay to increase chance of concurrent access
                sleep(Duration::from_millis(10)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all series were added
    let all_series = catalog.list_series().await.unwrap();
    assert_eq!(all_series.len(), num_tasks * series_per_task);

    // Verify no duplicates (each external_id should be unique)
    let mut external_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for series in &all_series {
        assert!(
            external_ids.insert(series.external_id.clone()),
            "Duplicate external_id found: {}",
            series.external_id
        );
    }
}

/// Test concurrent read operations
#[tokio::test]
async fn test_concurrent_catalog_reads() {
    let temp_dir = tempfile::tempdir().unwrap();
    let catalog = Arc::new(
        IcebergCatalog::new(temp_dir.path().to_path_buf())
            .await
            .unwrap(),
    );

    // Add some test series first
    let _series_ids: Vec<Uuid> = vec![];
    for i in 0..10 {
        let series_metadata = econ_graph_financial_data::catalog::SeriesMetadata {
            series_id: Uuid::new_v4(),
            external_id: format!("READ_TEST_SERIES_{}", i),
            title: format!("Read Test Series {}", i),
            description: Some(format!("Description for read test series {}", i)),
            units: Some("Units".to_string()),
            frequency: "monthly".to_string(),
            seasonal_adjustment: Some("seasonally_adjusted".to_string()),
            source: "Test Source".to_string(),
            is_active: true,
            start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            end_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
            total_points: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        catalog.add_series(series_metadata).await.unwrap();
    }

    let num_readers = 20;
    let mut handles = vec![];

    // Spawn concurrent readers
    for reader_id in 0..num_readers {
        let catalog = Arc::clone(&catalog);
        let handle = tokio::spawn(async move {
            for _ in 0..5 {
                // Test list_series
                let series_list = catalog.list_series().await.unwrap();
                assert!(!series_list.is_empty());

                // Test get_stats
                let stats = catalog.get_stats().await.unwrap();
                assert!(stats.total_series > 0);

                sleep(Duration::from_millis(5)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all readers to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

/// Test concurrent write operations with data validation
#[tokio::test]
async fn test_concurrent_data_validation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let database = Database::new_with_parquet(temp_dir.path().to_string_lossy().to_string())
        .await
        .unwrap();

    let num_tasks = 5;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let database = Arc::new(database.clone());
        let handle = tokio::spawn(async move {
            for series_idx in 0..3 {
                let series = EconomicSeries {
                    id: Uuid::new_v4(),
                    source_id: Uuid::new_v4(),
                    external_id: format!("VALIDATION_TASK_{}_SERIES_{}", task_id, series_idx),
                    title: format!(
                        "Validation Test Series {} from Task {}",
                        series_idx, task_id
                    ),
                    description: Some(format!(
                        "Description for validation test series {} from task {}",
                        series_idx, task_id
                    )),
                    units: Some("Units".to_string()),
                    frequency: "monthly".to_string(),
                    seasonal_adjustment: Some("seasonally_adjusted".to_string()),
                    start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                    end_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
                    is_active: true,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };

                // Validate the series
                series.validate().unwrap();

                // Add some data points
                let mut data_points = vec![];
                for day in 1..=5 {
                    let data_point = DataPoint {
                        id: Uuid::new_v4(),
                        series_id: series.id,
                        date: NaiveDate::from_ymd_opt(2024, 1, day).unwrap(),
                        value: Some(DecimalScalar(
                            Decimal::from_str(&format!("{}.{}", day * 10, day)).unwrap(),
                        )),
                        revision_date: NaiveDate::from_ymd_opt(2024, 1, day).unwrap(),
                        is_original_release: true,
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                    };

                    // Validate the data point
                    data_point.validate().unwrap();
                    data_points.push(data_point);
                }

                // Store the series and data points concurrently
                database.create_series(series.clone()).await.unwrap();
                database.create_data_points(data_points).await.unwrap();

                sleep(Duration::from_millis(10)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all series were created
    let all_series = database.list_series().await.unwrap();
    assert_eq!(all_series.len(), num_tasks * 3);
}

/// Test error recovery scenarios
#[tokio::test]
async fn test_catalog_consistency_under_load() {
    let temp_dir = tempfile::tempdir().unwrap();
    let catalog = Arc::new(
        IcebergCatalog::new(temp_dir.path().to_path_buf())
            .await
            .unwrap(),
    );

    let num_tasks = 8;
    let mut handles = vec![];

    // Mix of read and write operations
    for task_id in 0..num_tasks {
        let catalog = Arc::clone(&catalog);
        let handle = tokio::spawn(async move {
            for operation in 0..10 {
                if task_id % 2 == 0 {
                    // Write operation
                    let series_metadata = econ_graph_financial_data::catalog::SeriesMetadata {
                        series_id: Uuid::new_v4(),
                        external_id: format!("LOAD_TEST_TASK_{}_OP_{}", task_id, operation),
                        title: format!("Load Test Series {} Op {}", task_id, operation),
                        description: Some("Load test description".to_string()),
                        units: Some("Units".to_string()),
                        frequency: "monthly".to_string(),
                        seasonal_adjustment: Some("seasonally_adjusted".to_string()),
                        source: "Load Test Source".to_string(),
                        is_active: true,
                        start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                        end_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
                        total_points: 0,
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                    };

                    catalog.add_series(series_metadata).await.unwrap();
                } else {
                    // Read operation
                    let series_list = catalog.list_series().await.unwrap();
                    let stats = catalog.get_stats().await.unwrap();

                    // Verify consistency: stats should match actual series count
                    assert_eq!(stats.total_series as usize, series_list.len());
                }

                sleep(Duration::from_millis(5)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Final consistency check
    let final_stats = catalog.get_stats().await.unwrap();
    let final_series = catalog.list_series().await.unwrap();
    assert_eq!(final_stats.total_series as usize, final_series.len());
}

/// Test concurrent access with invalid data (should not corrupt catalog)
#[tokio::test]
async fn test_concurrent_invalid_data_handling() {
    let temp_dir = tempfile::tempdir().unwrap();
    let database = Database::new_with_parquet(temp_dir.path().to_string_lossy().to_string())
        .await
        .unwrap();

    let num_tasks = 3;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let database = Arc::new(database.clone());
        let handle = tokio::spawn(async move {
            // Mix of valid and invalid data
            for series_idx in 0..2 {
                if task_id == 0 {
                    // Valid series
                    let series = EconomicSeries {
                        id: Uuid::new_v4(),
                        source_id: Uuid::new_v4(),
                        external_id: format!("VALID_TASK_{}_SERIES_{}", task_id, series_idx),
                        title: format!("Valid Series {}", series_idx),
                        description: Some("Valid description".to_string()),
                        units: Some("Units".to_string()),
                        frequency: "monthly".to_string(),
                        seasonal_adjustment: Some("seasonally_adjusted".to_string()),
                        start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                        end_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
                        is_active: true,
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                    };

                    series.validate().unwrap();
                    database.create_series(series).await.unwrap();
                } else {
                    // Invalid series (should be rejected by validation)
                    let invalid_series = EconomicSeries {
                        id: Uuid::new_v4(),
                        source_id: Uuid::new_v4(),
                        external_id: "invalid id with spaces".to_string(), // Invalid external ID
                        title: "".to_string(),                             // Empty title
                        description: None,
                        units: None,
                        frequency: "invalid_frequency".to_string(), // Invalid frequency
                        seasonal_adjustment: Some("invalid_adjustment".to_string()),
                        start_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
                        end_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()), // End before start
                        is_active: true,
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                    };

                    // This should fail validation
                    assert!(invalid_series.validate().is_err());
                }

                sleep(Duration::from_millis(10)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify only valid series were created
    let all_series = database.list_series().await.unwrap();
    assert_eq!(all_series.len(), 2); // Only valid series from task 0
}
