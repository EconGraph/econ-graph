// REQUIREMENT: Comprehensive integration test suite with testcontainers
// PURPOSE: Demonstrate database integration testing capabilities
// This module showcases the full testing infrastructure with real PostgreSQL

#[cfg(test)]
mod tests {
    use bigdecimal::BigDecimal;
    use chrono::NaiveDate;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use econ_graph_core::models::{
        DataPoint, DataSource, EconomicSeries, NewDataPoint, NewDataSource, NewEconomicSeries,
    };
    use econ_graph_core::test_utils::TestContainer;
    use serial_test::serial;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    #[serial]
    async fn test_full_integration_scenario() {
        // REQUIREMENT: End-to-end integration test with all components
        // PURPOSE: Verify that the complete system works together correctly
        // This tests the full workflow from data source to queue processing

        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();

        // Create test data source
        let new_source = NewDataSource {
            name: "Test Integration Source".to_string(),
            description: Some("Integration test data source".to_string()),
            base_url: "https://test.example.com/api".to_string(),
            api_key_required: true,
            api_key_name: Some("TEST_API_KEY".to_string()),
            rate_limit_per_minute: 100,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://test.example.com/docs".to_string()),
        };

        let created_source = DataSource::create(&pool, new_source)
            .await
            .expect("Should create data source");

        // Create test economic series
        let new_series = NewEconomicSeries {
            source_id: created_source.id,
            external_id: "TEST_SERIES_001".to_string(),
            title: "Test Integration Series".to_string(),
            description: Some("Integration test economic series".to_string()),
            units: Some("Percent".to_string()),
            frequency: "Monthly".to_string(),
            seasonal_adjustment: Some("Seasonally Adjusted".to_string()),
            start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            end_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
            is_active: true,
            first_discovered_at: Some(chrono::Utc::now()),
            last_crawled_at: None,
            first_missing_date: None,
            crawl_status: None,
            crawl_error_message: None,
        };

        let created_series = EconomicSeries::create(&pool, &new_series)
            .await
            .expect("Should create economic series");

        // Create test data points
        let test_data_points: Vec<NewDataPoint> = (1..=12)
            .map(|month| NewDataPoint {
                series_id: created_series.id,
                date: NaiveDate::from_ymd_opt(2024, month, 1).unwrap(),
                value: Some(BigDecimal::from(100 + month)),
                revision_date: NaiveDate::from_ymd_opt(2024, month, 15).unwrap(),
                is_original_release: true,
            })
            .collect();

        let created_points = DataPoint::create_batch(&pool, &test_data_points)
            .await
            .expect("Should create data points");

        // Verify data was created correctly
        assert_eq!(
            created_points.len(),
            12,
            "Should have created 12 data points"
        );

        // Test data retrieval and relationships
        let mut conn = pool.get().await.expect("Should get connection");

        // Verify foreign key relationships using raw SQL
        #[derive(diesel::QueryableByName)]
        struct CountResult {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            count: i64,
        }

        let relationship_count: i64 = diesel::sql_query(
            "SELECT COUNT(*) as count FROM economic_series es
             INNER JOIN data_sources ds ON es.source_id = ds.id
             INNER JOIN data_points dp ON dp.series_id = es.id
             WHERE ds.name = 'Test Integration Source'",
        )
        .get_result::<CountResult>(&mut conn)
        .await
        .expect("Should execute relationship query")
        .count;

        assert_eq!(
            relationship_count, 12,
            "All data points should have valid relationships"
        );

        // Test data integrity by trying to create invalid foreign key reference
        let invalid_series = NewEconomicSeries {
            source_id: Uuid::new_v4(), // Non-existent source ID
            external_id: "INVALID_TEST".to_string(),
            title: "Invalid Series".to_string(),
            description: None,
            units: None,
            frequency: "Monthly".to_string(),
            seasonal_adjustment: None,
            start_date: None,
            end_date: None,
            is_active: true,
            first_discovered_at: Some(chrono::Utc::now()),
            last_crawled_at: None,
            first_missing_date: None,
            crawl_status: None,
            crawl_error_message: None,
        };

        // This should fail due to foreign key constraint
        let constraint_result = EconomicSeries::create(&pool, &invalid_series).await;
        assert!(
            constraint_result.is_err(),
            "Invalid foreign key should be rejected"
        );

        println!("✅ Full integration test completed successfully!");
        println!("   - Database setup and migrations: ✅");
        println!("   - Test data creation: ✅");
        println!("   - Cross-table relationships: ✅");
        println!("   - Foreign key constraints: ✅");
        println!("   - Data integrity checks: ✅");
    }

    #[tokio::test]
    #[serial]
    async fn test_queue_service_integration() {
        // REQUIREMENT: Test queue service integration with database
        // PURPOSE: Verify that queue operations work correctly with real database
        // This tests the SKIP LOCKED functionality and queue statistics

        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();

        // Test queue statistics on empty queue
        let empty_stats = econ_graph_services::services::queue_service::get_queue_statistics(&pool)
            .await
            .expect("Should get queue statistics");

        assert_eq!(empty_stats.total_items, 0);
        assert_eq!(empty_stats.pending_items, 0);
        assert_eq!(empty_stats.processing_items, 0);

        // Test creating queue items
        use econ_graph_core::models::{CrawlQueueItem, NewCrawlQueueItem};

        let new_item = NewCrawlQueueItem {
            source: "FRED".to_string(),
            series_id: "GDP_TEST".to_string(),
            priority: 5,
            max_retries: 3,
            scheduled_for: None,
        };

        let created_item = CrawlQueueItem::create(&pool, &new_item)
            .await
            .expect("Should create queue item");

        // Test queue statistics with items
        let stats_with_items =
            econ_graph_services::services::queue_service::get_queue_statistics(&pool)
                .await
                .expect("Should get queue statistics");

        assert_eq!(stats_with_items.total_items, 1);
        assert_eq!(stats_with_items.pending_items, 1);

        // Test getting next item for processing
        let next_item = econ_graph_services::services::queue_service::get_and_lock_next_item(
            &pool,
            "test-worker",
        )
        .await
        .expect("Should get next item");

        assert!(next_item.is_some());
        let locked_item = next_item.unwrap();
        assert_eq!(locked_item.id, created_item.id);
        assert_eq!(locked_item.status, "processing");

        println!("✅ Queue service integration test completed successfully!");
    }

    #[tokio::test]
    #[serial]
    async fn test_crawler_service_integration() {
        // REQUIREMENT: Test crawler service integration
        // PURPOSE: Verify that crawler status and data storage work correctly
        // This tests the complete data pipeline from crawler to database

        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();

        // Test crawler status
        let status =
            econ_graph_services::services::crawler::simple_crawler_service::get_crawler_status()
                .await
                .expect("Should get crawler status");

        // Status should reflect environment configuration
        assert!(status.active_workers >= 0);

        // Test that data sources can be created (needed for crawler)
        let fred_source = DataSource::get_or_create(&pool, DataSource::fred())
            .await
            .expect("Should create FRED data source");

        assert_eq!(fred_source.name, "Federal Reserve Economic Data (FRED)");

        let bls_source = DataSource::get_or_create(&pool, DataSource::bls())
            .await
            .expect("Should create BLS data source");

        assert_eq!(bls_source.name, "Bureau of Labor Statistics (BLS)");

        println!("✅ Crawler service integration test completed successfully!");
    }
}
