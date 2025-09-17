/**
 * REQUIREMENT: Comprehensive integration tests for SearchService
 * PURPOSE: Test full-text search functionality with PostgreSQL integration
 * This ensures advanced search capabilities work correctly for economic time series data
 */

#[cfg(test)]
mod tests {
    use econ_graph_core::{
        database::{create_pool, DatabasePool},
        error::AppResult,
        models::search::{SearchParams, SearchSortOrder, SearchSuggestion, SuggestionType},
        services::search_service::SearchService,
        test_utils::TestContainer,
    };
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use serial_test::serial;
    use std::sync::Arc;
    use uuid::Uuid;

    /// Test basic search functionality with simple queries
    #[tokio::test]
    #[serial]
    async fn test_basic_search_functionality() -> AppResult<()> {
        // REQUIREMENT: Test basic search operations
        // PURPOSE: Verify that simple search queries return expected results
        // This ensures the core search functionality works correctly

        let container = TestContainer::new().await;
        let pool = container.pool();
        let search_service = SearchService::new(Arc::new(pool.clone()));

        // Create test data sources and series
        let (source_id, series_ids) = create_test_search_data(&pool).await?;

        // Test basic text search
        let search_params = SearchParams {
            query: Some("GDP".to_string()),
            source_id: None,
            category: None,
            frequency: None,
            is_active: Some(true),
            sort_by: SearchSortOrder::Relevance,
            limit: Some(10),
            offset: Some(0),
        };

        let results = search_service.search_series(&search_params).await?;

        // Should find GDP-related series
        assert!(!results.is_empty(), "Search should return results for 'GDP' query");

        // Verify results contain expected series
        let has_gdp_series = results.iter().any(|result|
            result.title.to_lowercase().contains("gdp") ||
            result.description.to_lowercase().contains("gdp")
        );
        assert!(has_gdp_series, "Results should contain GDP-related series");

        println!("✅ Basic search functionality test passed");

        Ok(())
    }

    /// Test advanced search with multiple filters
    #[tokio::test]
    #[serial]
    async fn test_advanced_search_with_filters() -> AppResult<()> {
        // REQUIREMENT: Test search with multiple filter combinations
        // PURPOSE: Verify that complex search queries with filters work correctly
        // This ensures users can find specific types of economic data

        let container = TestContainer::new().await;
        let pool = container.pool();
        let search_service = SearchService::new(Arc::new(pool.clone()));

        // Create test data
        let (source_id, series_ids) = create_test_search_data(&pool).await?;

        // Test search with source filter
        let search_params = SearchParams {
            query: Some("economic".to_string()),
            source_id: Some(source_id),
            category: None,
            frequency: Some("Monthly".to_string()),
            is_active: Some(true),
            sort_by: SearchSortOrder::Title,
            limit: Some(5),
            offset: Some(0),
        };

        let results = search_service.search_series(&search_params).await?;

        // All results should be from the specified source
        for result in &results {
            assert_eq!(result.source_id, source_id, "All results should be from specified source");
        }

        // Test search with category filter
        let search_params = SearchParams {
            query: Some("unemployment".to_string()),
            source_id: None,
            category: Some("Employment".to_string()),
            frequency: None,
            is_active: Some(true),
            sort_by: SearchSortOrder::LastUpdated,
            limit: Some(10),
            offset: Some(0),
        };

        let results = search_service.search_series(&search_params).await?;

        // Results should be employment-related
        for result in &results {
            assert!(result.category.as_ref().map_or(false, |cat| cat.contains("Employment")),
                   "Results should be in Employment category");
        }

        println!("✅ Advanced search with filters test passed");

        Ok(())
    }

    /// Test search suggestions functionality
    #[tokio::test]
    #[serial]
    async fn test_search_suggestions() -> AppResult<()> {
        // REQUIREMENT: Test search suggestion generation
        // PURPOSE: Verify that search suggestions help users find relevant data
        // This improves user experience by providing intelligent search assistance

        let container = TestContainer::new().await;
        let pool = container.pool();
        let search_service = SearchService::new(Arc::new(pool.clone()));

        // Create test data
        let (_, _) = create_test_search_data(&pool).await?;

        // Test partial query suggestions
        let suggestions = search_service.get_search_suggestions("gdp", 5).await?;

        assert!(!suggestions.is_empty(), "Should return suggestions for 'gdp'");

        // Verify suggestion types
        let has_series_suggestions = suggestions.iter().any(|s| matches!(s.suggestion_type, SuggestionType::Series));
        let has_category_suggestions = suggestions.iter().any(|s| matches!(s.suggestion_type, SuggestionType::Category));

        assert!(has_series_suggestions || has_category_suggestions,
               "Should return series or category suggestions");

        // Test empty query suggestions (popular searches)
        let popular_suggestions = search_service.get_search_suggestions("", 10).await?;

        assert!(!popular_suggestions.is_empty(), "Should return popular search suggestions");

        println!("✅ Search suggestions test passed");

        Ok(())
    }

    /// Test search analytics and statistics
    #[tokio::test]
    #[serial]
    async fn test_search_analytics() -> AppResult<()> {
        // REQUIREMENT: Test search analytics and statistics tracking
        // PURPOSE: Verify that search usage is properly tracked for insights
        // This enables understanding of user search patterns and popular data

        let container = TestContainer::new().await;
        let pool = container.pool();
        let search_service = SearchService::new(Arc::new(pool.clone()));

        // Create test data
        let (_, series_ids) = create_test_search_data(&pool).await?;

        // Perform some searches to generate analytics data
        let search_params = SearchParams {
            query: Some("GDP".to_string()),
            source_id: None,
            category: None,
            frequency: None,
            is_active: Some(true),
            sort_by: SearchSortOrder::Relevance,
            limit: Some(10),
            offset: Some(0),
        };

        let _results = search_service.search_series(&search_params).await?;

        // Test search statistics
        let stats = search_service.get_search_statistics().await?;

        assert!(stats.total_searches >= 1, "Should track search count");
        assert!(!stats.popular_queries.is_empty(), "Should track popular queries");
        assert!(!stats.popular_categories.is_empty(), "Should track popular categories");

        // Test search analytics for specific time period
        let recent_stats = search_service.get_search_analytics(
            chrono::Utc::now() - chrono::Duration::days(7),
            chrono::Utc::now()
        ).await?;

        assert!(recent_stats.total_searches >= 0, "Should return recent search statistics");

        println!("✅ Search analytics test passed");

        Ok(())
    }

    /// Test search performance with large datasets
    #[tokio::test]
    #[serial]
    async fn test_search_performance() -> AppResult<()> {
        // REQUIREMENT: Test search performance with realistic data volumes
        // PURPOSE: Verify that search remains fast with large datasets
        // This ensures good user experience even with extensive economic data

        let container = TestContainer::new().await;
        let pool = container.pool();
        let search_service = SearchService::new(Arc::new(pool.clone()));

        // Create larger test dataset
        let (_, _) = create_large_test_search_data(&pool).await?;

        // Test search performance
        let start_time = std::time::Instant::now();

        let search_params = SearchParams {
            query: Some("economic".to_string()),
            source_id: None,
            category: None,
            frequency: None,
            is_active: Some(true),
            sort_by: SearchSortOrder::Relevance,
            limit: Some(50),
            offset: Some(0),
        };

        let results = search_service.search_series(&search_params).await?;

        let search_duration = start_time.elapsed();

        // Search should complete within reasonable time (adjust threshold as needed)
        assert!(search_duration.as_millis() < 1000,
               "Search should complete within 1 second, took {:?}", search_duration);

        assert!(!results.is_empty(), "Should return results for large dataset");

        println!("✅ Search performance test passed - completed in {:?}", search_duration);

        Ok(())
    }

    /// Test search error handling
    #[tokio::test]
    #[serial]
    async fn test_search_error_handling() -> AppResult<()> {
        // REQUIREMENT: Test search error handling and edge cases
        // PURPOSE: Verify that search handles invalid inputs gracefully
        // This ensures robust error handling for user inputs

        let container = TestContainer::new().await;
        let pool = container.pool();
        let search_service = SearchService::new(Arc::new(pool.clone()));

        // Test with invalid parameters
        let invalid_params = SearchParams {
            query: Some("".to_string()), // Empty query
            source_id: Some(Uuid::new_v4()), // Non-existent source
            category: None,
            frequency: None,
            is_active: Some(true),
            sort_by: SearchSortOrder::Relevance,
            limit: Some(0), // Invalid limit
            offset: Some(-1), // Invalid offset
        };

        // Should handle invalid parameters gracefully
        let results = search_service.search_series(&invalid_params).await?;

        // Should return empty results rather than error
        assert!(results.is_empty(), "Should return empty results for invalid parameters");

        // Test with very long query
        let long_query = "a".repeat(1000);
        let long_params = SearchParams {
            query: Some(long_query),
            source_id: None,
            category: None,
            frequency: None,
            is_active: Some(true),
            sort_by: SearchSortOrder::Relevance,
            limit: Some(10),
            offset: Some(0),
        };

        let results = search_service.search_series(&long_params).await?;

        // Should handle long queries without crashing
        assert!(results.is_empty(), "Should handle long queries gracefully");

        println!("✅ Search error handling test passed");

        Ok(())
    }

    /// Test search result ranking and relevance
    #[tokio::test]
    #[serial]
    async fn test_search_result_ranking() -> AppResult<()> {
        // REQUIREMENT: Test search result ranking and relevance scoring
        // PURPOSE: Verify that most relevant results appear first
        // This ensures users find the most useful data quickly

        let container = TestContainer::new().await;
        let pool = container.pool();
        let search_service = SearchService::new(Arc::new(pool.clone()));

        // Create test data with known relevance
        let (_, _) = create_test_search_data(&pool).await?;

        // Test relevance-based ranking
        let search_params = SearchParams {
            query: Some("GDP".to_string()),
            source_id: None,
            category: None,
            frequency: None,
            is_active: Some(true),
            sort_by: SearchSortOrder::Relevance,
            limit: Some(10),
            offset: Some(0),
        };

        let results = search_service.search_series(&search_params).await?;

        if results.len() > 1 {
            // First result should have higher relevance score than second
            assert!(results[0].relevance_score >= results[1].relevance_score,
                   "Results should be ranked by relevance");
        }

        // Test title-based sorting
        let title_params = SearchParams {
            query: Some("economic".to_string()),
            source_id: None,
            category: None,
            frequency: None,
            is_active: Some(true),
            sort_by: SearchSortOrder::Title,
            limit: Some(10),
            offset: Some(0),
        };

        let title_results = search_service.search_series(&title_params).await?;

        if title_results.len() > 1 {
            // Results should be sorted alphabetically by title
            assert!(title_results[0].title <= title_results[1].title,
                   "Results should be sorted by title");
        }

        println!("✅ Search result ranking test passed");

        Ok(())
    }

    /// Create test data for search tests
    async fn create_test_search_data(pool: &DatabasePool) -> AppResult<(Uuid, Vec<Uuid>)> {
        use econ_graph_core::schema::{data_sources, economic_series};
        use econ_graph_core::models::{NewDataSource, NewEconomicSeries};

        let mut conn = pool.get().await?;

        // Create test data source
        let new_source = NewDataSource {
            name: "Test Search Source".to_string(),
            description: Some("Test data source for search tests".to_string()),
            base_url: "https://test-search.example.com".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
        };

        let source = diesel::insert_into(data_sources::table)
            .values(&new_source)
            .returning(econ_graph_core::models::DataSource::as_select())
            .get_result::<econ_graph_core::models::DataSource>(&mut conn)
            .await?;

        // Create test economic series
        let test_series = vec![
            NewEconomicSeries {
                title: "Real Gross Domestic Product".to_string(),
                description: "Real GDP measures the inflation-adjusted value of all goods and services produced".to_string(),
                source_id: source.id,
                frequency: "Quarterly".to_string(),
                units: "Billions of Chained 2017 Dollars".to_string(),
                category: Some("GDP".to_string()),
                is_active: true,
                start_date: Some(chrono::NaiveDate::from_ymd_opt(1947, 1, 1).unwrap()),
                end_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 9, 30).unwrap()),
                last_updated: Some(chrono::Utc::now()),
            },
            NewEconomicSeries {
                title: "Unemployment Rate".to_string(),
                description: "Percent of labor force that is unemployed".to_string(),
                source_id: source.id,
                frequency: "Monthly".to_string(),
                units: "Percent".to_string(),
                category: Some("Employment".to_string()),
                is_active: true,
                start_date: Some(chrono::NaiveDate::from_ymd_opt(1948, 1, 1).unwrap()),
                end_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 11, 1).unwrap()),
                last_updated: Some(chrono::Utc::now()),
            },
            NewEconomicSeries {
                title: "Consumer Price Index".to_string(),
                description: "Measure of average change in prices paid by urban consumers".to_string(),
                source_id: source.id,
                frequency: "Monthly".to_string(),
                units: "Index 1982-84=100".to_string(),
                category: Some("Inflation".to_string()),
                is_active: true,
                start_date: Some(chrono::NaiveDate::from_ymd_opt(1913, 1, 1).unwrap()),
                end_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 11, 1).unwrap()),
                last_updated: Some(chrono::Utc::now()),
            },
        ];

        let mut series_ids = Vec::new();
        for series in test_series {
            let created_series = diesel::insert_into(economic_series::table)
                .values(&series)
                .returning(econ_graph_core::models::EconomicSeries::as_select())
                .get_result::<econ_graph_core::models::EconomicSeries>(&mut conn)
                .await?;
            series_ids.push(created_series.id);
        }

        Ok((source.id, series_ids))
    }

    /// Create larger test dataset for performance testing
    async fn create_large_test_search_data(pool: &DatabasePool) -> AppResult<(Uuid, Vec<Uuid>)> {
        use econ_graph_core::schema::{data_sources, economic_series};
        use econ_graph_core::models::{NewDataSource, NewEconomicSeries};

        let mut conn = pool.get().await?;

        // Create test data source
        let new_source = NewDataSource {
            name: "Large Test Search Source".to_string(),
            description: Some("Large test data source for performance tests".to_string()),
            base_url: "https://large-test-search.example.com".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
        };

        let source = diesel::insert_into(data_sources::table)
            .values(&new_source)
            .returning(econ_graph_core::models::DataSource::as_select())
            .get_result::<econ_graph_core::models::DataSource>(&mut conn)
            .await?;

        // Create many test series for performance testing
        let mut test_series = Vec::new();
        let categories = vec!["GDP", "Employment", "Inflation", "Interest Rates", "Trade", "Production"];
        let frequencies = vec!["Daily", "Weekly", "Monthly", "Quarterly", "Annual"];

        for i in 0..100 {
            let category = categories[i % categories.len()];
            let frequency = frequencies[i % frequencies.len()];

            test_series.push(NewEconomicSeries {
                title: format!("Test Economic Series {} - {}", i + 1, category),
                description: format!("Test description for economic series {} in category {}", i + 1, category),
                source_id: source.id,
                frequency: frequency.to_string(),
                units: "Various Units".to_string(),
                category: Some(category.to_string()),
                is_active: true,
                start_date: Some(chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()),
                end_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 12, 1).unwrap()),
                last_updated: Some(chrono::Utc::now()),
            });
        }

        let mut series_ids = Vec::new();
        for series in test_series {
            let created_series = diesel::insert_into(economic_series::table)
                .values(&series)
                .returning(econ_graph_core::models::EconomicSeries::as_select())
                .get_result::<econ_graph_core::models::EconomicSeries>(&mut conn)
                .await?;
            series_ids.push(created_series.id);
        }

        Ok((source.id, series_ids))
    }
}
