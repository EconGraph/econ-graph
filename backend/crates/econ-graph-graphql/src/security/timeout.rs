//! # Query Timeout Management
//!
//! This module provides query timeout management to prevent long-running queries
//! from blocking the system and consuming excessive resources.
//!
//! # Timeout Strategy
//!
//! - Global timeout for all queries
//! - Per-query timeout configuration
//! - Timeout escalation and cleanup
//! - Resource monitoring and alerting
//!
//! # Security Benefits
//!
//! - Prevents resource exhaustion
//! - Ensures system responsiveness
//! - Protects against malicious long-running queries
//! - Provides predictable query execution times
//!
//! # Implementation
//!
//! Uses async timeouts with proper cleanup and resource management.
//! Integrates with the GraphQL execution context for seamless operation.

use std::time::Duration;
use tokio::time::{error::Elapsed, timeout};
use tracing::{debug, error, warn};

/// Timeout manager configuration
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Default query timeout in seconds
    pub default_timeout: u64,
    /// Maximum query timeout in seconds
    pub max_timeout: u64,
    /// Minimum query timeout in seconds
    pub min_timeout: u64,
    /// Enable timeout management
    pub enabled: bool,
    /// Timeout escalation factor
    pub escalation_factor: f64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout: 30, // 30 seconds
            max_timeout: 300,    // 5 minutes
            min_timeout: 5,      // 5 seconds
            enabled: true,
            escalation_factor: 1.5,
        }
    }
}

/// Timeout manager
pub struct TimeoutManager {
    /// Timeout configuration
    config: TimeoutConfig,
    /// Active timeout tracking
    active_timeouts: std::sync::Arc<std::sync::RwLock<Vec<TimeoutEntry>>>,
}

/// Timeout entry for tracking active queries
#[derive(Debug, Clone)]
pub struct TimeoutEntry {
    /// Query identifier
    query_id: String,
    /// Timeout duration
    timeout_duration: Duration,
    /// Start time
    start_time: std::time::Instant,
    /// Query description
    description: String,
}

impl TimeoutManager {
    /// Create a new timeout manager
    pub fn new(default_timeout: u64) -> Self {
        Self {
            config: TimeoutConfig {
                default_timeout,
                ..Default::default()
            },
            active_timeouts: std::sync::Arc::new(std::sync::RwLock::new(Vec::new())),
        }
    }

    /// Execute a query with timeout
    pub async fn execute_with_timeout<F, T>(
        &self,
        query_id: &str,
        query_description: &str,
        operation: F,
    ) -> Result<T, TimeoutError>
    where
        F: std::future::Future<Output = T>,
    {
        if !self.config.enabled {
            return Ok(operation.await);
        }

        let timeout_duration = Duration::from_secs(self.config.default_timeout);
        let start_time = std::time::Instant::now();

        // Register the timeout
        self.register_timeout(query_id, timeout_duration, query_description, start_time);

        // Execute with timeout
        let result = timeout(timeout_duration, operation).await;

        // Clean up the timeout entry
        self.unregister_timeout(query_id);

        match result {
            Ok(value) => {
                let elapsed = start_time.elapsed();
                debug!(
                    "Query '{}' completed in {:?} (timeout: {:?})",
                    query_id, elapsed, timeout_duration
                );
                Ok(value)
            }
            Err(_timeout_error) => {
                let elapsed = start_time.elapsed();
                warn!(
                    "Query '{}' timed out after {:?} (limit: {:?})",
                    query_id, elapsed, timeout_duration
                );
                Err(TimeoutError::QueryTimeout {
                    duration: Duration::from_secs(self.config.default_timeout),
                })
            }
        }
    }

    /// Execute a query with custom timeout
    pub async fn execute_with_custom_timeout<F, T>(
        &self,
        query_id: &str,
        query_description: &str,
        custom_timeout: u64,
        operation: F,
    ) -> Result<T, TimeoutError>
    where
        F: std::future::Future<Output = T>,
    {
        if !self.config.enabled {
            return Ok(operation.await);
        }

        // Validate custom timeout
        let timeout_duration = Duration::from_secs(
            custom_timeout
                .max(self.config.min_timeout)
                .min(self.config.max_timeout),
        );

        let start_time = std::time::Instant::now();

        // Register the timeout
        self.register_timeout(query_id, timeout_duration, query_description, start_time);

        // Execute with timeout
        let result = timeout(timeout_duration, operation).await;

        // Clean up the timeout entry
        self.unregister_timeout(query_id);

        match result {
            Ok(value) => {
                let elapsed = start_time.elapsed();
                debug!(
                    "Query '{}' completed in {:?} (custom timeout: {:?})",
                    query_id, elapsed, timeout_duration
                );
                Ok(value)
            }
            Err(_timeout_error) => {
                let elapsed = start_time.elapsed();
                warn!(
                    "Query '{}' timed out after {:?} (custom limit: {:?})",
                    query_id, elapsed, timeout_duration
                );
                Err(TimeoutError::QueryTimeout {
                    duration: Duration::from_secs(self.config.default_timeout),
                })
            }
        }
    }

    /// Register a timeout entry
    fn register_timeout(
        &self,
        query_id: &str,
        timeout_duration: Duration,
        description: &str,
        start_time: std::time::Instant,
    ) {
        let mut timeouts = self.active_timeouts.write().unwrap();
        timeouts.push(TimeoutEntry {
            query_id: query_id.to_string(),
            timeout_duration,
            start_time,
            description: description.to_string(),
        });
    }

    /// Unregister a timeout entry
    fn unregister_timeout(&self, query_id: &str) {
        let mut timeouts = self.active_timeouts.write().unwrap();
        timeouts.retain(|entry| entry.query_id != query_id);
    }

    /// Get active timeout entries
    pub fn get_active_timeouts(&self) -> Vec<TimeoutEntry> {
        let timeouts = self.active_timeouts.read().unwrap();
        timeouts.clone()
    }

    /// Clean up expired timeouts
    pub fn cleanup_expired_timeouts(&self) {
        let mut timeouts = self.active_timeouts.write().unwrap();
        let now = std::time::Instant::now();

        let initial_count = timeouts.len();
        timeouts.retain(|entry| now.duration_since(entry.start_time) < entry.timeout_duration);

        let cleaned_count = initial_count - timeouts.len();
        if cleaned_count > 0 {
            debug!("Cleaned up {} expired timeout entries", cleaned_count);
        }
    }

    /// Get timeout statistics
    pub fn get_timeout_statistics(&self) -> TimeoutStatistics {
        let timeouts = self.active_timeouts.read().unwrap();
        let now = std::time::Instant::now();

        let mut total_queries = 0;
        let mut active_queries = 0;
        let mut expired_queries = 0;
        let mut total_duration = Duration::from_secs(0);

        for entry in timeouts.iter() {
            total_queries += 1;
            let elapsed = now.duration_since(entry.start_time);

            if elapsed < entry.timeout_duration {
                active_queries += 1;
            } else {
                expired_queries += 1;
            }

            total_duration += elapsed;
        }

        let average_duration = if total_queries > 0 {
            total_duration / total_queries
        } else {
            Duration::from_secs(0)
        };

        TimeoutStatistics {
            total_queries: total_queries as usize,
            active_queries,
            expired_queries,
            average_duration,
            config: self.config.clone(),
        }
    }

    /// Update the configuration
    pub fn update_config(&mut self, config: TimeoutConfig) {
        self.config = config;
    }

    /// Get the current configuration
    pub fn config(&self) -> &TimeoutConfig {
        &self.config
    }

    /// Set the default timeout
    pub fn set_default_timeout(&mut self, timeout: u64) {
        self.config.default_timeout = timeout
            .max(self.config.min_timeout)
            .min(self.config.max_timeout);
    }

    /// Enable or disable timeout management
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }
}

/// Timeout statistics
#[derive(Debug, Clone)]
pub struct TimeoutStatistics {
    /// Total number of queries tracked
    pub total_queries: usize,
    /// Number of active queries
    pub active_queries: usize,
    /// Number of expired queries
    pub expired_queries: usize,
    /// Average query duration
    pub average_duration: Duration,
    /// Current configuration
    pub config: TimeoutConfig,
}

/// Timeout error types
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError {
    #[error("Query timed out after {duration:?}")]
    QueryTimeout { duration: Duration },
    #[error("Timeout configuration error: {message}")]
    ConfigurationError { message: String },
    #[error("Timeout management error: {message}")]
    ManagementError { message: String },
}

/// Timeout-aware GraphQL context
pub struct TimeoutContext {
    /// Timeout manager
    timeout_manager: std::sync::Arc<TimeoutManager>,
    /// Query identifier
    query_id: String,
    /// Query description
    query_description: String,
}

impl TimeoutContext {
    /// Create a new timeout context
    pub fn new(
        timeout_manager: std::sync::Arc<TimeoutManager>,
        query_id: String,
        query_description: String,
    ) -> Self {
        Self {
            timeout_manager,
            query_id,
            query_description,
        }
    }

    /// Execute an operation with timeout
    pub async fn execute_with_timeout<F, T>(&self, operation: F) -> Result<T, TimeoutError>
    where
        F: std::future::Future<Output = T>,
    {
        self.timeout_manager
            .execute_with_timeout(&self.query_id, &self.query_description, operation)
            .await
    }

    /// Execute an operation with custom timeout
    pub async fn execute_with_custom_timeout<F, T>(
        &self,
        custom_timeout: u64,
        operation: F,
    ) -> Result<T, TimeoutError>
    where
        F: std::future::Future<Output = T>,
    {
        self.timeout_manager
            .execute_with_custom_timeout(
                &self.query_id,
                &self.query_description,
                custom_timeout,
                operation,
            )
            .await
    }

    /// Get the query identifier
    pub fn query_id(&self) -> &str {
        &self.query_id
    }

    /// Get the query description
    pub fn query_description(&self) -> &str {
        &self.query_description
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_timeout_manager_basic() {
        let manager = TimeoutManager::new(1); // 1 second timeout

        let result = manager
            .execute_with_timeout("test_query", "Test query", async {
                // Simulate a quick operation
                "success"
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_timeout_manager_timeout() {
        let manager = TimeoutManager::new(1); // 1 second timeout

        let result = manager
            .execute_with_timeout("slow_query", "Slow query", async {
                // Simulate a slow operation
                sleep(Duration::from_secs(2)).await;
                "success"
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_timeout_manager_disabled() {
        let mut manager = TimeoutManager::new(1);
        manager.set_enabled(false);

        let result = manager
            .execute_with_timeout("slow_query", "Slow query", async {
                // Simulate a slow operation
                sleep(Duration::from_secs(2)).await;
                "success"
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_custom_timeout() {
        let manager = TimeoutManager::new(1);

        let result = manager
            .execute_with_custom_timeout("custom_query", "Custom query", 3, async {
                // Simulate a 2-second operation
                sleep(Duration::from_secs(2)).await;
                "success"
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_timeout_statistics() {
        let manager = TimeoutManager::new(1);

        // Execute a quick query
        let _ = manager
            .execute_with_timeout("quick_query", "Quick query", async { "success" })
            .await;

        let stats = manager.get_timeout_statistics();
        assert!(stats.total_queries >= 1);
    }
}
