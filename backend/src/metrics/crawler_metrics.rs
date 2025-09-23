//! Unified Crawler Metrics for EconGraph
//!
//! This module provides comprehensive Prometheus metrics for all crawler types
//! in the EconGraph system, including economic data crawlers, SEC EDGAR crawler,
//! and queue-based crawlers. The metrics are designed to be unified across
//! all crawler types while allowing for crawler-specific extensions.

use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
};
use std::sync::Arc;

/// Unified crawler metrics that work across all crawler types
pub struct CrawlerMetrics {
    // === CORE CRAWLER METRICS (All Crawler Types) ===

    /// Total requests made by crawlers
    pub crawler_requests_total: IntCounterVec,

    /// Request duration in seconds
    pub crawler_request_duration_seconds: HistogramVec,

    /// Data points/items collected by crawlers
    pub crawler_items_collected_total: IntCounterVec,

    /// Bytes downloaded by crawlers
    pub crawler_bytes_downloaded_total: IntCounterVec,

    /// Errors encountered by crawlers
    pub crawler_errors_total: IntCounterVec,

    /// Rate limit hits by crawlers
    pub crawler_rate_limit_hits_total: IntCounterVec,

    /// Retry attempts by crawlers
    pub crawler_retries_total: IntCounterVec,

    /// Timeout errors by crawlers
    pub crawler_timeouts_total: IntCounterVec,

    /// Active crawler workers
    pub crawler_workers_active: IntGauge,

    /// Queue size for crawler jobs
    pub crawler_queue_size: IntGauge,

    /// Last successful crawl timestamp per source
    pub crawler_last_success_timestamp: IntGauge,

    // === ECONOMIC DATA CRAWLER SPECIFIC ===

    /// Economic series discovered per source
    pub economic_series_discovered_total: IntCounterVec,

    /// Economic data points collected per source
    pub economic_data_points_collected_total: IntCounterVec,

    /// API quota usage per source
    pub economic_api_quota_usage: IntGauge,

    /// Data freshness in hours per source
    pub economic_data_freshness_hours: IntGauge,

    // === SEC EDGAR CRAWLER SPECIFIC ===

    /// SEC filings downloaded
    pub sec_filings_downloaded_total: IntCounterVec,

    /// XBRL files processed
    pub sec_xbrl_files_processed_total: IntCounterVec,

    /// Taxonomy components downloaded
    pub sec_taxonomy_components_downloaded_total: IntCounterVec,

    /// Filing processing duration
    pub sec_filing_processing_duration_seconds: HistogramVec,

    /// XBRL parsing duration
    pub sec_xbrl_parsing_duration_seconds: HistogramVec,

    /// Companies crawled
    pub sec_companies_crawled_total: IntCounterVec,

    /// Filing sizes in bytes
    pub sec_filing_sizes_bytes: HistogramVec,

    // === BANDWIDTH AND PERFORMANCE ===

    /// Bandwidth usage per source (bytes per second)
    pub crawler_bandwidth_bytes_per_second: HistogramVec,

    /// Requests per second per source
    pub crawler_requests_per_second: HistogramVec,

    /// Response time percentiles per source
    pub crawler_response_time_seconds: HistogramVec,

    /// Memory usage per crawler type
    pub crawler_memory_usage_bytes: IntGauge,

    /// CPU usage per crawler type
    pub crawler_cpu_usage_percent: IntGauge,

    // === POLITENESS AND COMPLIANCE ===

    /// Request delays between requests per source
    pub crawler_request_delays_seconds: HistogramVec,

    /// Robots.txt compliance checks
    pub crawler_robots_txt_compliance: IntCounterVec,

    /// User agent usage tracking
    pub crawler_user_agent_usage: IntCounterVec,

    /// Concurrent request limits per source
    pub crawler_concurrent_requests: IntGauge,

    /// Politeness policy compliance
    pub crawler_politeness_compliance: IntCounterVec,

    // === DATA QUALITY ===

    /// Data validation errors per source
    pub crawler_data_validation_errors_total: IntCounterVec,

    /// Data completeness score per source
    pub crawler_data_completeness_score: IntGauge,

    /// Data accuracy score per source
    pub crawler_data_accuracy_score: IntGauge,

    /// Duplicate data detection per source
    pub crawler_duplicate_data_total: IntCounterVec,
}

impl CrawlerMetrics {
    /// Create and register all crawler metrics
    pub fn new(registry: &Registry) -> anyhow::Result<Self> {
        // === CORE CRAWLER METRICS ===

        let crawler_requests_total = IntCounterVec::new(
            Opts::new("econgraph_crawler_requests_total", "Total number of crawler requests"),
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
            Opts::new("econgraph_crawler_items_collected_total", "Total number of items collected by crawlers"),
            &["crawler_type", "source", "item_type"],
        )?;
        registry.register(Box::new(crawler_items_collected_total.clone()))?;

        let crawler_bytes_downloaded_total = IntCounterVec::new(
            Opts::new("econgraph_crawler_bytes_downloaded_total", "Total bytes downloaded by crawlers"),
            &["crawler_type", "source"],
        )?;
        registry.register(Box::new(crawler_bytes_downloaded_total.clone()))?;

        let crawler_errors_total = IntCounterVec::new(
            Opts::new("econgraph_crawler_errors_total", "Total number of crawler errors"),
            &["crawler_type", "source", "error_type"],
        )?;
        registry.register(Box::new(crawler_errors_total.clone()))?;

        let crawler_rate_limit_hits_total = IntCounterVec::new(
            Opts::new("econgraph_crawler_rate_limit_hits_total", "Total number of rate limit hits"),
            &["crawler_type", "source"],
        )?;
        registry.register(Box::new(crawler_rate_limit_hits_total.clone()))?;

        let crawler_retries_total = IntCounterVec::new(
            Opts::new("econgraph_crawler_retries_total", "Total number of retry attempts"),
            &["crawler_type", "source", "retry_reason"],
        )?;
        registry.register(Box::new(crawler_retries_total.clone()))?;

        let crawler_timeouts_total = IntCounterVec::new(
            Opts::new("econgraph_crawler_timeouts_total", "Total number of timeout errors"),
            &["crawler_type", "source"],
        )?;
        registry.register(Box::new(crawler_timeouts_total.clone()))?;

        let crawler_workers_active = IntGauge::new(
            "econgraph_crawler_workers_active",
            "Number of active crawler workers",
        )?;
        registry.register(Box::new(crawler_workers_active.clone()))?;

        let crawler_queue_size = IntGauge::new(
            "econgraph_crawler_queue_size",
            "Number of items in crawler queue",
        )?;
        registry.register(Box::new(crawler_queue_size.clone()))?;

        let crawler_last_success_timestamp = IntGauge::new(
            "econgraph_crawler_last_success_timestamp",
            "Timestamp of last successful crawl per source",
        )?;
        registry.register(Box::new(crawler_last_success_timestamp.clone()))?;

        // === ECONOMIC DATA CRAWLER SPECIFIC ===

        let economic_series_discovered_total = IntCounterVec::new(
            Opts::new("econgraph_economic_series_discovered_total", "Total number of economic series discovered"),
            &["source", "series_type"],
        )?;
        registry.register(Box::new(economic_series_discovered_total.clone()))?;

        let economic_data_points_collected_total = IntCounterVec::new(
            Opts::new("econgraph_economic_data_points_collected_total", "Total number of economic data points collected"),
            &["source", "series_id"],
        )?;
        registry.register(Box::new(economic_data_points_collected_total.clone()))?;

        let economic_api_quota_usage = IntGauge::new(
            "econgraph_economic_api_quota_usage",
            "API quota usage per source (percentage)",
        )?;
        registry.register(Box::new(economic_api_quota_usage.clone()))?;

        let economic_data_freshness_hours = IntGauge::new(
            "econgraph_economic_data_freshness_hours",
            "Data freshness in hours per source",
        )?;
        registry.register(Box::new(economic_data_freshness_hours.clone()))?;

        // === SEC EDGAR CRAWLER SPECIFIC ===

        let sec_filings_downloaded_total = IntCounterVec::new(
            Opts::new("econgraph_sec_filings_downloaded_total", "Total number of SEC filings downloaded"),
            &["form_type", "company", "status"],
        )?;
        registry.register(Box::new(sec_filings_downloaded_total.clone()))?;

        let sec_xbrl_files_processed_total = IntCounterVec::new(
            Opts::new("econgraph_sec_xbrl_files_processed_total", "Total number of XBRL files processed"),
            &["form_type", "company", "status"],
        )?;
        registry.register(Box::new(sec_xbrl_files_processed_total.clone()))?;

        let sec_taxonomy_components_downloaded_total = IntCounterVec::new(
            Opts::new("econgraph_sec_taxonomy_components_downloaded_total", "Total number of taxonomy components downloaded"),
            &["taxonomy_type", "version"],
        )?;
        registry.register(Box::new(sec_taxonomy_components_downloaded_total.clone()))?;

        let sec_filing_processing_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "econgraph_sec_filing_processing_duration_seconds",
                "SEC filing processing duration in seconds",
            )
            .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0]),
            &["form_type", "company"],
        )?;
        registry.register(Box::new(sec_filing_processing_duration_seconds.clone()))?;

        let sec_xbrl_parsing_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "econgraph_sec_xbrl_parsing_duration_seconds",
                "XBRL parsing duration in seconds",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
            &["form_type", "company"],
        )?;
        registry.register(Box::new(sec_xbrl_parsing_duration_seconds.clone()))?;

        let sec_companies_crawled_total = IntCounterVec::new(
            Opts::new("econgraph_sec_companies_crawled_total", "Total number of companies crawled"),
            &["company", "status"],
        )?;
        registry.register(Box::new(sec_companies_crawled_total.clone()))?;

        let sec_filing_sizes_bytes = HistogramVec::new(
            HistogramOpts::new(
                "econgraph_sec_filing_sizes_bytes",
                "SEC filing sizes in bytes",
            )
            .buckets(vec![1024.0, 10240.0, 102400.0, 1048576.0, 10485760.0, 104857600.0]),
            &["form_type", "company"],
        )?;
        registry.register(Box::new(sec_filing_sizes_bytes.clone()))?;

        // === BANDWIDTH AND PERFORMANCE ===

        let crawler_bandwidth_bytes_per_second = HistogramVec::new(
            HistogramOpts::new(
                "econgraph_crawler_bandwidth_bytes_per_second",
                "Bandwidth usage per source in bytes per second",
            )
            .buckets(vec![1024.0, 10240.0, 102400.0, 1048576.0, 10485760.0, 104857600.0]),
            &["crawler_type", "source"],
        )?;
        registry.register(Box::new(crawler_bandwidth_bytes_per_second.clone()))?;

        let crawler_requests_per_second = HistogramVec::new(
            HistogramOpts::new(
                "econgraph_crawler_requests_per_second",
                "Requests per second per source",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]),
            &["crawler_type", "source"],
        )?;
        registry.register(Box::new(crawler_requests_per_second.clone()))?;

        let crawler_response_time_seconds = HistogramVec::new(
            HistogramOpts::new(
                "econgraph_crawler_response_time_seconds",
                "Response time in seconds per source",
            )
            .buckets(vec![0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
            &["crawler_type", "source"],
        )?;
        registry.register(Box::new(crawler_response_time_seconds.clone()))?;

        let crawler_memory_usage_bytes = IntGauge::new(
            "econgraph_crawler_memory_usage_bytes",
            "Memory usage in bytes per crawler type",
        )?;
        registry.register(Box::new(crawler_memory_usage_bytes.clone()))?;

        let crawler_cpu_usage_percent = IntGauge::new(
            "econgraph_crawler_cpu_usage_percent",
            "CPU usage percentage per crawler type",
        )?;
        registry.register(Box::new(crawler_cpu_usage_percent.clone()))?;

        // === POLITENESS AND COMPLIANCE ===

        let crawler_request_delays_seconds = HistogramVec::new(
            HistogramOpts::new(
                "econgraph_crawler_request_delays_seconds",
                "Request delays between requests per source",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
            &["crawler_type", "source"],
        )?;
        registry.register(Box::new(crawler_request_delays_seconds.clone()))?;

        let crawler_robots_txt_compliance = IntCounterVec::new(
            Opts::new("econgraph_crawler_robots_txt_compliance", "Robots.txt compliance checks"),
            &["crawler_type", "source", "compliance_status"],
        )?;
        registry.register(Box::new(crawler_robots_txt_compliance.clone()))?;

        let crawler_user_agent_usage = IntCounterVec::new(
            Opts::new("econgraph_crawler_user_agent_usage", "User agent usage tracking"),
            &["crawler_type", "source", "user_agent"],
        )?;
        registry.register(Box::new(crawler_user_agent_usage.clone()))?;

        let crawler_concurrent_requests = IntGauge::new(
            "econgraph_crawler_concurrent_requests",
            "Number of concurrent requests per source",
        )?;
        registry.register(Box::new(crawler_concurrent_requests.clone()))?;

        let crawler_politeness_compliance = IntCounterVec::new(
            Opts::new("econgraph_crawler_politeness_compliance", "Politeness policy compliance"),
            &["crawler_type", "source", "policy_type", "compliance_status"],
        )?;
        registry.register(Box::new(crawler_politeness_compliance.clone()))?;

        // === DATA QUALITY ===

        let crawler_data_validation_errors_total = IntCounterVec::new(
            Opts::new("econgraph_crawler_data_validation_errors_total", "Total number of data validation errors"),
            &["crawler_type", "source", "validation_type"],
        )?;
        registry.register(Box::new(crawler_data_validation_errors_total.clone()))?;

        let crawler_data_completeness_score = IntGauge::new(
            "econgraph_crawler_data_completeness_score",
            "Data completeness score per source (0-100)",
        )?;
        registry.register(Box::new(crawler_data_completeness_score.clone()))?;

        let crawler_data_accuracy_score = IntGauge::new(
            "econgraph_crawler_data_accuracy_score",
            "Data accuracy score per source (0-100)",
        )?;
        registry.register(Box::new(crawler_data_accuracy_score.clone()))?;

        let crawler_duplicate_data_total = IntCounterVec::new(
            Opts::new("econgraph_crawler_duplicate_data_total", "Total number of duplicate data items detected"),
            &["crawler_type", "source", "data_type"],
        )?;
        registry.register(Box::new(crawler_duplicate_data_total.clone()))?;

        Ok(CrawlerMetrics {
            // Core metrics
            crawler_requests_total,
            crawler_request_duration_seconds,
            crawler_items_collected_total,
            crawler_bytes_downloaded_total,
            crawler_errors_total,
            crawler_rate_limit_hits_total,
            crawler_retries_total,
            crawler_timeouts_total,
            crawler_workers_active,
            crawler_queue_size,
            crawler_last_success_timestamp,

            // Economic data crawler specific
            economic_series_discovered_total,
            economic_data_points_collected_total,
            economic_api_quota_usage,
            economic_data_freshness_hours,

            // SEC EDGAR crawler specific
            sec_filings_downloaded_total,
            sec_xbrl_files_processed_total,
            sec_taxonomy_components_downloaded_total,
            sec_filing_processing_duration_seconds,
            sec_xbrl_parsing_duration_seconds,
            sec_companies_crawled_total,
            sec_filing_sizes_bytes,

            // Bandwidth and performance
            crawler_bandwidth_bytes_per_second,
            crawler_requests_per_second,
            crawler_response_time_seconds,
            crawler_memory_usage_bytes,
            crawler_cpu_usage_percent,

            // Politeness and compliance
            crawler_request_delays_seconds,
            crawler_robots_txt_compliance,
            crawler_user_agent_usage,
            crawler_concurrent_requests,
            crawler_politeness_compliance,

            // Data quality
            crawler_data_validation_errors_total,
            crawler_data_completeness_score,
            crawler_data_accuracy_score,
            crawler_duplicate_data_total,
        })
    }

    // === CORE CRAWLER METRIC RECORDING FUNCTIONS ===

    /// Record a crawler request
    pub fn record_request(&self, crawler_type: &str, source: &str, endpoint: &str, status: &str, duration: f64) {
        self.crawler_requests_total
            .with_label_values(&[crawler_type, source, endpoint, status])
            .inc();

        self.crawler_request_duration_seconds
            .with_label_values(&[crawler_type, source, endpoint])
            .observe(duration);
    }

    /// Record items collected by crawler
    pub fn record_items_collected(&self, crawler_type: &str, source: &str, item_type: &str, count: u64) {
        self.crawler_items_collected_total
            .with_label_values(&[crawler_type, source, item_type])
            .inc_by(count);
    }

    /// Record bytes downloaded by crawler
    pub fn record_bytes_downloaded(&self, crawler_type: &str, source: &str, bytes: u64) {
        self.crawler_bytes_downloaded_total
            .with_label_values(&[crawler_type, source])
            .inc_by(bytes);
    }

    /// Record crawler error
    pub fn record_error(&self, crawler_type: &str, source: &str, error_type: &str) {
        self.crawler_errors_total
            .with_label_values(&[crawler_type, source, error_type])
            .inc();
    }

    /// Record rate limit hit
    pub fn record_rate_limit_hit(&self, crawler_type: &str, source: &str) {
        self.crawler_rate_limit_hits_total
            .with_label_values(&[crawler_type, source])
            .inc();
    }

    /// Record retry attempt
    pub fn record_retry(&self, crawler_type: &str, source: &str, retry_reason: &str) {
        self.crawler_retries_total
            .with_label_values(&[crawler_type, source, retry_reason])
            .inc();
    }

    /// Record timeout error
    pub fn record_timeout(&self, crawler_type: &str, source: &str) {
        self.crawler_timeouts_total
            .with_label_values(&[crawler_type, source])
            .inc();
    }

    /// Update active workers count
    pub fn update_active_workers(&self, count: i64) {
        self.crawler_workers_active.set(count);
    }

    /// Update queue size
    pub fn update_queue_size(&self, size: i64) {
        self.crawler_queue_size.set(size);
    }

    /// Update last success timestamp
    pub fn update_last_success_timestamp(&self, timestamp: i64) {
        self.crawler_last_success_timestamp.set(timestamp);
    }

    // === ECONOMIC DATA CRAWLER SPECIFIC FUNCTIONS ===

    /// Record economic series discovered
    pub fn record_economic_series_discovered(&self, source: &str, series_type: &str, count: u64) {
        self.economic_series_discovered_total
            .with_label_values(&[source, series_type])
            .inc_by(count);
    }

    /// Record economic data points collected
    pub fn record_economic_data_points_collected(&self, source: &str, series_id: &str, count: u64) {
        self.economic_data_points_collected_total
            .with_label_values(&[source, series_id])
            .inc_by(count);
    }

    /// Update API quota usage
    pub fn update_api_quota_usage(&self, usage_percent: i64) {
        self.economic_api_quota_usage.set(usage_percent);
    }

    /// Update data freshness
    pub fn update_data_freshness(&self, hours: i64) {
        self.economic_data_freshness_hours.set(hours);
    }

    // === SEC EDGAR CRAWLER SPECIFIC FUNCTIONS ===

    /// Record SEC filing downloaded
    pub fn record_sec_filing_downloaded(&self, form_type: &str, company: &str, status: &str) {
        self.sec_filings_downloaded_total
            .with_label_values(&[form_type, company, status])
            .inc();
    }

    /// Record XBRL file processed
    pub fn record_sec_xbrl_file_processed(&self, form_type: &str, company: &str, status: &str) {
        self.sec_xbrl_files_processed_total
            .with_label_values(&[form_type, company, status])
            .inc();
    }

    /// Record taxonomy component downloaded
    pub fn record_sec_taxonomy_component_downloaded(&self, taxonomy_type: &str, version: &str) {
        self.sec_taxonomy_components_downloaded_total
            .with_label_values(&[taxonomy_type, version])
            .inc();
    }

    /// Record filing processing duration
    pub fn record_sec_filing_processing_duration(&self, form_type: &str, company: &str, duration: f64) {
        self.sec_filing_processing_duration_seconds
            .with_label_values(&[form_type, company])
            .observe(duration);
    }

    /// Record XBRL parsing duration
    pub fn record_sec_xbrl_parsing_duration(&self, form_type: &str, company: &str, duration: f64) {
        self.sec_xbrl_parsing_duration_seconds
            .with_label_values(&[form_type, company])
            .observe(duration);
    }

    /// Record company crawled
    pub fn record_sec_company_crawled(&self, company: &str, status: &str) {
        self.sec_companies_crawled_total
            .with_label_values(&[company, status])
            .inc();
    }

    /// Record filing size
    pub fn record_sec_filing_size(&self, form_type: &str, company: &str, size_bytes: f64) {
        self.sec_filing_sizes_bytes
            .with_label_values(&[form_type, company])
            .observe(size_bytes);
    }

    // === BANDWIDTH AND PERFORMANCE FUNCTIONS ===

    /// Record bandwidth usage
    pub fn record_bandwidth_usage(&self, crawler_type: &str, source: &str, bytes_per_second: f64) {
        self.crawler_bandwidth_bytes_per_second
            .with_label_values(&[crawler_type, source])
            .observe(bytes_per_second);
    }

    /// Record requests per second
    pub fn record_requests_per_second(&self, crawler_type: &str, source: &str, rps: f64) {
        self.crawler_requests_per_second
            .with_label_values(&[crawler_type, source])
            .observe(rps);
    }

    /// Record response time
    pub fn record_response_time(&self, crawler_type: &str, source: &str, response_time: f64) {
        self.crawler_response_time_seconds
            .with_label_values(&[crawler_type, source])
            .observe(response_time);
    }

    /// Update memory usage
    pub fn update_memory_usage(&self, bytes: i64) {
        self.crawler_memory_usage_bytes.set(bytes);
    }

    /// Update CPU usage
    pub fn update_cpu_usage(&self, percent: i64) {
        self.crawler_cpu_usage_percent.set(percent);
    }

    // === POLITENESS AND COMPLIANCE FUNCTIONS ===

    /// Record request delay
    pub fn record_request_delay(&self, crawler_type: &str, source: &str, delay_seconds: f64) {
        self.crawler_request_delays_seconds
            .with_label_values(&[crawler_type, source])
            .observe(delay_seconds);
    }

    /// Record robots.txt compliance check
    pub fn record_robots_txt_compliance(&self, crawler_type: &str, source: &str, compliance_status: &str) {
        self.crawler_robots_txt_compliance
            .with_label_values(&[crawler_type, source, compliance_status])
            .inc();
    }

    /// Record user agent usage
    pub fn record_user_agent_usage(&self, crawler_type: &str, source: &str, user_agent: &str) {
        self.crawler_user_agent_usage
            .with_label_values(&[crawler_type, source, user_agent])
            .inc();
    }

    /// Update concurrent requests
    pub fn update_concurrent_requests(&self, count: i64) {
        self.crawler_concurrent_requests.set(count);
    }

    /// Record politeness compliance
    pub fn record_politeness_compliance(&self, crawler_type: &str, source: &str, policy_type: &str, compliance_status: &str) {
        self.crawler_politeness_compliance
            .with_label_values(&[crawler_type, source, policy_type, compliance_status])
            .inc();
    }

    // === DATA QUALITY FUNCTIONS ===

    /// Record data validation error
    pub fn record_data_validation_error(&self, crawler_type: &str, source: &str, validation_type: &str) {
        self.crawler_data_validation_errors_total
            .with_label_values(&[crawler_type, source, validation_type])
            .inc();
    }

    /// Update data completeness score
    pub fn update_data_completeness_score(&self, score: i64) {
        self.crawler_data_completeness_score.set(score);
    }

    /// Update data accuracy score
    pub fn update_data_accuracy_score(&self, score: i64) {
        self.crawler_data_accuracy_score.set(score);
    }

    /// Record duplicate data detection
    pub fn record_duplicate_data(&self, crawler_type: &str, source: &str, data_type: &str, count: u64) {
        self.crawler_duplicate_data_total
            .with_label_values(&[crawler_type, source, data_type])
            .inc_by(count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// Create a test-specific metrics instance with isolated registry
    fn create_test_crawler_metrics() -> anyhow::Result<CrawlerMetrics> {
        let registry = Registry::new();
        CrawlerMetrics::new(&registry)
    }

    #[tokio::test]
    #[serial]
    async fn test_crawler_metrics_initialization() {
        // Test that crawler metrics can be initialized without errors
        let _metrics = create_test_crawler_metrics().expect("Crawler metrics should initialize successfully");
    }

    #[tokio::test]
    #[serial]
    async fn test_core_metrics_recording() {
        let metrics = create_test_crawler_metrics().expect("Should create metrics");

        // Test core metric recording functions
        metrics.record_request("economic", "fred", "/api/series", "success", 1.5);
        metrics.record_items_collected("economic", "fred", "series", 10);
        metrics.record_bytes_downloaded("economic", "fred", 1024);
        metrics.record_error("economic", "fred", "timeout");
        metrics.record_rate_limit_hit("economic", "fred");
        metrics.record_retry("economic", "fred", "timeout");
        metrics.record_timeout("economic", "fred");

        // Test gauge updates
        metrics.update_active_workers(5);
        metrics.update_queue_size(100);
        metrics.update_last_success_timestamp(1234567890);
    }

    #[tokio::test]
    #[serial]
    async fn test_economic_data_metrics_recording() {
        let metrics = create_test_crawler_metrics().expect("Should create metrics");

        // Test economic data specific metrics
        metrics.record_economic_series_discovered("fred", "economic", 5);
        metrics.record_economic_data_points_collected("fred", "GDP", 1000);
        metrics.update_api_quota_usage(75);
        metrics.update_data_freshness(24);
    }

    #[tokio::test]
    #[serial]
    async fn test_sec_edgar_metrics_recording() {
        let metrics = create_test_crawler_metrics().expect("Should create metrics");

        // Test SEC EDGAR specific metrics
        metrics.record_sec_filing_downloaded("10-K", "AAPL", "success");
        metrics.record_sec_xbrl_file_processed("10-K", "AAPL", "success");
        metrics.record_sec_taxonomy_component_downloaded("us-gaap", "2023");
        metrics.record_sec_filing_processing_duration("10-K", "AAPL", 30.0);
        metrics.record_sec_xbrl_parsing_duration("10-K", "AAPL", 5.0);
        metrics.record_sec_company_crawled("AAPL", "success");
        metrics.record_sec_filing_size("10-K", "AAPL", 1048576.0);
    }

    #[tokio::test]
    #[serial]
    async fn test_performance_metrics_recording() {
        let metrics = create_test_crawler_metrics().expect("Should create metrics");

        // Test performance metrics
        metrics.record_bandwidth_usage("economic", "fred", 1024.0);
        metrics.record_requests_per_second("economic", "fred", 2.0);
        metrics.record_response_time("economic", "fred", 0.5);
        metrics.update_memory_usage(1048576);
        metrics.update_cpu_usage(25);
    }

    #[tokio::test]
    #[serial]
    async fn test_politeness_metrics_recording() {
        let metrics = create_test_crawler_metrics().expect("Should create metrics");

        // Test politeness metrics
        metrics.record_request_delay("economic", "fred", 2.0);
        metrics.record_robots_txt_compliance("economic", "fred", "compliant");
        metrics.record_user_agent_usage("economic", "fred", "EconGraph-Crawler/1.0");
        metrics.update_concurrent_requests(3);
        metrics.record_politeness_compliance("economic", "fred", "rate_limit", "compliant");
    }

    #[tokio::test]
    #[serial]
    async fn test_data_quality_metrics_recording() {
        let metrics = create_test_crawler_metrics().expect("Should create metrics");

        // Test data quality metrics
        metrics.record_data_validation_error("economic", "fred", "schema");
        metrics.update_data_completeness_score(95);
        metrics.update_data_accuracy_score(98);
        metrics.record_duplicate_data("economic", "fred", "series", 5);
    }
}
