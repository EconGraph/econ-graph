// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # EconGraph Crawler
//!
//! Data acquisition and crawling functionality for the `EconGraph` system.
//! This crate provides comprehensive tools for crawling external data sources
//! with advanced features like rate limiting, retry logic, and data validation.
//!
//! ## Features
//!
//! - **Multi-Source Crawling**: Support for various economic data sources and APIs
//! - **Rate Limiting**: Intelligent rate limiting to respect source policies
//! - **Retry Logic**: Robust retry mechanisms with exponential backoff
//! - **Data Validation**: Comprehensive data validation and quality checks
//! - **Progress Tracking**: Real-time progress monitoring and status reporting
//! - **Error Handling**: Comprehensive error handling and recovery mechanisms
//!
//! ## Architecture
//!
//! This crate provides both library and binary interfaces:
//! - **Library**: Shared utilities and crawling components
//! - **Binaries**: Standalone crawler applications for specific data sources
//! - **Utilities**: Common crawling utilities and helper functions
//!
//! ## Usage
//!
//! ```rust,no_run
//! use econ_graph_crawler::CrawlerConfig;
//! use econ_graph_crawler::CrawlerService;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize crawler with configuration
//!     let config = CrawlerConfig::new();
//!     let crawler = CrawlerService::new(config);
//!
//!     // Start crawling process
//!     Ok(())
//! }
//! ```

// This crate primarily contains binaries, but we can add shared utilities here if needed
