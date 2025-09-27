//! # EconGraph Metrics
//!
//! This crate provides comprehensive metrics collection and monitoring capabilities for the `EconGraph` system.
//! It offers structured metrics for crawlers, performance monitoring, and system health tracking using Prometheus.
//!
//! ## Features
//!
//! - **Crawler Metrics**: Request tracking, duration monitoring, and error reporting
//! - **Performance Monitoring**: Request duration histograms and throughput metrics
//! - **Error Tracking**: Comprehensive error categorization and counting
//! - **Resource Usage**: Bandwidth and data collection metrics
//! - **Rate Limiting**: Track rate limit hits and retry attempts
//!
//! ## Usage
//!
//! ```rust,no_run
//! use econ_graph_metrics::{CRAWLER_METRICS, DEFAULT_REGISTRY};
//! use econ_graph_metrics::crawler::CrawlerMetrics;
//!
//! // Record a crawler request
//! CRAWLER_METRICS.record_request("sec_edgar", "sec.gov", "/filings", "200", 1.5);
//!
//! // Record items collected
//! CRAWLER_METRICS.record_items_collected("sec_edgar", "sec.gov", "10k_filing", 1);
//! ```

use once_cell::sync::Lazy;
pub use prometheus;
use prometheus::Registry;
use std::sync::Arc;

pub mod crawler;

/// Shared default registry used across crates
///
/// This registry is lazily initialized and provides a centralized location for all metrics
/// collection across the `EconGraph` system. It uses the default Prometheus registry as the
/// underlying implementation.
pub static DEFAULT_REGISTRY: Lazy<Arc<Registry>> =
    Lazy::new(|| Arc::new(prometheus::default_registry().clone()));
