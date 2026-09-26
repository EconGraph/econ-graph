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
//! ## Modules
//!
//! - [`source`]: [`SourceId`], the canonical source names stored in `crawl_queue.source`
//! - [`policy`]: [`SourcePolicy`], per-source rate, concurrency and retry settings
//! - [`error`]: [`CrawlError`], the retry-relevant error taxonomy
//! - [`http`]: [`HttpFetcher`], the one shared, rate-limited, retrying HTTP client
//! - [`rate_limit`]: [`SourceRateLimiter`], per-source token bucket + concurrency limit
//! - [`adapter`]: the [`SourceAdapter`] trait and [`AdapterRegistry`]
//! - [`persist`]: shared database writes (series, data points, catalog metadata, crawl attempts)
//! - [`worker`]: the [`Worker`] that drains `crawl_queue`, and the [`JobHandler`] extension point
//! - [`scheduler`]: [`RefreshScheduler`], enqueues due series refreshes and weekly catalog discovery
//! - [`status`]: crawler status derived from `crawl_queue` ([`status::crawler_status`])
//! - [`cli`]: the `crawler` operator CLI (enqueue, discover, status, sources, fetch)
//! - `testkit` (feature `testkit`, always on in this crate's tests): mock upstream + adapter contract tests
//!
//! ## Usage
//!
//! ```rust,no_run
//! use std::collections::HashMap;
//! use econ_graph_crawler::{
//!     AdapterRegistry, ApiKeys, CrawlCtx, HttpConfig, HttpFetcher, SourceId,
//! };
//!
//! # async fn run(pool: econ_graph_core::DatabasePool) -> Result<(), Box<dyn std::error::Error>> {
//! let http = HttpFetcher::new(HttpConfig::default(), HashMap::new())?;
//! let ctx = CrawlCtx { http, pool, keys: ApiKeys::from_env() };
//!
//! let registry = AdapterRegistry::new(); // register source adapters here
//! if let Some(adapter) = registry.get("FRED".parse::<SourceId>()?) {
//!     let series = adapter.fetch_series(&ctx, "GDP", None).await?;
//!     println!("{} observations", series.points.len());
//! }
//! # Ok(())
//! # }
//! ```

pub mod adapter;
pub mod cli;
pub mod error;
pub mod http;
pub mod persist;
pub mod policy;
pub mod rate_limit;
pub mod reference;
pub mod scheduler;
pub mod source;
pub mod sources;
pub mod status;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
pub mod worker;

pub use adapter::{
    AdapterRegistry, ApiKeys, CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries,
    NewSeriesMetadataLite, SourceAdapter,
};
pub use error::CrawlError;
pub use http::{HttpConfig, HttpFetcher};
pub use policy::SourcePolicy;
pub use rate_limit::{SourcePermit, SourceRateLimiter};
pub use scheduler::{RefreshScheduler, SchedulerConfig, SchedulerTickStats};
pub use source::{SourceId, UnknownSource};
pub use worker::{JobHandler, JobOutcome, JobStats, Worker, WorkerConfig};
