//! # Crawler Metrics
//!
//! This module provides comprehensive metrics collection for web crawlers in the `EconGraph` system.
//! It tracks request performance, data collection, errors, and resource usage across different
//! crawler types and data sources.
//!
//! ## Features
//!
//! - **Request Tracking**: Monitor request counts, durations, and success rates
//! - **Data Collection**: Track items collected and bytes downloaded
//! - **Error Monitoring**: Categorize and count different types of errors
//! - **Rate Limiting**: Monitor rate limit hits and retry attempts
//! - **Performance Analysis**: Histogram-based duration tracking for performance insights
//!
//! ## Usage
//!
//! ```rust,no_run
//! use econ_graph_metrics::crawler::{CRAWLER_METRICS, CrawlerMetrics};
//!
//! // Record a successful request
//! CRAWLER_METRICS.record_request("sec_edgar", "sec.gov", "/filings", "200", 1.5);
//!
//! // Record collected items
//! CRAWLER_METRICS.record_items_collected("sec_edgar", "sec.gov", "10k_filing", 1);
//!
//! // Record an error
//! CRAWLER_METRICS.record_error("sec_edgar", "sec.gov", "network_timeout");
//! ```

use crate::DEFAULT_REGISTRY;
use once_cell::sync::Lazy;
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry};

/// Comprehensive metrics collection for web crawlers
///
/// This struct provides all the necessary Prometheus metrics for monitoring crawler performance,
/// data collection, and error tracking. Each metric is designed to provide insights into
/// crawler behavior and system health.
pub struct CrawlerMetrics {
    /// Total number of crawler requests made, categorized by type, source, endpoint, and status
    pub crawler_requests_total: IntCounterVec,
    /// Duration of crawler requests in seconds, with histogram buckets for performance analysis
    pub crawler_request_duration_seconds: HistogramVec,
    /// Total number of items collected by crawlers, categorized by type, source, and item type
    pub crawler_items_collected_total: IntCounterVec,
    /// Total bytes downloaded by crawlers, categorized by type and source
    pub crawler_bytes_downloaded_total: IntCounterVec,
    /// Total number of crawler errors, categorized by type, source, and error type
    pub crawler_errors_total: IntCounterVec,
    /// Total number of rate limit hits, categorized by type and source
    pub crawler_rate_limit_hits_total: IntCounterVec,
    /// Total number of retry attempts, categorized by type, source, and retry reason
    pub crawler_retries_total: IntCounterVec,
    /// Total number of timeout errors, categorized by type and source
    pub crawler_timeouts_total: IntCounterVec,
}

impl CrawlerMetrics {
    /// Create a new `CrawlerMetrics` instance with all metrics registered to the provided registry
    ///
    /// This method initializes all Prometheus metrics with their respective labels and registers
    /// them with the given registry. Each metric is configured with appropriate names, help text,
    /// and label dimensions for comprehensive monitoring.
    ///
    /// # Parameters
    /// - `registry`: The Prometheus registry to register metrics with
    ///
    /// # Returns
    /// Returns a `CrawlerMetrics` instance with all metrics properly initialized and registered
    ///
    /// # Errors
    ///
    /// Returns an error if any metric fails to register with the provided registry
    pub fn new(registry: &Registry) -> anyhow::Result<Self> {
        let crawler_requests_total = IntCounterVec::new(
            Opts::new(
                "econgraph_crawler_requests_total",
                "Total number of crawler requests",
            ),
            &["crawler_type", "source", "endpoint", "status"],
        )?;
        registry.register(Box::new(crawler_requests_total.clone()))?;

        let crawler_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "econgraph_crawler_request_duration_seconds",
                "Crawler request duration in seconds",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0, 120.0]),
            &["crawler_type", "source", "endpoint"],
        )?;
        registry.register(Box::new(crawler_request_duration_seconds.clone()))?;

        let crawler_items_collected_total = IntCounterVec::new(
            Opts::new(
                "econgraph_crawler_items_collected_total",
                "Total number of items collected by crawlers",
            ),
            &["crawler_type", "source", "item_type"],
        )?;
        registry.register(Box::new(crawler_items_collected_total.clone()))?;

        let crawler_bytes_downloaded_total = IntCounterVec::new(
            Opts::new(
                "econgraph_crawler_bytes_downloaded_total",
                "Total bytes downloaded by crawlers",
            ),
            &["crawler_type", "source"],
        )?;
        registry.register(Box::new(crawler_bytes_downloaded_total.clone()))?;

        let crawler_errors_total = IntCounterVec::new(
            Opts::new(
                "econgraph_crawler_errors_total",
                "Total number of crawler errors",
            ),
            &["crawler_type", "source", "error_type"],
        )?;
        registry.register(Box::new(crawler_errors_total.clone()))?;

        let crawler_rate_limit_hits_total = IntCounterVec::new(
            Opts::new(
                "econgraph_crawler_rate_limit_hits_total",
                "Total number of rate limit hits",
            ),
            &["crawler_type", "source"],
        )?;
        registry.register(Box::new(crawler_rate_limit_hits_total.clone()))?;

        let crawler_retries_total = IntCounterVec::new(
            Opts::new(
                "econgraph_crawler_retries_total",
                "Total number of retry attempts",
            ),
            &["crawler_type", "source", "retry_reason"],
        )?;
        registry.register(Box::new(crawler_retries_total.clone()))?;

        let crawler_timeouts_total = IntCounterVec::new(
            Opts::new(
                "econgraph_crawler_timeouts_total",
                "Total number of timeout errors",
            ),
            &["crawler_type", "source"],
        )?;
        registry.register(Box::new(crawler_timeouts_total.clone()))?;

        Ok(Self {
            crawler_requests_total,
            crawler_request_duration_seconds,
            crawler_items_collected_total,
            crawler_bytes_downloaded_total,
            crawler_errors_total,
            crawler_rate_limit_hits_total,
            crawler_retries_total,
            crawler_timeouts_total,
        })
    }

    /// Record a crawler request with its duration and status
    ///
    /// This method updates both the request counter and duration histogram metrics
    /// for comprehensive request monitoring. The metrics are labeled with the crawler
    /// type, source, endpoint, and status for detailed analysis.
    ///
    /// # Parameters
    /// - `crawler_type`: Type of crawler (e.g., "sec_edgar", "census")
    /// - `source`: Data source being crawled (e.g., "sec.gov", "census.gov")
    /// - `endpoint`: Specific endpoint or path being requested
    /// - `status`: HTTP status code or result status (e.g., "200", "404", "error")
    /// - `duration`: Request duration in seconds
    pub fn record_request(
        &self,
        crawler_type: &str,
        source: &str,
        endpoint: &str,
        status: &str,
        duration: f64,
    ) {
        self.crawler_requests_total
            .with_label_values(&[crawler_type, source, endpoint, status])
            .inc();
        self.crawler_request_duration_seconds
            .with_label_values(&[crawler_type, source, endpoint])
            .observe(duration);
    }

    /// Record the number of items collected by a crawler
    ///
    /// This method tracks the volume of data collected by crawlers, providing insights
    /// into crawler productivity and data source richness. The metric is labeled with
    /// crawler type, source, and item type for detailed analysis.
    ///
    /// # Parameters
    /// - `crawler_type`: Type of crawler (e.g., "sec_edgar", "census")
    /// - `source`: Data source being crawled (e.g., "sec.gov", "census.gov")
    /// - `item_type`: Type of item collected (e.g., "10k_filing", "economic_series")
    /// - `count`: Number of items collected
    pub fn record_items_collected(
        &self,
        crawler_type: &str,
        source: &str,
        item_type: &str,
        count: u64,
    ) {
        self.crawler_items_collected_total
            .with_label_values(&[crawler_type, source, item_type])
            .inc_by(count);
    }

    /// Record the number of bytes downloaded by a crawler
    ///
    /// This method tracks bandwidth usage and data volume downloaded by crawlers,
    /// providing insights into resource consumption and data transfer patterns.
    /// The metric is labeled with crawler type and source for analysis.
    ///
    /// # Parameters
    /// - `crawler_type`: Type of crawler (e.g., "sec_edgar", "census")
    /// - `source`: Data source being crawled (e.g., "sec.gov", "census.gov")
    /// - `bytes`: Number of bytes downloaded
    pub fn record_bytes_downloaded(&self, crawler_type: &str, source: &str, bytes: u64) {
        self.crawler_bytes_downloaded_total
            .with_label_values(&[crawler_type, source])
            .inc_by(bytes);
    }

    /// Record a crawler error
    ///
    /// This method tracks errors encountered during crawling operations, providing
    /// insights into system reliability and common failure patterns. The metric
    /// is labeled with crawler type, source, and error type for detailed analysis.
    ///
    /// # Parameters
    /// - `crawler_type`: Type of crawler (e.g., "sec_edgar", "census")
    /// - `source`: Data source being crawled (e.g., "sec.gov", "census.gov")
    /// - `error_type`: Type of error (e.g., "network_timeout", "parse_error", "auth_failed")
    pub fn record_error(&self, crawler_type: &str, source: &str, error_type: &str) {
        self.crawler_errors_total
            .with_label_values(&[crawler_type, source, error_type])
            .inc();
    }

    /// Record a rate limit hit
    ///
    /// This method tracks when crawlers encounter rate limits from data sources,
    /// providing insights into crawling politeness and source limitations.
    /// The metric is labeled with crawler type and source for analysis.
    ///
    /// # Parameters
    /// - `crawler_type`: Type of crawler (e.g., "sec_edgar", "census")
    /// - `source`: Data source being crawled (e.g., "sec.gov", "census.gov")
    pub fn record_rate_limit_hit(&self, crawler_type: &str, source: &str) {
        self.crawler_rate_limit_hits_total
            .with_label_values(&[crawler_type, source])
            .inc();
    }

    /// Record a retry attempt
    ///
    /// This method tracks retry attempts made by crawlers, providing insights into
    /// system resilience and common failure recovery patterns. The metric is labeled
    /// with crawler type, source, and retry reason for detailed analysis.
    ///
    /// # Parameters
    /// - `crawler_type`: Type of crawler (e.g., "sec_edgar", "census")
    /// - `source`: Data source being crawled (e.g., "sec.gov", "census.gov")
    /// - `retry_reason`: Reason for the retry (e.g., "network_error", "timeout", "rate_limit")
    pub fn record_retry(&self, crawler_type: &str, source: &str, retry_reason: &str) {
        self.crawler_retries_total
            .with_label_values(&[crawler_type, source, retry_reason])
            .inc();
    }

    /// Record a timeout error
    ///
    /// This method tracks timeout errors encountered by crawlers, providing insights
    /// into network performance and source responsiveness. The metric is labeled
    /// with crawler type and source for analysis.
    ///
    /// # Parameters
    /// - `crawler_type`: Type of crawler (e.g., "sec_edgar", "census")
    /// - `source`: Data source being crawled (e.g., "sec.gov", "census.gov")
    pub fn record_timeout(&self, crawler_type: &str, source: &str) {
        self.crawler_timeouts_total
            .with_label_values(&[crawler_type, source])
            .inc();
    }
}

/// Global crawler metrics instance
///
/// This static provides a lazily-initialized global instance of `CrawlerMetrics`
/// that is registered with the default registry. It can be used throughout the
/// application for consistent metrics collection without needing to pass metrics
/// instances around.
///
/// # Panics
///
/// Panics if the metrics fail to initialize during lazy initialization
pub static CRAWLER_METRICS: Lazy<CrawlerMetrics> = Lazy::new(|| {
    CrawlerMetrics::new(&DEFAULT_REGISTRY).expect("Failed to initialize crawler metrics")
});
