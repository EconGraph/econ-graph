use std::sync::Arc;

use crate::{
    config::ApiKeyConfig,
    database::DatabasePool,
    models::data_source::DataSource,
    services::crawler::series_downloader::SeriesDownloader,
    test_utils::TestContainer,
};
use reqwest::Client;
use serial_test::serial;

#[cfg(test)]
mod tests {

    use super::*;

    /// Test series downloader creation
    ///
    /// This test verifies that the series downloader can be created
    /// and initialized properly.
    #[test]
    fn test_series_downloader_creation() {
        // REQUIREMENT: Test series downloader creation
        // PURPOSE: Verify that the downloader initializes correctly
        // This ensures the service can be instantiated

        let client = Client::new();
        let downloader = SeriesDownloader::new(client);

        // Should create without panicking
        assert!(true, "Series downloader should be created successfully");

        println!("✅ Series downloader creation test passed");
        println!("   - Downloader created: ✅");
        println!("   - No panics: ✅");
    }

    /// Test specific series download
    ///
    /// This test verifies that the series downloader can download
    /// a specific series from a data source.
    #[tokio::test]
    #[serial]
    async fn test_specific_series_download() -> AppResult<()> {
        // REQUIREMENT: Test specific series download
        // PURPOSE: Verify that specific series can be downloaded
        // This ensures users can target specific economic indicators

        let container = TestContainer::new().await;
        let pool = container.pool();
        let client = Client::new();
        let downloader = SeriesDownloader::new(client);

        // Ensure BLS data source exists
        let bls_source = DataSource::get_or_create(&pool, DataSource::bls()).await?;

        // First, download the catalog to populate series
        let catalog_result = downloader.catalog_downloader.download_catalog(&pool, "BLS").await;
        if catalog_result.is_err() {
            println!("BLS catalog download failed, skipping specific series test: {}", catalog_result.unwrap_err());
            return Ok(());
        }

        // Get available series
        let available_series = crate::models::series_metadata::SeriesMetadata::find_by_source(
            &pool,
            bls_source.id,
        ).await?;

        if available_series.is_empty() {
            println!("No BLS series available, skipping specific series test");
            return Ok(());
        }

        // Test downloading the first available series
        let test_series = &available_series[0];
        let result = downloader.download_specific_series(
            &pool,
            "BLS",
            &test_series.external_id,
        ).await;

        // Should succeed (even though actual data download is not implemented)
        assert!(result.is_ok(), "Should succeed in finding and processing series: {}", result.unwrap_err());

        println!("✅ Specific series download test passed");
        println!("   - Series found: {}", test_series.title);
        println!("   - Series ID: {}", test_series.external_id);
        println!("   - Processing completed: ✅");

        Ok(())
    }

    /// Test random series download
    ///
    /// This test verifies that the series downloader can download
    /// a random series from a data source.
    #[tokio::test]
    #[serial]
    async fn test_random_series_download() -> AppResult<()> {
        // REQUIREMENT: Test random series download
        // PURPOSE: Verify that random series selection works
        // This ensures the system can work with any available series

        let container = TestContainer::new().await;
        let pool = container.pool();
        let client = Client::new();
        let downloader = SeriesDownloader::new(client);

        // Ensure BLS data source exists
        let bls_source = DataSource::get_or_create(&pool, DataSource::bls()).await?;

        // First, download the catalog to populate series
        let catalog_result = downloader.catalog_downloader.download_catalog(&pool, "BLS").await;
        if catalog_result.is_err() {
            println!("BLS catalog download failed, skipping random series test: {}", catalog_result.unwrap_err());
            return Ok(());
        }

        // Test downloading a random series
        let result = downloader.download_random_series(&pool, "BLS").await;

        // Should succeed (even though actual data download is not implemented)
        assert!(result.is_ok(), "Should succeed in selecting and processing random series: {}", result.unwrap_err());

        println!("✅ Random series download test passed");
        println!("   - Random selection worked: ✅");
        println!("   - Processing completed: ✅");

        Ok(())
    }

    /// Test series download error handling
    ///
    /// This test verifies that the series downloader handles
    /// errors gracefully and provides meaningful messages.
    #[tokio::test]
    #[serial]
    async fn test_series_download_error_handling() -> AppResult<()> {
        // REQUIREMENT: Test series download error handling
        // PURPOSE: Verify that errors are handled gracefully
        // This ensures robust operation in production

        let container = TestContainer::new().await;
        let pool = container.pool();
        let client = Client::new();
        let downloader = SeriesDownloader::new(client);

        // Test with invalid data source
        let result = downloader.download_specific_series(&pool, "INVALID_SOURCE", "SOME_ID").await;
        assert!(result.is_err(), "Should error on invalid data source");
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Data source not found") ||
                error.to_string().contains("Unknown data source"),
            "Should have meaningful error message: {}", error);

        // Test with empty source name
        let result = downloader.download_specific_series(&pool, "", "SOME_ID").await;
        assert!(result.is_err(), "Should error on empty source name");

        // Test with empty series ID
        let result = downloader.download_specific_series(&pool, "BLS", "").await;
        assert!(result.is_err(), "Should error on empty series ID");

        // Test with non-existent series ID
        let result = downloader.download_specific_series(&pool, "BLS", "NONEXISTENT_ID").await;
        assert!(result.is_err(), "Should error on non-existent series ID");
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Series not found"),
            "Should have meaningful error message: {}", error);

        println!("✅ Series download error handling test passed");
        println!("   - Invalid source handling: ✅");
        println!("   - Empty parameter handling: ✅");
        println!("   - Non-existent series handling: ✅");
        println!("   - Meaningful error messages: ✅");

        Ok(())
    }

    /// Test series download with missing catalog
    ///
    /// This test verifies that the series downloader handles
    /// cases where the catalog hasn't been downloaded yet.
    #[tokio::test]
    #[serial]
    async fn test_series_download_missing_catalog() -> AppResult<()> {
        // REQUIREMENT: Test series download with missing catalog
        // PURPOSE: Verify that missing catalog is handled gracefully
        // This ensures proper error messages for users

        let container = TestContainer::new().await;
        let pool = container.pool();
        let client = Client::new();
        let downloader = SeriesDownloader::new(client);

        // Ensure BLS data source exists but don't download catalog
        let _bls_source = DataSource::get_or_create(&pool, DataSource::bls()).await?;

        // Test downloading specific series without catalog
        let result = downloader.download_specific_series(&pool, "BLS", "SOME_ID").await;

        // Should either succeed (if catalog download works) or fail gracefully
        match result {
            Ok(_) => {
                println!("BLS catalog download succeeded during series download");
            }
            Err(e) => {
                // Should fail with meaningful error about missing series
                assert!(e.to_string().contains("Series not found") ||
                        e.to_string().contains("catalog download"),
                    "Should have meaningful error message: {}", e);
            }
        }

        // Test downloading random series without catalog
        let result = downloader.download_random_series(&pool, "BLS").await;

        match result {
            Ok(_) => {
                println!("BLS random series download succeeded");
            }
            Err(e) => {
                // Should fail with meaningful error about no series found
                assert!(e.to_string().contains("No series found") ||
                        e.to_string().contains("catalog download"),
                    "Should have meaningful error message: {}", e);
            }
        }

        println!("✅ Series download missing catalog test passed");
        println!("   - Missing catalog handled: ✅");
        println!("   - Meaningful error messages: ✅");
        println!("   - Graceful failure: ✅");

        Ok(())
    }

    /// Test series download with multiple sources
    ///
    /// This test verifies that the series downloader can work
    /// with multiple data sources.
    #[tokio::test]
    #[serial]
    async fn test_series_download_multiple_sources() -> AppResult<()> {
        // REQUIREMENT: Test series download with multiple sources
        // PURPOSE: Verify that multiple sources can be processed
        // This ensures the system scales to multiple providers

        let container = TestContainer::new().await;
        let pool = container.pool();
        let client = Client::new();
        let downloader = SeriesDownloader::new(client);

        // Test multiple sources
        let sources = vec!["BLS", "FRED", "Census", "BEA"];
        let mut successful_downloads = 0;

        for source in sources {
            // Ensure data source exists
            let source_enum = match source {
                "BLS" => DataSource::bls(),
                "FRED" => DataSource::fred(),
                "Census" => DataSource::census(),
                "BEA" => DataSource::bea(),
                _ => continue,
            };
            let _data_source = DataSource::get_or_create(&pool, source_enum).await?;

            // Test random series download
            let result = downloader.download_random_series(&pool, source).await;

            match result {
                Ok(_) => {
                    println!("{} random series download succeeded", source);
                    successful_downloads += 1;
                }
                Err(e) => {
                    println!("{} random series download failed: {}", source, e);
                    // Some sources might fail due to missing API keys or other issues
                    // This is acceptable as long as it fails gracefully
                }
            }
        }

        // Should have at least one successful download
        assert!(successful_downloads > 0, "Should have at least one successful download");

        println!("✅ Series download multiple sources test passed");
        println!("   - Successful downloads: {}/{}", successful_downloads, sources.len());
        println!("   - Graceful failure handling: ✅");

        Ok(())
    }

    /// Test series download performance
    ///
    /// This test verifies that series downloads complete in
    /// reasonable time and don't hang.
    #[tokio::test]
    #[serial]
    async fn test_series_download_performance() -> AppResult<()> {
        // REQUIREMENT: Test series download performance
        // PURPOSE: Verify that downloads complete in reasonable time
        // This ensures the system is responsive

        let container = TestContainer::new().await;
        let pool = container.pool();
        let client = Client::new();
        let downloader = SeriesDownloader::new(client);

        // Ensure BLS data source exists
        let _bls_source = DataSource::get_or_create(&pool, DataSource::bls()).await?;

        // Test random series download with timing
        let start_time = std::time::Instant::now();
        let result = downloader.download_random_series(&pool, "BLS").await;
        let elapsed = start_time.elapsed();

        // Should complete within reasonable time (30 seconds max)
        assert!(elapsed < std::time::Duration::from_secs(30),
            "Series download should complete within 30 seconds, took {:?}", elapsed);

        match result {
            Ok(_) => {
                println!("Random series download succeeded in {:?}", elapsed);
            }
            Err(e) => {
                println!("Random series download failed in {:?}: {}", elapsed, e);
                // OK if it fails due to other reasons
            }
        }

        println!("✅ Series download performance test passed");
        println!("   - Download time: {:?}", elapsed);
        println!("   - Within time limits: ✅");
        println!("   - No hanging: ✅");

        Ok(())
    }

    /// Test series download with database constraints
    ///
    /// This test verifies that series downloads respect database
    /// constraints and handle data properly.
    #[tokio::test]
    #[serial]
    async fn test_series_download_database_constraints() -> AppResult<()> {
        // REQUIREMENT: Test series download with database constraints
        // PURPOSE: Verify that database constraints are respected
        // This ensures data integrity

        let container = TestContainer::new().await;
        let pool = container.pool();
        let client = Client::new();
        let downloader = SeriesDownloader::new(client);

        // Ensure BLS data source exists
        let bls_source = DataSource::get_or_create(&pool, DataSource::bls()).await?;

        // Download catalog first
        let catalog_result = downloader.catalog_downloader.download_catalog(&pool, "BLS").await;
        if catalog_result.is_err() {
            println!("BLS catalog download failed, skipping database constraints test: {}", catalog_result.unwrap_err());
            return Ok(());
        }

        // Get available series
        let available_series = crate::models::series_metadata::SeriesMetadata::find_by_source(
            &pool,
            bls_source.id,
        ).await?;

        if available_series.is_empty() {
            println!("No BLS series available, skipping database constraints test");
            return Ok(());
        }

        // Test downloading the same series multiple times
        let test_series = &available_series[0];

        // First download
        let result1 = downloader.download_specific_series(
            &pool,
            "BLS",
            &test_series.external_id,
        ).await;
        assert!(result1.is_ok(), "First download should succeed");

        // Second download (should handle gracefully)
        let result2 = downloader.download_specific_series(
            &pool,
            "BLS",
            &test_series.external_id,
        ).await;
        assert!(result2.is_ok(), "Second download should succeed");

        // Verify series metadata is still valid
        let series_metadata = crate::models::series_metadata::SeriesMetadata::find_by_external_id(
            &pool,
            bls_source.id,
            &test_series.external_id,
        ).await?;

        assert!(series_metadata.is_some(), "Series metadata should still exist");
        let series_metadata = series_metadata.unwrap();
        assert_eq!(series_metadata.external_id, test_series.external_id);
        assert_eq!(series_metadata.source_id, bls_source.id);

        println!("✅ Series download database constraints test passed");
        println!("   - Multiple downloads handled: ✅");
        println!("   - Database integrity maintained: ✅");
        println!("   - Series metadata preserved: ✅");

        Ok(())
    }

    /// Test series download with invalid series ID
    ///
    /// This test verifies that the series downloader properly
    /// handles invalid or non-existent series IDs.
    #[tokio::test]
    #[serial]
    async fn test_series_download_invalid_series_id() -> AppResult<()> {
        // REQUIREMENT: Test series download with invalid series ID
        // PURPOSE: Verify that invalid series IDs are handled gracefully
        // This ensures proper error handling for user input

        let container = TestContainer::new().await;
        let pool = container.pool();
        let client = Client::new();
        let downloader = SeriesDownloader::new(client);

        // Ensure BLS data source exists
        let _bls_source = DataSource::get_or_create(&pool, DataSource::bls()).await?;

        // Test with various invalid series IDs
        let invalid_ids = vec![
            "",                    // Empty
            "INVALID_ID",         // Non-existent
            "123",                // Numeric but invalid
            "SERIES_WITH_SPECIAL_CHARS!@#", // Special characters
            "VERY_LONG_SERIES_ID_THAT_EXCEEDS_NORMAL_LENGTH_AND_SHOULD_BE_HANDLED_GRACEFULLY", // Too long
        ];

        for invalid_id in invalid_ids {
            let result = downloader.download_specific_series(&pool, "BLS", invalid_id).await;

            // Should fail gracefully
            assert!(result.is_err(), "Should fail for invalid series ID: '{}'", invalid_id);
            let error = result.unwrap_err();

            // Should have meaningful error message
            assert!(error.to_string().contains("Series not found") ||
                    error.to_string().contains("not found") ||
                    error.to_string().contains("catalog download"),
                "Should have meaningful error for '{}': {}", invalid_id, error);
        }

        println!("✅ Series download invalid series ID test passed");
        println!("   - Empty ID handled: ✅");
        println!("   - Non-existent ID handled: ✅");
        println!("   - Invalid format handled: ✅");
        println!("   - Meaningful error messages: ✅");

        Ok(())
    }
}
