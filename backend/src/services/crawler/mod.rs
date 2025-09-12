//! Crawler service for downloading economic data series
//!
//! This module provides the core crawler functionality for:
//! 1. Downloading data source catalogs into the database
//! 2. Downloading specific series or random series into the database

pub mod catalog_downloader;
pub mod cli;
pub mod comprehensive_crawler;
pub mod comprehensive_crawler_scheduler;
pub mod crawler_service;
pub mod enhanced_crawler_scheduler;
pub mod enhanced_crawler_service;
pub mod legacy_crawler_service;
pub mod series_downloader;
pub mod simple_crawler_service;

#[cfg(test)]
mod tests;

pub use catalog_downloader::CatalogDownloader;
pub use cli::CrawlerCli;
pub use series_downloader::SeriesDownloader;
