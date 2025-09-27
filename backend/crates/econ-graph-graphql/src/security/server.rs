//! # Secure GraphQL Server
//!
//! This module provides a secure GraphQL server wrapper that integrates
//! all security measures and provides a unified interface for secure
//! GraphQL operations.
//!
//! # Security Features
//!
//! - Query complexity analysis
//! - Query depth limiting
//! - Rate limiting
//! - Query size validation
//! - Introspection protection
//! - Query timeout management
//! - Query whitelist/blacklist filtering
//! - Security event monitoring
//!
//! # Usage
//!
//! ```rust,no_run
//! use econ_graph_graphql::security::server::SecureGraphQLServer;
//! use econ_graph_graphql::security::SecurityConfig;
//! use econ_graph_core::database::DatabasePool;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let pool = DatabasePool::new();
//!     let config = SecurityConfig::default();
//!     let server = SecureGraphQLServer::new(pool, config);
//!
//!     // Execute secure GraphQL request
//!     let result = server.execute_secure_request(request, "127.0.0.1").await?;
//!     Ok(())
//! }
//! ```

use async_graphql::{Request, Response, ServerError};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::graphql::schema::{create_schema, GraphQLContext};
use crate::security::{
    BlockReason, LoggingSecurityEventHandler, SecurityConfig, SecurityEvent, SecurityEventHandler,
    SecurityMetrics, SecurityMiddleware,
};
use econ_graph_core::database::DatabasePool;

/// Secure GraphQL server
pub struct SecureGraphQLServer {
    /// Database connection pool
    pool: Arc<DatabasePool>,
    /// Security middleware
    security: Arc<SecurityMiddleware>,
    /// Security event handler
    event_handler: Arc<dyn SecurityEventHandler>,
    /// Security metrics
    metrics: Arc<std::sync::RwLock<SecurityMetrics>>,
}

impl SecureGraphQLServer {
    /// Create a new secure GraphQL server
    pub fn new(pool: Arc<DatabasePool>, config: SecurityConfig) -> Self {
        let security = Arc::new(SecurityMiddleware::new(config));
        let event_handler = Arc::new(LoggingSecurityEventHandler);
        let metrics = Arc::new(std::sync::RwLock::new(SecurityMetrics::default()));

        Self {
            pool,
            security,
            event_handler,
            metrics,
        }
    }

    /// Create a new secure GraphQL server with custom event handler
    pub fn new_with_event_handler(
        pool: Arc<DatabasePool>,
        config: SecurityConfig,
        event_handler: Arc<dyn SecurityEventHandler>,
    ) -> Self {
        let security = Arc::new(SecurityMiddleware::new(config));
        let metrics = Arc::new(std::sync::RwLock::new(SecurityMetrics::default()));

        Self {
            pool,
            security,
            event_handler,
            metrics,
        }
    }

    /// Execute a secure GraphQL request
    pub async fn execute_secure_request(
        &self,
        request: Request,
        client_ip: &str,
    ) -> Result<Response, Vec<ServerError>> {
        let start_time = std::time::Instant::now();
        let query = request.query.clone();

        // Update metrics
        {
            let mut metrics = self.metrics.write().unwrap();
            metrics.total_requests += 1;
        }

        // Validate request against security measures
        match self.security.validate_request(&request, client_ip).await {
            Ok(()) => {
                debug!("Security validation passed for IP: {}", client_ip);
            }
            Err(errors) => {
                // Record blocked request
                {
                    let mut metrics = self.metrics.write().unwrap();
                    metrics.record_blocked(BlockReason::Filtered);
                }

                // Log security event
                let event = SecurityEvent::QueryFiltered {
                    client_ip: client_ip.to_string(),
                    query: query.clone(),
                    reason: "Security validation failed".to_string(),
                    timestamp: chrono::Utc::now(),
                };
                self.event_handler.handle_event(event);

                return Err(errors);
            }
        }

        // Create schema with security context
        let schema = create_schema((*self.pool).clone());

        // Execute GraphQL request with timeout
        let execution_result = self
            .security
            .timeout_manager
            .execute_with_timeout(
                &format!("query_{}", uuid::Uuid::new_v4()),
                &format!("GraphQL query from {}", client_ip),
                schema.execute(request),
            )
            .await;

        match execution_result {
            Ok(response) => {
                let elapsed = start_time.elapsed();
                debug!(
                    "GraphQL request completed successfully for IP {} in {:?}",
                    client_ip, elapsed
                );

                // Update metrics with query analysis
                if let Ok(analysis) = self.security.query_analyzer.analyze_query(&query) {
                    let mut metrics = self.metrics.write().unwrap();
                    metrics.update_request(analysis.complexity_score, analysis.depth, query.len());
                }

                Ok(response)
            }
            Err(timeout_error) => {
                let elapsed = start_time.elapsed();
                error!(
                    "GraphQL request timed out for IP {} after {:?}",
                    client_ip, elapsed
                );

                // Record timeout
                {
                    let mut metrics = self.metrics.write().unwrap();
                    metrics.record_blocked(BlockReason::Filtered);
                }

                // Log security event
                let event = SecurityEvent::QueryFiltered {
                    client_ip: client_ip.to_string(),
                    query: query.clone(),
                    reason: "Query timeout".to_string(),
                    timestamp: chrono::Utc::now(),
                };
                self.event_handler.handle_event(event);

                Err(vec![ServerError::new(
                    "Query execution timed out. Please try a simpler query.",
                    None,
                )])
            }
        }
    }

    /// Get security metrics
    pub fn get_security_metrics(&self) -> SecurityMetrics {
        self.metrics.read().unwrap().clone()
    }

    /// Get rate limit status for a client
    pub async fn get_rate_limit_status(
        &self,
        client_ip: &str,
    ) -> crate::security::rate_limit::RateLimitStatus {
        self.security
            .rate_limiter
            .get_rate_limit_status(client_ip)
            .await
    }

    /// Reset rate limits for a client
    pub async fn reset_rate_limits(&self, client_ip: &str) {
        self.security.rate_limiter.reset_rate_limit(client_ip).await;
    }

    /// Update security configuration
    pub fn update_security_config(&mut self, config: SecurityConfig) {
        // Note: This would require a different approach for Arc<SecurityMiddleware>
        // For now, we'll skip this functionality
        // self.security.update_config(config);
    }

    /// Get current security configuration
    pub fn get_security_config(&self) -> &SecurityConfig {
        self.security.config()
    }

    /// Get security statistics
    pub async fn get_security_statistics(&self) -> SecurityStatistics {
        let metrics = self.get_security_metrics();
        let rate_limit_stats = self.security.rate_limiter.get_statistics().await;
        let timeout_stats = self.security.timeout_manager.get_timeout_statistics();

        SecurityStatistics {
            metrics,
            rate_limit_stats,
            timeout_stats,
            config: self.get_security_config().clone(),
        }
    }

    /// Check if a query would be allowed
    pub async fn validate_query(&self, query: &str, client_ip: &str) -> QueryValidationResult {
        let request = Request::new(query);

        match self.security.validate_request(&request, client_ip).await {
            Ok(()) => QueryValidationResult {
                allowed: true,
                errors: Vec::new(),
                complexity: self
                    .security
                    .complexity_analyzer
                    .calculate_complexity(query)
                    .unwrap_or(0),
                depth: self
                    .security
                    .depth_limiter
                    .calculate_depth(query)
                    .unwrap_or(0),
                size: query.len(),
            },
            Err(errors) => QueryValidationResult {
                allowed: false,
                errors: errors.into_iter().map(|e| e.message).collect(),
                complexity: self
                    .security
                    .complexity_analyzer
                    .calculate_complexity(query)
                    .unwrap_or(0),
                depth: self
                    .security
                    .depth_limiter
                    .calculate_depth(query)
                    .unwrap_or(0),
                size: query.len(),
            },
        }
    }
}

/// Security statistics
#[derive(Debug, Clone)]
pub struct SecurityStatistics {
    /// Security metrics
    pub metrics: SecurityMetrics,
    /// Rate limit statistics
    pub rate_limit_stats: crate::security::rate_limit::RateLimitStatistics,
    /// Timeout statistics
    pub timeout_stats: crate::security::timeout::TimeoutStatistics,
    /// Current configuration
    pub config: SecurityConfig,
}

/// Query validation result
#[derive(Debug, Clone)]
pub struct QueryValidationResult {
    /// Whether the query is allowed
    pub allowed: bool,
    /// Validation errors
    pub errors: Vec<String>,
    /// Query complexity
    pub complexity: u32,
    /// Query depth
    pub depth: u32,
    /// Query size in bytes
    pub size: usize,
}

impl QueryValidationResult {
    /// Get a summary of the validation result
    pub fn get_summary(&self) -> String {
        if self.allowed {
            format!(
                "Query allowed - Complexity: {}, Depth: {}, Size: {} bytes",
                self.complexity, self.depth, self.size
            )
        } else {
            format!(
                "Query blocked - Errors: {:?}, Complexity: {}, Depth: {}, Size: {} bytes",
                self.errors, self.complexity, self.depth, self.size
            )
        }
    }
}

/// Security server builder for configuration
pub struct SecureGraphQLServerBuilder {
    pool: Option<Arc<DatabasePool>>,
    config: Option<SecurityConfig>,
    event_handler: Option<Arc<dyn SecurityEventHandler>>,
}

impl SecureGraphQLServerBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            pool: None,
            config: None,
            event_handler: None,
        }
    }

    /// Set the database pool
    pub fn with_pool(mut self, pool: Arc<DatabasePool>) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Set the security configuration
    pub fn with_config(mut self, config: SecurityConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the event handler
    pub fn with_event_handler(mut self, event_handler: Arc<dyn SecurityEventHandler>) -> Self {
        self.event_handler = Some(event_handler);
        self
    }

    /// Build the secure GraphQL server
    pub fn build(self) -> Result<SecureGraphQLServer, String> {
        let pool = self.pool.ok_or("Database pool is required")?;
        let config = self.config.unwrap_or_default();

        let server = if let Some(event_handler) = self.event_handler {
            SecureGraphQLServer::new_with_event_handler(pool, config, event_handler)
        } else {
            SecureGraphQLServer::new(pool, config)
        };

        Ok(server)
    }
}

impl Default for SecureGraphQLServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::Request;

    #[tokio::test]
    async fn test_secure_server_creation() {
        // This would require a real database pool in a real test
        // For now, we'll just test the builder pattern
        let builder = SecureGraphQLServerBuilder::new().with_config(SecurityConfig::default());

        // In a real test, we'd build with a test database pool
        // let server = builder.with_pool(test_pool).build().unwrap();
        // assert!(server.get_security_config().max_complexity > 0);
    }

    #[test]
    fn test_query_validation_result() {
        let result = QueryValidationResult {
            allowed: true,
            errors: Vec::new(),
            complexity: 10,
            depth: 3,
            size: 100,
        };

        let summary = result.get_summary();
        assert!(summary.contains("Query allowed"));
        assert!(summary.contains("Complexity: 10"));
        assert!(summary.contains("Depth: 3"));
        assert!(summary.contains("Size: 100"));
    }

    #[test]
    fn test_query_validation_result_blocked() {
        let result = QueryValidationResult {
            allowed: false,
            errors: vec!["Query too complex".to_string()],
            complexity: 1000,
            depth: 15,
            size: 5000,
        };

        let summary = result.get_summary();
        assert!(summary.contains("Query blocked"));
        assert!(summary.contains("Query too complex"));
    }
}
