use crate::DEFAULT_REGISTRY;
use once_cell::sync::Lazy;
use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
};

pub struct CrawlerMetrics {
    pub crawler_requests_total: IntCounterVec,
    pub crawler_request_duration_seconds: HistogramVec,
    pub crawler_items_collected_total: IntCounterVec,
    pub crawler_bytes_downloaded_total: IntCounterVec,
    pub crawler_errors_total: IntCounterVec,
    pub crawler_rate_limit_hits_total: IntCounterVec,
    pub crawler_retries_total: IntCounterVec,
    pub crawler_timeouts_total: IntCounterVec,
}

impl CrawlerMetrics {
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

    pub fn record_bytes_downloaded(&self, crawler_type: &str, source: &str, bytes: u64) {
        self.crawler_bytes_downloaded_total
            .with_label_values(&[crawler_type, source])
            .inc_by(bytes);
    }

    pub fn record_error(&self, crawler_type: &str, source: &str, error_type: &str) {
        self.crawler_errors_total
            .with_label_values(&[crawler_type, source, error_type])
            .inc();
    }

    pub fn record_rate_limit_hit(&self, crawler_type: &str, source: &str) {
        self.crawler_rate_limit_hits_total
            .with_label_values(&[crawler_type, source])
            .inc();
    }

    pub fn record_retry(&self, crawler_type: &str, source: &str, retry_reason: &str) {
        self.crawler_retries_total
            .with_label_values(&[crawler_type, source, retry_reason])
            .inc();
    }

    pub fn record_timeout(&self, crawler_type: &str, source: &str) {
        self.crawler_timeouts_total
            .with_label_values(&[crawler_type, source])
            .inc();
    }
}

pub static CRAWLER_METRICS: Lazy<CrawlerMetrics> = Lazy::new(|| {
    CrawlerMetrics::new(&DEFAULT_REGISTRY).expect("Failed to initialize crawler metrics")
});
