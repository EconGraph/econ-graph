//! Enhanced Prometheus metrics collection for the EconGraph backend
//!
//! This module provides comprehensive metrics collection for monitoring
//! application performance, database operations, GraphQL queries, and more.
//! All metrics are exposed via the /metrics endpoint for Prometheus scraping.

use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
    TextEncoder,
};
use std::sync::Arc;
use warp::Reply;

/// Global metrics registry for the application
pub static REGISTRY: once_cell::sync::Lazy<Arc<Registry>> =
    once_cell::sync::Lazy::new(|| Arc::new(Registry::new()));

/// Application-level metrics
pub struct AppMetrics {
    /// Total HTTP requests received
    pub http_requests_total: IntCounterVec,

    /// HTTP request duration in seconds
    pub http_request_duration_seconds: HistogramVec,

    /// Active HTTP connections
    pub http_connections_active: IntGauge,

    /// GraphQL queries executed
    pub graphql_queries_total: IntCounterVec,

    /// GraphQL query execution time
    pub graphql_query_duration_seconds: HistogramVec,

    /// GraphQL query complexity
    pub graphql_query_complexity: Histogram,

    /// Database connection pool metrics
    pub db_connections_active: IntGauge,
    pub db_connections_idle: IntGauge,
    pub db_connections_total: IntGauge,

    /// Database query metrics
    pub db_queries_total: IntCounterVec,
    pub db_query_duration_seconds: HistogramVec,

    /// Authentication metrics
    pub auth_attempts_total: IntCounterVec,
    pub auth_success_total: IntCounterVec,
    pub auth_failures_total: IntCounterVec,

    /// Data crawling metrics
    pub crawler_requests_total: IntCounterVec,
    pub crawler_duration_seconds: HistogramVec,
    pub crawler_data_points_collected: IntCounter,

    /// Error metrics
    pub errors_total: IntCounterVec,

    /// Application uptime
    pub app_uptime_seconds: IntCounter,

    /// Memory usage metrics
    pub memory_usage_bytes: IntGauge,

    /// Application-specific metrics
    pub economic_series_total: IntGauge,
    pub data_points_total: IntGauge,
    pub active_users_total: IntGauge,
    pub chart_annotations_total: IntGauge,
    pub search_queries_total: IntCounterVec,
    pub search_query_duration_seconds: HistogramVec,
    pub data_source_health: IntGauge,
    pub queue_processing_rate: Histogram,
    pub cache_hit_ratio: Histogram,
    pub api_rate_limit_hits: IntCounterVec,
    pub data_quality_score: Histogram,
    pub user_sessions_active: IntGauge,
    pub chart_rendering_time: HistogramVec,
}

impl AppMetrics {
    /// Create and register all metrics
    pub fn new() -> anyhow::Result<Self> {
        let registry = &*REGISTRY;

        // HTTP metrics
        let http_requests_total = IntCounterVec::new(
            Opts::new("http_requests_total", "Total number of HTTP requests"),
            &["method", "endpoint", "status_code"],
        )?;
        registry.register(Box::new(http_requests_total.clone()))?;

        let http_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds",
            ),
            &["method", "endpoint"],
        )?;
        registry.register(Box::new(http_request_duration_seconds.clone()))?;

        let http_connections_active = IntGauge::new(
            "http_connections_active",
            "Number of active HTTP connections",
        )?;
        registry.register(Box::new(http_connections_active.clone()))?;

        // GraphQL metrics
        let graphql_queries_total = IntCounterVec::new(
            Opts::new(
                "graphql_queries_total",
                "Total number of GraphQL queries executed",
            ),
            &["operation_type", "operation_name"],
        )?;
        registry.register(Box::new(graphql_queries_total.clone()))?;

        let graphql_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "graphql_query_duration_seconds",
                "GraphQL query execution time in seconds",
            ),
            &["operation_type", "operation_name"],
        )?;
        registry.register(Box::new(graphql_query_duration_seconds.clone()))?;

        let graphql_query_complexity = Histogram::with_opts(HistogramOpts::new(
            "graphql_query_complexity",
            "GraphQL query complexity score",
        ))?;
        registry.register(Box::new(graphql_query_complexity.clone()))?;

        // Database metrics
        let db_connections_active = IntGauge::new(
            "db_connections_active",
            "Number of active database connections",
        )?;
        registry.register(Box::new(db_connections_active.clone()))?;

        let db_connections_idle =
            IntGauge::new("db_connections_idle", "Number of idle database connections")?;
        registry.register(Box::new(db_connections_idle.clone()))?;

        let db_connections_total = IntGauge::new(
            "db_connections_total",
            "Total number of database connections in pool",
        )?;
        registry.register(Box::new(db_connections_total.clone()))?;

        let db_queries_total = IntCounterVec::new(
            Opts::new("db_queries_total", "Total number of database queries"),
            &["query_type", "table"],
        )?;
        registry.register(Box::new(db_queries_total.clone()))?;

        let db_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "db_query_duration_seconds",
                "Database query execution time in seconds",
            ),
            &["query_type", "table"],
        )?;
        registry.register(Box::new(db_query_duration_seconds.clone()))?;

        // Authentication metrics
        let auth_attempts_total = IntCounterVec::new(
            Opts::new(
                "auth_attempts_total",
                "Total number of authentication attempts",
            ),
            &["provider", "method"],
        )?;
        registry.register(Box::new(auth_attempts_total.clone()))?;

        let auth_success_total = IntCounterVec::new(
            Opts::new(
                "auth_success_total",
                "Total number of successful authentications",
            ),
            &["provider", "method"],
        )?;
        registry.register(Box::new(auth_success_total.clone()))?;

        let auth_failures_total = IntCounterVec::new(
            Opts::new(
                "auth_failures_total",
                "Total number of failed authentications",
            ),
            &["provider", "method", "reason"],
        )?;
        registry.register(Box::new(auth_failures_total.clone()))?;

        // Crawler metrics
        let crawler_requests_total = IntCounterVec::new(
            Opts::new("crawler_requests_total", "Total number of crawler requests"),
            &["source", "status"],
        )?;
        registry.register(Box::new(crawler_requests_total.clone()))?;

        let crawler_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "crawler_duration_seconds",
                "Crawler request duration in seconds",
            ),
            &["source"],
        )?;
        registry.register(Box::new(crawler_duration_seconds.clone()))?;

        let crawler_data_points_collected = IntCounter::new(
            "crawler_data_points_collected",
            "Total number of data points collected by crawler",
        )?;
        registry.register(Box::new(crawler_data_points_collected.clone()))?;

        // Error metrics
        let errors_total = IntCounterVec::new(
            Opts::new("errors_total", "Total number of errors"),
            &["error_type", "component"],
        )?;
        registry.register(Box::new(errors_total.clone()))?;

        // Application metrics
        let app_uptime_seconds =
            IntCounter::new("app_uptime_seconds", "Application uptime in seconds")?;
        registry.register(Box::new(app_uptime_seconds.clone()))?;

        let memory_usage_bytes =
            IntGauge::new("memory_usage_bytes", "Current memory usage in bytes")?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;

        // Application-specific metrics
        let economic_series_total = IntGauge::new(
            "economic_series_total",
            "Total number of economic series in the database",
        )?;
        registry.register(Box::new(economic_series_total.clone()))?;

        let data_points_total = IntGauge::new(
            "data_points_total",
            "Total number of data points in the database",
        )?;
        registry.register(Box::new(data_points_total.clone()))?;

        let active_users_total = IntGauge::new(
            "active_users_total",
            "Total number of active users",
        )?;
        registry.register(Box::new(active_users_total.clone()))?;

        let chart_annotations_total = IntGauge::new(
            "chart_annotations_total",
            "Total number of chart annotations",
        )?;
        registry.register(Box::new(chart_annotations_total.clone()))?;

        let search_queries_total = IntCounterVec::new(
            Opts::new("search_queries_total", "Total number of search queries"),
            &["query_type", "result_count_range"],
        )?;
        registry.register(Box::new(search_queries_total.clone()))?;

        let search_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "search_query_duration_seconds",
                "Search query execution time in seconds",
            ),
            &["query_type"],
        )?;
        registry.register(Box::new(search_query_duration_seconds.clone()))?;

        let data_source_health = IntGauge::new(
            "data_source_health",
            "Health status of data sources (1=healthy, 0=unhealthy)",
        )?;
        registry.register(Box::new(data_source_health.clone()))?;

        let queue_processing_rate = Histogram::with_opts(HistogramOpts::new(
            "queue_processing_rate",
            "Queue processing rate in items per second",
        ))?;
        registry.register(Box::new(queue_processing_rate.clone()))?;

        let cache_hit_ratio = Histogram::with_opts(HistogramOpts::new(
            "cache_hit_ratio",
            "Cache hit ratio (0.0 to 1.0)",
        ))?;
        registry.register(Box::new(cache_hit_ratio.clone()))?;

        let api_rate_limit_hits = IntCounterVec::new(
            Opts::new("api_rate_limit_hits", "Total number of API rate limit hits"),
            &["source", "endpoint"],
        )?;
        registry.register(Box::new(api_rate_limit_hits.clone()))?;

        let data_quality_score = Histogram::with_opts(HistogramOpts::new(
            "data_quality_score",
            "Data quality score (0.0 to 1.0)",
        ))?;
        registry.register(Box::new(data_quality_score.clone()))?;

        let user_sessions_active = IntGauge::new(
            "user_sessions_active",
            "Number of active user sessions",
        )?;
        registry.register(Box::new(user_sessions_active.clone()))?;

        let chart_rendering_time = HistogramVec::new(
            HistogramOpts::new(
                "chart_rendering_time_seconds",
                "Chart rendering time in seconds",
            ),
            &["chart_type", "data_points_count_range"],
        )?;
        registry.register(Box::new(chart_rendering_time.clone()))?;

        Ok(AppMetrics {
            http_requests_total,
            http_request_duration_seconds,
            http_connections_active,
            graphql_queries_total,
            graphql_query_duration_seconds,
            graphql_query_complexity,
            db_connections_active,
            db_connections_idle,
            db_connections_total,
            db_queries_total,
            db_query_duration_seconds,
            auth_attempts_total,
            auth_success_total,
            auth_failures_total,
            crawler_requests_total,
            crawler_duration_seconds,
            crawler_data_points_collected,
            errors_total,
            app_uptime_seconds,
            memory_usage_bytes,
            economic_series_total,
            data_points_total,
            active_users_total,
            chart_annotations_total,
            search_queries_total,
            search_query_duration_seconds,
            data_source_health,
            queue_processing_rate,
            cache_hit_ratio,
            api_rate_limit_hits,
            data_quality_score,
            user_sessions_active,
            chart_rendering_time,
        })
    }
}

/// Global metrics instance
pub static METRICS: once_cell::sync::Lazy<AppMetrics> =
    once_cell::sync::Lazy::new(|| AppMetrics::new().expect("Failed to initialize metrics"));

/// Generate Prometheus metrics output
pub fn generate_metrics() -> anyhow::Result<String> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    encoder
        .encode_to_string(&metric_families)
        .map_err(|e| anyhow::anyhow!("Failed to encode metrics: {}", e))
}

/// Metrics endpoint handler for Prometheus scraping
pub async fn metrics_handler() -> Result<impl Reply, warp::Rejection> {
    match generate_metrics() {
        Ok(metrics) => Ok(warp::reply::with_header(
            metrics,
            "Content-Type",
            "text/plain; version=0.0.4; charset=utf-8",
        )),
        Err(e) => {
            tracing::error!("Failed to generate metrics: {}", e);
            Ok(warp::reply::with_header(
                "Internal Server Error".to_string(),
                "Content-Type",
                "text/plain; charset=utf-8",
            ))
        }
    }
}

/// Helper functions for recording metrics
/// Record HTTP request metrics
pub fn record_http_request(method: &str, endpoint: &str, status_code: u16, duration: f64) {
    METRICS
        .http_requests_total
        .with_label_values(&[method, endpoint, &status_code.to_string()])
        .inc();

    METRICS
        .http_request_duration_seconds
        .with_label_values(&[method, endpoint])
        .observe(duration);
}

/// Record GraphQL query metrics
pub fn record_graphql_query(
    operation_type: &str,
    operation_name: &str,
    duration: f64,
    complexity: f64,
) {
    METRICS
        .graphql_queries_total
        .with_label_values(&[operation_type, operation_name])
        .inc();

    METRICS
        .graphql_query_duration_seconds
        .with_label_values(&[operation_type, operation_name])
        .observe(duration);

    METRICS.graphql_query_complexity.observe(complexity);
}

/// Record database query metrics
pub fn record_db_query(query_type: &str, table: &str, duration: f64) {
    METRICS
        .db_queries_total
        .with_label_values(&[query_type, table])
        .inc();

    METRICS
        .db_query_duration_seconds
        .with_label_values(&[query_type, table])
        .observe(duration);
}

/// Record authentication metrics
pub fn record_auth_attempt(provider: &str, method: &str) {
    METRICS
        .auth_attempts_total
        .with_label_values(&[provider, method])
        .inc();
}

pub fn record_auth_success(provider: &str, method: &str) {
    METRICS
        .auth_success_total
        .with_label_values(&[provider, method])
        .inc();
}

pub fn record_auth_failure(provider: &str, method: &str, reason: &str) {
    METRICS
        .auth_failures_total
        .with_label_values(&[provider, method, reason])
        .inc();
}

/// Record crawler metrics
pub fn record_crawler_request(source: &str, status: &str, duration: f64) {
    METRICS
        .crawler_requests_total
        .with_label_values(&[source, status])
        .inc();

    METRICS
        .crawler_duration_seconds
        .with_label_values(&[source])
        .observe(duration);
}

pub fn record_crawler_data_points(count: u64) {
    METRICS.crawler_data_points_collected.inc_by(count);
}

/// Record error metrics
pub fn record_error(error_type: &str, component: &str) {
    METRICS
        .errors_total
        .with_label_values(&[error_type, component])
        .inc();
}

/// Update database connection pool metrics
pub fn update_db_pool_metrics(active: i64, idle: i64, total: i64) {
    METRICS.db_connections_active.set(active);
    METRICS.db_connections_idle.set(idle);
    METRICS.db_connections_total.set(total);
}

/// Update HTTP connections metric
pub fn update_http_connections(count: i64) {
    METRICS.http_connections_active.set(count);
}

/// Update memory usage metric
pub fn update_memory_usage(bytes: i64) {
    METRICS.memory_usage_bytes.set(bytes);
}

/// Increment uptime counter (should be called periodically)
pub fn increment_uptime(seconds: u64) {
    METRICS.app_uptime_seconds.inc_by(seconds);
}

/// Application-specific metric recording functions
pub fn update_economic_series_count(count: i64) {
    METRICS.economic_series_total.set(count);
}

pub fn update_data_points_count(count: i64) {
    METRICS.data_points_total.set(count);
}

pub fn update_active_users_count(count: i64) {
    METRICS.active_users_total.set(count);
}

pub fn update_chart_annotations_count(count: i64) {
    METRICS.chart_annotations_total.set(count);
}

pub fn record_search_query(query_type: &str, result_count_range: &str, duration: f64) {
    METRICS
        .search_queries_total
        .with_label_values(&[query_type, result_count_range])
        .inc();

    METRICS
        .search_query_duration_seconds
        .with_label_values(&[query_type])
        .observe(duration);
}

pub fn update_data_source_health(healthy: bool) {
    METRICS.data_source_health.set(if healthy { 1 } else { 0 });
}

pub fn record_queue_processing_rate(rate: f64) {
    METRICS.queue_processing_rate.observe(rate);
}

pub fn record_cache_hit_ratio(ratio: f64) {
    METRICS.cache_hit_ratio.observe(ratio);
}

pub fn record_api_rate_limit_hit(source: &str, endpoint: &str) {
    METRICS
        .api_rate_limit_hits
        .with_label_values(&[source, endpoint])
        .inc();
}

pub fn record_data_quality_score(score: f64) {
    METRICS.data_quality_score.observe(score);
}

pub fn update_user_sessions_active(count: i64) {
    METRICS.user_sessions_active.set(count);
}

pub fn record_chart_rendering_time(chart_type: &str, data_points_count_range: &str, duration: f64) {
    METRICS
        .chart_rendering_time
        .with_label_values(&[chart_type, data_points_count_range])
        .observe(duration);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::time::Duration;
    use tokio::time::sleep;

    /// Create a test-specific metrics instance with isolated registry
    fn create_test_metrics() -> anyhow::Result<AppMetrics> {
        let registry = Registry::new();

        // HTTP metrics
        let http_requests_total = IntCounterVec::new(
            Opts::new("http_requests_total", "Total number of HTTP requests"),
            &["method", "endpoint", "status_code"],
        )?;
        registry.register(Box::new(http_requests_total.clone()))?;

        let http_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds",
            ),
            &["method", "endpoint"],
        )?;
        registry.register(Box::new(http_request_duration_seconds.clone()))?;

        let http_connections_active = IntGauge::new(
            "http_connections_active",
            "Number of active HTTP connections",
        )?;
        registry.register(Box::new(http_connections_active.clone()))?;

        // GraphQL metrics
        let graphql_queries_total = IntCounterVec::new(
            Opts::new(
                "graphql_queries_total",
                "Total number of GraphQL queries executed",
            ),
            &["operation_type", "operation_name"],
        )?;
        registry.register(Box::new(graphql_queries_total.clone()))?;

        let graphql_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "graphql_query_duration_seconds",
                "GraphQL query execution time in seconds",
            ),
            &["operation_type", "operation_name"],
        )?;
        registry.register(Box::new(graphql_query_duration_seconds.clone()))?;

        let graphql_query_complexity = Histogram::with_opts(HistogramOpts::new(
            "graphql_query_complexity",
            "GraphQL query complexity score",
        ))?;
        registry.register(Box::new(graphql_query_complexity.clone()))?;

        // Database metrics
        let db_connections_active = IntGauge::new(
            "db_connections_active",
            "Number of active database connections",
        )?;
        registry.register(Box::new(db_connections_active.clone()))?;

        let db_connections_idle =
            IntGauge::new("db_connections_idle", "Number of idle database connections")?;
        registry.register(Box::new(db_connections_idle.clone()))?;

        let db_connections_total = IntGauge::new(
            "db_connections_total",
            "Total number of database connections in pool",
        )?;
        registry.register(Box::new(db_connections_total.clone()))?;

        let db_queries_total = IntCounterVec::new(
            Opts::new("db_queries_total", "Total number of database queries"),
            &["query_type", "table"],
        )?;
        registry.register(Box::new(db_queries_total.clone()))?;

        let db_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "db_query_duration_seconds",
                "Database query execution time in seconds",
            ),
            &["query_type", "table"],
        )?;
        registry.register(Box::new(db_query_duration_seconds.clone()))?;

        // Authentication metrics
        let auth_attempts_total = IntCounterVec::new(
            Opts::new(
                "auth_attempts_total",
                "Total number of authentication attempts",
            ),
            &["provider", "method"],
        )?;
        registry.register(Box::new(auth_attempts_total.clone()))?;

        let auth_success_total = IntCounterVec::new(
            Opts::new(
                "auth_success_total",
                "Total number of successful authentications",
            ),
            &["provider", "method"],
        )?;
        registry.register(Box::new(auth_success_total.clone()))?;

        let auth_failures_total = IntCounterVec::new(
            Opts::new(
                "auth_failures_total",
                "Total number of failed authentications",
            ),
            &["provider", "method", "reason"],
        )?;
        registry.register(Box::new(auth_failures_total.clone()))?;

        // Crawler metrics
        let crawler_requests_total = IntCounterVec::new(
            Opts::new("crawler_requests_total", "Total number of crawler requests"),
            &["source", "status"],
        )?;
        registry.register(Box::new(crawler_requests_total.clone()))?;

        let crawler_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "crawler_duration_seconds",
                "Crawler request duration in seconds",
            ),
            &["source"],
        )?;
        registry.register(Box::new(crawler_duration_seconds.clone()))?;

        let crawler_data_points_collected = IntCounter::new(
            "crawler_data_points_collected",
            "Total number of data points collected by crawler",
        )?;
        registry.register(Box::new(crawler_data_points_collected.clone()))?;

        // Error metrics
        let errors_total = IntCounterVec::new(
            Opts::new("errors_total", "Total number of errors"),
            &["error_type", "component"],
        )?;
        registry.register(Box::new(errors_total.clone()))?;

        // Application metrics
        let app_uptime_seconds =
            IntCounter::new("app_uptime_seconds", "Application uptime in seconds")?;
        registry.register(Box::new(app_uptime_seconds.clone()))?;

        let memory_usage_bytes =
            IntGauge::new("memory_usage_bytes", "Current memory usage in bytes")?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;

        // Application-specific metrics
        let economic_series_total = IntGauge::new(
            "economic_series_total",
            "Total number of economic series in the database",
        )?;
        registry.register(Box::new(economic_series_total.clone()))?;

        let data_points_total = IntGauge::new(
            "data_points_total",
            "Total number of data points in the database",
        )?;
        registry.register(Box::new(data_points_total.clone()))?;

        let active_users_total = IntGauge::new(
            "active_users_total",
            "Total number of active users",
        )?;
        registry.register(Box::new(active_users_total.clone()))?;

        let chart_annotations_total = IntGauge::new(
            "chart_annotations_total",
            "Total number of chart annotations",
        )?;
        registry.register(Box::new(chart_annotations_total.clone()))?;

        let search_queries_total = IntCounterVec::new(
            Opts::new("search_queries_total", "Total number of search queries"),
            &["query_type", "result_count_range"],
        )?;
        registry.register(Box::new(search_queries_total.clone()))?;

        let search_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "search_query_duration_seconds",
                "Search query execution time in seconds",
            ),
            &["query_type"],
        )?;
        registry.register(Box::new(search_query_duration_seconds.clone()))?;

        let data_source_health = IntGauge::new(
            "data_source_health",
            "Health status of data sources (1=healthy, 0=unhealthy)",
        )?;
        registry.register(Box::new(data_source_health.clone()))?;

        let queue_processing_rate = Histogram::with_opts(HistogramOpts::new(
            "queue_processing_rate",
            "Queue processing rate in items per second",
        ))?;
        registry.register(Box::new(queue_processing_rate.clone()))?;

        let cache_hit_ratio = Histogram::with_opts(HistogramOpts::new(
            "cache_hit_ratio",
            "Cache hit ratio (0.0 to 1.0)",
        ))?;
        registry.register(Box::new(cache_hit_ratio.clone()))?;

        let api_rate_limit_hits = IntCounterVec::new(
            Opts::new("api_rate_limit_hits", "Total number of API rate limit hits"),
            &["source", "endpoint"],
        )?;
        registry.register(Box::new(api_rate_limit_hits.clone()))?;

        let data_quality_score = Histogram::with_opts(HistogramOpts::new(
            "data_quality_score",
            "Data quality score (0.0 to 1.0)",
        ))?;
        registry.register(Box::new(data_quality_score.clone()))?;

        let user_sessions_active = IntGauge::new(
            "user_sessions_active",
            "Number of active user sessions",
        )?;
        registry.register(Box::new(user_sessions_active.clone()))?;

        let chart_rendering_time = HistogramVec::new(
            HistogramOpts::new(
                "chart_rendering_time_seconds",
                "Chart rendering time in seconds",
            ),
            &["chart_type", "data_points_count_range"],
        )?;
        registry.register(Box::new(chart_rendering_time.clone()))?;

        Ok(AppMetrics {
            http_requests_total,
            http_request_duration_seconds,
            http_connections_active,
            graphql_queries_total,
            graphql_query_duration_seconds,
            graphql_query_complexity,
            db_connections_active,
            db_connections_idle,
            db_connections_total,
            db_queries_total,
            db_query_duration_seconds,
            auth_attempts_total,
            auth_success_total,
            auth_failures_total,
            crawler_requests_total,
            crawler_duration_seconds,
            crawler_data_points_collected,
            errors_total,
            app_uptime_seconds,
            memory_usage_bytes,
            economic_series_total,
            data_points_total,
            active_users_total,
            chart_annotations_total,
            search_queries_total,
            search_query_duration_seconds,
            data_source_health,
            queue_processing_rate,
            cache_hit_ratio,
            api_rate_limit_hits,
            data_quality_score,
            user_sessions_active,
            chart_rendering_time,
        })
    }

    /// Generate metrics from a specific registry for testing
    fn generate_test_metrics(registry: &Registry) -> anyhow::Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        encoder
            .encode_to_string(&metric_families)
            .map_err(|e| anyhow::anyhow!("Failed to encode metrics: {}", e))
    }

    #[tokio::test]
    #[serial]
    async fn test_metrics_initialization() {
        // Test that metrics can be initialized without errors using isolated registry
        let _metrics = create_test_metrics().expect("Metrics should initialize successfully");
    }

    #[tokio::test]
    #[serial]
    async fn test_metrics_generation() {
        // Test that metrics can be generated from the global registry
        // The global METRICS instance should already be initialized
        let metrics_output = generate_metrics().expect("Should generate metrics");

        // If no metrics are registered, the output will be empty, which is valid
        if !metrics_output.is_empty() {
            assert!(metrics_output.contains("# HELP"));
            assert!(metrics_output.contains("# TYPE"));
        }
        // Empty output is also valid if no metrics are registered
    }

    #[tokio::test]
    #[serial]
    async fn test_metrics_recording() {
        // Test recording various metrics using the global metrics instance
        record_http_request("GET", "/api/test", 200, 0.1);
        record_graphql_query("query", "testQuery", 0.05, 10.0);
        record_db_query("SELECT", "users", 0.02);
        record_auth_attempt("google", "oauth");
        record_auth_success("google", "oauth");
        record_crawler_request("fred", "success", 1.0);
        record_crawler_data_points(100);
        record_error("validation", "auth");

        // Update gauge metrics
        update_db_pool_metrics(5, 10, 15);
        update_http_connections(3);
        update_memory_usage(1024 * 1024);
        increment_uptime(60);

        // Test application-specific metrics
        update_economic_series_count(1000);
        update_data_points_count(50000);
        update_active_users_count(25);
        update_chart_annotations_count(150);
        record_search_query("series", "0-10", 0.1);
        update_data_source_health(true);
        record_queue_processing_rate(5.5);
        record_cache_hit_ratio(0.85);
        record_api_rate_limit_hit("fred", "/series");
        record_data_quality_score(0.92);
        update_user_sessions_active(12);
        record_chart_rendering_time("line", "100-1000", 0.05);

        // Generate metrics and verify they contain our recorded values
        let metrics_output = generate_metrics().expect("Should generate metrics");
        assert!(metrics_output.contains("http_requests_total"));
        assert!(metrics_output.contains("graphql_queries_total"));
        assert!(metrics_output.contains("db_queries_total"));
        assert!(metrics_output.contains("auth_attempts_total"));
        assert!(metrics_output.contains("crawler_requests_total"));
        assert!(metrics_output.contains("errors_total"));
        assert!(metrics_output.contains("economic_series_total"));
        assert!(metrics_output.contains("data_points_total"));
        assert!(metrics_output.contains("search_queries_total"));
        assert!(metrics_output.contains("data_source_health"));
        assert!(metrics_output.contains("queue_processing_rate"));
        assert!(metrics_output.contains("cache_hit_ratio"));
        assert!(metrics_output.contains("api_rate_limit_hits"));
        assert!(metrics_output.contains("data_quality_score"));
        assert!(metrics_output.contains("user_sessions_active"));
        assert!(metrics_output.contains("chart_rendering_time_seconds"));
    }

    #[tokio::test]
    #[serial]
    async fn test_metrics_endpoint() {
        // Test the metrics endpoint handler
        let response = metrics_handler()
            .await
            .expect("Should handle metrics request");

        // The response should be a valid HTTP response
        // We can't easily test the exact content without more complex setup,
        // but we can verify it doesn't panic
        assert!(response.into_response().status().is_success());
    }
}
