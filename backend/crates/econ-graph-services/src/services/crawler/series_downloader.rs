//! Series downloader for specific economic data series
//!
//! Handles downloading actual time series data for specific series

use crate::services::crawler::CatalogDownloader;
use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::AppResult;
use econ_graph_core::models::{DataSource, SeriesMetadata};
use reqwest::Client;

/// Service for downloading specific series data
pub struct SeriesDownloader {
    catalog_downloader: CatalogDownloader,
}

impl SeriesDownloader {
    /// Create a new series downloader
    pub fn new(client: Client) -> Self {
        Self {
            catalog_downloader: CatalogDownloader::new(client),
        }
    }

    /// Download a specific series from a data source
    pub async fn download_specific_series(
        &self,
        pool: &DatabasePool,
        source_name: &str,
        series_id: &str,
    ) -> AppResult<()> {
        // First, ensure the catalog is downloaded
        println!("📋 Ensuring catalog is downloaded for {}", source_name);
        self.catalog_downloader
            .download_catalog(pool, source_name)
            .await?;

        // Find the data source
        let data_source = DataSource::find_by_name(pool, source_name)
            .await?
            .ok_or_else(|| {
                econ_graph_core::error::AppError::ValidationError(format!(
                    "Data source not found: {}",
                    source_name
                ))
            })?;

        // Find the series metadata
        let series_metadata =
            SeriesMetadata::find_by_external_id(pool, data_source.id, series_id).await?;
        let Some(series_metadata) = series_metadata else {
            return Err(econ_graph_core::error::AppError::ValidationError(format!(
                "Series not found: {} in data source {}. Try running catalog download first to see available series",
                series_id, source_name
            )));
        };

        println!("📊 Found series: {}", series_metadata.title);
        println!(
            "📝 Description: {}",
            series_metadata.description.as_deref().unwrap_or("N/A")
        );
        println!(
            "📏 Units: {}",
            series_metadata.units.as_deref().unwrap_or("N/A")
        );
        println!(
            "🔄 Frequency: {}",
            series_metadata.frequency.as_deref().unwrap_or("N/A")
        );

        // TODO: Implement actual data downloading
        // This would involve calling the appropriate crawler service
        // to fetch the actual time series data and store it in the database
        println!("⚠️  Data downloading not yet implemented - this is a placeholder");
        println!(
            "🔗 Series URL: {}",
            series_metadata.data_url.as_deref().unwrap_or("N/A")
        );

        Ok(())
    }

    /// Download a random series from a data source
    pub async fn download_random_series(
        &self,
        pool: &DatabasePool,
        source_name: &str,
    ) -> AppResult<()> {
        // First, ensure the catalog is downloaded
        println!("📋 Ensuring catalog is downloaded for {}", source_name);
        self.catalog_downloader
            .download_catalog(pool, source_name)
            .await?;

        // Find the data source
        let data_source = DataSource::find_by_name(pool, source_name)
            .await?
            .ok_or_else(|| {
                econ_graph_core::error::AppError::ValidationError(format!(
                    "Data source not found: {}",
                    source_name
                ))
            })?;

        // Get all available series for this source
        let all_series = SeriesMetadata::find_by_source(pool, data_source.id).await?;

        if all_series.is_empty() {
            return Err(econ_graph_core::error::AppError::ValidationError(format!(
                "No series found for data source: {}. Try running catalog download first",
                source_name
            )));
        }

        // Select a random series
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let random_series = all_series.choose(&mut rng).unwrap();

        println!("🎲 Randomly selected series: {}", random_series.title);
        println!("🆔 Series ID: {}", random_series.external_id);
        println!(
            "📝 Description: {}",
            random_series.description.as_deref().unwrap_or("N/A")
        );
        println!(
            "📏 Units: {}",
            random_series.units.as_deref().unwrap_or("N/A")
        );
        println!(
            "🔄 Frequency: {}",
            random_series.frequency.as_deref().unwrap_or("N/A")
        );

        // TODO: Implement actual data downloading
        println!("⚠️  Data downloading not yet implemented - this is a placeholder");
        println!(
            "🔗 Series URL: {}",
            random_series.data_url.as_deref().unwrap_or("N/A")
        );

        Ok(())
    }
}
