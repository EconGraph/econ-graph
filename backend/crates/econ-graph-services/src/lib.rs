// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # EconGraph Services
//!
//! Business logic, data processing, and service implementations for the `EconGraph` system.
//! This crate contains all the core business logic, external API integrations, and data
//! processing services that power the economic data platform.
//!
//! ## Features
//!
//! - **Data Discovery**: Comprehensive series discovery across multiple economic data sources
//! - **Crawler Services**: Advanced web crawling and data collection capabilities
//! - **Search & Analysis**: Global economic analysis and search functionality
//! - **API Integration**: Integration with major economic data providers (FRED, BLS, Census, etc.)
//! - **Queue Management**: Asynchronous task processing and job scheduling
//! - **Collaboration**: User collaboration and data sharing features
//!
//! ## Architecture
//!
//! This crate follows a service-oriented architecture:
//! - **Services**: Core business logic and external integrations
//! - **Crawlers**: Data collection and web scraping capabilities
//! - **Series Discovery**: Economic data source integration and discovery
//! - **Queue System**: Asynchronous processing and job management
//!
//! ## Usage
//!
//! ```rust,no_run
//! use econ_graph_services::services::{SearchService, CrawlerService};
//! use econ_graph_services::services::queue_service::QueueService;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize services
//!     let search_service = SearchService::new();
//!     let crawler_service = CrawlerService::new();
//!
//!     // Use services for data processing
//!     Ok(())
//! }
//! ```

pub mod services;

// Re-export commonly used services
pub use services::*;
