use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Metrics collection for financial data service
///
/// Tracks performance metrics, operation counts, and system health
/// for monitoring and alerting purposes.
#[derive(Clone)]
pub struct MetricsCollector {
    metrics: Arc<RwLock<HashMap<String, MetricValue>>>,
    start_time: Instant,
}

#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Timer(Duration),
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }

    /// Increment a counter metric
    pub async fn increment_counter(&self, name: &str, value: u64) {
        let mut metrics = self.metrics.write().await;
        let current = metrics
            .get(name)
            .and_then(|v| match v {
                MetricValue::Counter(c) => Some(*c),
                _ => None,
            })
            .unwrap_or(0);
        metrics.insert(name.to_string(), MetricValue::Counter(current + value));
        debug!(
            "Counter {} incremented by {} to {}",
            name,
            value,
            current + value
        );
    }

    /// Set a gauge metric
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        metrics.insert(name.to_string(), MetricValue::Gauge(value));
        debug!("Gauge {} set to {}", name, value);
    }

    /// Record a histogram value
    pub async fn record_histogram(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        let current = metrics
            .get(name)
            .and_then(|v| match v {
                MetricValue::Histogram(h) => Some(h.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let mut new_histogram = current;
        new_histogram.push(value);

        // Keep only last 1000 values to prevent memory growth
        if new_histogram.len() > 1000 {
            new_histogram.drain(0..new_histogram.len() - 1000);
        }

        metrics.insert(name.to_string(), MetricValue::Histogram(new_histogram));
        debug!("Histogram {} recorded value {}", name, value);
    }

    /// Record operation timing
    pub async fn record_timing(&self, name: &str, duration: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.insert(name.to_string(), MetricValue::Timer(duration));
        debug!("Timer {} recorded duration {:?}", name, duration);
    }

    /// Get all metrics as a summary
    pub async fn get_metrics_summary(&self) -> HashMap<String, String> {
        let metrics = self.metrics.read().await;
        let mut summary = HashMap::new();

        for (name, value) in metrics.iter() {
            let summary_value = match value {
                MetricValue::Counter(c) => format!("{}", c),
                MetricValue::Gauge(g) => format!("{:.2}", g),
                MetricValue::Histogram(h) => {
                    if h.is_empty() {
                        "0".to_string()
                    } else {
                        let avg = h.iter().sum::<f64>() / h.len() as f64;
                        let min = h.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                        let max = h.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                        format!(
                            "count={}, avg={:.2}, min={:.2}, max={:.2}",
                            h.len(),
                            avg,
                            min,
                            max
                        )
                    }
                }
                MetricValue::Timer(d) => format!("{:?}", d),
            };
            summary.insert(name.clone(), summary_value);
        }

        // Add uptime
        let uptime = self.start_time.elapsed();
        summary.insert(
            "uptime_seconds".to_string(),
            format!("{:.2}", uptime.as_secs_f64()),
        );

        summary
    }

    /// Record GraphQL operation metrics
    pub async fn record_graphql_operation(
        &self,
        operation: &str,
        duration: Duration,
        success: bool,
    ) {
        self.record_timing(&format!("graphql_{}_duration", operation), duration)
            .await;
        self.increment_counter(&format!("graphql_{}_total", operation), 1)
            .await;

        if success {
            self.increment_counter(&format!("graphql_{}_success", operation), 1)
                .await;
        } else {
            self.increment_counter(&format!("graphql_{}_errors", operation), 1)
                .await;
        }
    }

    /// Record storage operation metrics
    pub async fn record_storage_operation(
        &self,
        operation: &str,
        duration: Duration,
        success: bool,
    ) {
        self.record_timing(&format!("storage_{}_duration", operation), duration)
            .await;
        self.increment_counter(&format!("storage_{}_total", operation), 1)
            .await;

        if success {
            self.increment_counter(&format!("storage_{}_success", operation), 1)
                .await;
        } else {
            self.increment_counter(&format!("storage_{}_errors", operation), 1)
                .await;
        }
    }
}

/// Health checker for the financial data service
///
/// Monitors system health, database connectivity, and service availability
#[derive(Clone)]
pub struct HealthChecker {
    checks: Arc<RwLock<HashMap<String, HealthCheck>>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    #[serde(skip)]
    pub last_check: Instant,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            checks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a health check
    pub async fn register_check(&self, name: String, check: HealthCheck) {
        let mut checks = self.checks.write().await;
        let name_clone = name.clone();
        checks.insert(name, check);
        info!("Health check '{}' registered", name_clone);
    }

    /// Update a health check status
    pub async fn update_check(&self, name: &str, status: HealthStatus, message: Option<String>) {
        let mut checks = self.checks.write().await;
        if let Some(check) = checks.get_mut(name) {
            let status_clone = status.clone();
            let message_clone = message.clone();
            check.status = status;
            check.last_check = Instant::now();
            check.message = message;

            match status_clone {
                HealthStatus::Healthy => debug!("Health check '{}' is healthy", name),
                HealthStatus::Degraded => {
                    warn!("Health check '{}' is degraded: {:?}", name, message_clone)
                }
                HealthStatus::Unhealthy => {
                    error!("Health check '{}' is unhealthy: {:?}", name, message_clone)
                }
            }
        }
    }

    /// Get overall health status
    pub async fn get_overall_health(&self) -> HealthStatus {
        let checks = self.checks.read().await;

        if checks.is_empty() {
            return HealthStatus::Healthy;
        }

        let mut has_unhealthy = false;
        let mut has_degraded = false;

        for check in checks.values() {
            match check.status {
                HealthStatus::Unhealthy => has_unhealthy = true,
                HealthStatus::Degraded => has_degraded = true,
                HealthStatus::Healthy => {}
            }
        }

        if has_unhealthy {
            HealthStatus::Unhealthy
        } else if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    /// Get detailed health report
    pub async fn get_health_report(&self) -> HashMap<String, HealthCheck> {
        let checks = self.checks.read().await;
        checks.clone()
    }

    /// Check database connectivity
    pub async fn check_database_health(&self, database: &crate::database::Database) {
        let start = Instant::now();

        match database.list_series().await {
            Ok(_) => {
                let duration = start.elapsed();
                self.update_check(
                    "database_connectivity",
                    HealthStatus::Healthy,
                    Some(format!("Database responsive in {:?}", duration)),
                )
                .await;
            }
            Err(e) => {
                self.update_check(
                    "database_connectivity",
                    HealthStatus::Unhealthy,
                    Some(format!("Database error: {}", e)),
                )
                .await;
            }
        }
    }

    /// Check storage health
    pub async fn check_storage_health(&self, storage: &dyn crate::storage::FinancialDataStorage) {
        let start = Instant::now();

        match storage.list_series().await {
            Ok(series) => {
                let duration = start.elapsed();
                let series_count = series.len();
                self.update_check(
                    "storage_health",
                    HealthStatus::Healthy,
                    Some(format!(
                        "Storage responsive, {} series in {:?}",
                        series_count, duration
                    )),
                )
                .await;
            }
            Err(e) => {
                self.update_check(
                    "storage_health",
                    HealthStatus::Unhealthy,
                    Some(format!("Storage error: {}", e)),
                )
                .await;
            }
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}
