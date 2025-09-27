//! # GraphQL Security Module
//!
//! This module provides comprehensive security features for the GraphQL API,
//! including query complexity analysis, depth limiting, rate limiting, and
//! abuse prevention mechanisms.
//!
//! # Security Features
//!
//! 1. **Query Complexity Analysis**: Prevents expensive queries that could DoS the system
//! 2. **Query Depth Limiting**: Prevents deeply nested queries that could cause stack overflow
//! 3. **Rate Limiting**: Prevents abuse through excessive request frequency
//! 4. **Query Size Limiting**: Prevents extremely large queries
//! 5. **Introspection Protection**: Controls access to schema introspection
//! 6. **Query Timeout**: Prevents long-running queries from blocking the system
//! 7. **Query Whitelisting/Blacklisting**: Allows/denies specific query patterns
//!
//! # Design Principles
//!
//! 1. **Security First**: All security measures are enabled by default
//! 2. **Configurable**: Security settings can be adjusted per environment
//! 3. **Performance**: Security checks are optimized for minimal overhead
//! 4. **Monitoring**: All security events are logged and monitored
//! 5. **Fail Safe**: Security failures default to denying access
//!
//! # Quality Standards
//!
//! - All security measures must be thoroughly tested
//! - Security configurations must be documented
//! - Performance impact must be measured and optimized
//! - Security events must be properly logged and monitored

pub mod complexity;
pub mod depth_limit;
pub mod input_validation;
pub mod introspection;
pub mod monitoring;
pub mod query_analysis;
pub mod rate_limit;
pub mod server;
pub mod timeout;
pub mod whitelist;

use async_graphql::{Request, Response, ServerError};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Security configuration for GraphQL API
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Maximum query complexity score
    pub max_complexity: u32,
    /// Maximum query depth
    pub max_depth: u32,
    /// Maximum query size in bytes
    pub max_query_size: usize,
    /// Query timeout in seconds
    pub query_timeout: u64,
    /// Enable introspection protection
    pub protect_introspection: bool,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Query whitelist/blacklist configuration
    pub query_filter: QueryFilterConfig,
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per minute per IP
    pub requests_per_minute: u32,
    /// Maximum requests per hour per IP
    pub requests_per_hour: u32,
    /// Maximum requests per day per IP
    pub requests_per_day: u32,
    /// Enable rate limiting
    pub enabled: bool,
}

/// Query filter configuration
#[derive(Debug, Clone)]
pub struct QueryFilterConfig {
    /// Enable query whitelisting
    pub enable_whitelist: bool,
    /// Enable query blacklisting
    pub enable_blacklist: bool,
    /// Whitelisted query patterns
    pub whitelist_patterns: Vec<String>,
    /// Blacklisted query patterns
    pub blacklist_patterns: Vec<String>,
    /// Case sensitive matching
    pub case_sensitive: bool,
    /// Use regex patterns
    pub use_regex: bool,
    /// Allow partial matches
    pub allow_partial_matches: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            max_complexity: 1000,
            max_depth: 10,
            max_query_size: 10000, // 10KB
            query_timeout: 30,     // 30 seconds
            protect_introspection: true,
            rate_limit: RateLimitConfig {
                requests_per_minute: 60,
                requests_per_hour: 1000,
                requests_per_day: 10000,
                enabled: true,
            },
            query_filter: QueryFilterConfig {
                enable_whitelist: false,
                enable_blacklist: true,
                whitelist_patterns: Vec::new(),
                blacklist_patterns: vec![
                    "__schema".to_string(),
                    "__type".to_string(),
                    "__typename".to_string(),
                ],
                case_sensitive: false,
                use_regex: false,
                allow_partial_matches: true,
            },
        }
    }
}

/// GraphQL security middleware
pub struct SecurityMiddleware {
    config: SecurityConfig,
    complexity_analyzer: complexity::ComplexityAnalyzer,
    depth_limiter: depth_limit::DepthLimiter,
    rate_limiter: rate_limit::RateLimiter,
    query_analyzer: query_analysis::QueryAnalyzer,
    introspection_protector: introspection::IntrospectionProtector,
    timeout_manager: timeout::TimeoutManager,
    query_filter: whitelist::QueryFilter,
}

impl SecurityMiddleware {
    /// Create a new security middleware with the given configuration
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            complexity_analyzer: complexity::ComplexityAnalyzer::new(config.max_complexity),
            depth_limiter: depth_limit::DepthLimiter::new(config.max_depth),
            rate_limiter: rate_limit::RateLimiter::new(rate_limit::RateLimitConfig {
                requests_per_minute: config.rate_limit.requests_per_minute,
                requests_per_hour: config.rate_limit.requests_per_hour,
                requests_per_day: config.rate_limit.requests_per_day,
                enabled: config.rate_limit.enabled,
            }),
            query_analyzer: query_analysis::QueryAnalyzer::new(config.max_query_size),
            introspection_protector: introspection::IntrospectionProtector::new(
                config.protect_introspection,
            ),
            timeout_manager: timeout::TimeoutManager::new(config.query_timeout),
            query_filter: whitelist::QueryFilter::new(whitelist::QueryFilterConfig {
                enable_whitelist: config.query_filter.enable_whitelist,
                enable_blacklist: config.query_filter.enable_blacklist,
                whitelist_patterns: config.query_filter.whitelist_patterns.clone(),
                blacklist_patterns: config.query_filter.blacklist_patterns.clone(),
                case_sensitive: config.query_filter.case_sensitive,
                use_regex: config.query_filter.use_regex,
                allow_partial_matches: config.query_filter.allow_partial_matches,
            }),
            config,
        }
    }

    /// Validate a GraphQL request against all security measures
    pub async fn validate_request(
        &self,
        request: &Request,
        client_ip: &str,
    ) -> Result<(), Vec<ServerError>> {
        let mut errors = Vec::new();

        // 1. Rate limiting check
        if self.config.rate_limit.enabled {
            if let Err(e) = self.rate_limiter.check_rate_limit(client_ip).await {
                error!("Rate limit exceeded for IP {}: {}", client_ip, e);
                errors.push(ServerError::new(
                    "Rate limit exceeded. Please try again later.",
                    None,
                ));
            }
        }

        // 2. Query size check
        if let Err(e) = self.query_analyzer.validate_query_size(&request.query) {
            warn!("Query size exceeded: {}", e);
            errors.push(ServerError::new(
                "Query too large. Please reduce the query size.",
                None,
            ));
        }

        // 3. Query depth check
        if let Err(e) = self.depth_limiter.validate_depth(&request.query) {
            warn!("Query depth exceeded: {}", e);
            errors.push(ServerError::new(
                "Query too deep. Please reduce the nesting level.",
                None,
            ));
        }

        // 4. Query complexity check
        if let Err(e) = self.complexity_analyzer.validate_complexity(&request.query) {
            warn!("Query complexity exceeded: {}", e);
            errors.push(ServerError::new(
                "Query too complex. Please simplify the query.",
                None,
            ));
        }

        // 5. Introspection protection
        if let Err(e) = self
            .introspection_protector
            .validate_introspection(&request.query)
        {
            warn!("Introspection query blocked: {}", e);
            errors.push(ServerError::new(
                "Introspection queries are not allowed.",
                None,
            ));
        }

        // 6. Query filtering (whitelist/blacklist)
        if let Err(e) = self.query_filter.validate_query(&request.query) {
            warn!("Query filtered: {}", e);
            errors.push(ServerError::new(
                "Query not allowed by security policy.",
                None,
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Get the current security configuration
    pub fn config(&self) -> &SecurityConfig {
        &self.config
    }

    /// Update the security configuration
    pub fn update_config(&mut self, config: SecurityConfig) {
        self.config = config.clone();
        self.complexity_analyzer = complexity::ComplexityAnalyzer::new(config.max_complexity);
        self.depth_limiter = depth_limit::DepthLimiter::new(config.max_depth);
        self.rate_limiter = rate_limit::RateLimiter::new(rate_limit::RateLimitConfig {
            requests_per_minute: config.rate_limit.requests_per_minute,
            requests_per_hour: config.rate_limit.requests_per_hour,
            requests_per_day: config.rate_limit.requests_per_day,
            enabled: config.rate_limit.enabled,
        });
        self.query_analyzer = query_analysis::QueryAnalyzer::new(config.max_query_size);
        self.introspection_protector =
            introspection::IntrospectionProtector::new(config.protect_introspection);
        self.timeout_manager = timeout::TimeoutManager::new(config.query_timeout);
        self.query_filter = whitelist::QueryFilter::new(whitelist::QueryFilterConfig {
            enable_whitelist: config.query_filter.enable_whitelist,
            enable_blacklist: config.query_filter.enable_blacklist,
            whitelist_patterns: config.query_filter.whitelist_patterns.clone(),
            blacklist_patterns: config.query_filter.blacklist_patterns.clone(),
            case_sensitive: config.query_filter.case_sensitive,
            use_regex: config.query_filter.use_regex,
            allow_partial_matches: config.query_filter.allow_partial_matches,
        });
    }
}

/// Security event types for monitoring and alerting
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SecurityEvent {
    /// Rate limit exceeded
    RateLimitExceeded {
        client_ip: String,
        requests_per_minute: u32,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Query complexity exceeded
    ComplexityExceeded {
        client_ip: String,
        complexity: u32,
        max_complexity: u32,
        query: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Query depth exceeded
    DepthExceeded {
        client_ip: String,
        depth: u32,
        max_depth: u32,
        query: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Query size exceeded
    QuerySizeExceeded {
        client_ip: String,
        size: usize,
        max_size: usize,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Introspection query blocked
    IntrospectionBlocked {
        client_ip: String,
        query: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Query filtered by security policy
    QueryFiltered {
        client_ip: String,
        query: String,
        reason: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// Security event handler for monitoring and alerting
pub trait SecurityEventHandler: Send + Sync {
    /// Handle a security event
    fn handle_event(&self, event: SecurityEvent);
}

/// Default security event handler that logs events
pub struct LoggingSecurityEventHandler;

impl SecurityEventHandler for LoggingSecurityEventHandler {
    fn handle_event(&self, event: SecurityEvent) {
        match event {
            SecurityEvent::RateLimitExceeded {
                client_ip,
                requests_per_minute,
                timestamp,
            } => {
                warn!(
                    "Rate limit exceeded for IP {}: {} requests per minute at {}",
                    client_ip, requests_per_minute, timestamp
                );
            }
            SecurityEvent::ComplexityExceeded {
                client_ip,
                complexity,
                max_complexity,
                query,
                timestamp,
            } => {
                warn!(
                    "Query complexity exceeded for IP {}: {} > {} at {}",
                    client_ip, complexity, max_complexity, timestamp
                );
                info!("Blocked query: {}", query);
            }
            SecurityEvent::DepthExceeded {
                client_ip,
                depth,
                max_depth,
                query,
                timestamp,
            } => {
                warn!(
                    "Query depth exceeded for IP {}: {} > {} at {}",
                    client_ip, depth, max_depth, timestamp
                );
                info!("Blocked query: {}", query);
            }
            SecurityEvent::QuerySizeExceeded {
                client_ip,
                size,
                max_size,
                timestamp,
            } => {
                warn!(
                    "Query size exceeded for IP {}: {} > {} at {}",
                    client_ip, size, max_size, timestamp
                );
            }
            SecurityEvent::IntrospectionBlocked {
                client_ip,
                query,
                timestamp,
            } => {
                warn!(
                    "Introspection query blocked for IP {} at {}",
                    client_ip, timestamp
                );
                info!("Blocked introspection query: {}", query);
            }
            SecurityEvent::QueryFiltered {
                client_ip,
                query,
                reason,
                timestamp,
            } => {
                warn!(
                    "Query filtered for IP {}: {} at {}",
                    client_ip, reason, timestamp
                );
                info!("Filtered query: {}", query);
            }
        }
    }
}

/// Security metrics for monitoring
#[derive(Debug, Default, Clone)]
pub struct SecurityMetrics {
    /// Total requests processed
    pub total_requests: u64,
    /// Requests blocked by rate limiting
    pub rate_limited_requests: u64,
    /// Requests blocked by complexity
    pub complexity_blocked_requests: u64,
    /// Requests blocked by depth
    pub depth_blocked_requests: u64,
    /// Requests blocked by size
    pub size_blocked_requests: u64,
    /// Requests blocked by introspection protection
    pub introspection_blocked_requests: u64,
    /// Requests blocked by query filtering
    pub filtered_requests: u64,
    /// Average query complexity
    pub average_complexity: f64,
    /// Average query depth
    pub average_depth: f64,
    /// Average query size
    pub average_size: f64,
}

impl SecurityMetrics {
    /// Update metrics with a new request
    pub fn update_request(&mut self, complexity: u32, depth: u32, size: usize) {
        self.total_requests += 1;

        // Update running averages
        let total = self.total_requests as f64;
        self.average_complexity =
            (self.average_complexity * (total - 1.0) + complexity as f64) / total;
        self.average_depth = (self.average_depth * (total - 1.0) + depth as f64) / total;
        self.average_size = (self.average_size * (total - 1.0) + size as f64) / total;
    }

    /// Record a blocked request
    pub fn record_blocked(&mut self, reason: BlockReason) {
        match reason {
            BlockReason::RateLimit => self.rate_limited_requests += 1,
            BlockReason::Complexity => self.complexity_blocked_requests += 1,
            BlockReason::Depth => self.depth_blocked_requests += 1,
            BlockReason::Size => self.size_blocked_requests += 1,
            BlockReason::Introspection => self.introspection_blocked_requests += 1,
            BlockReason::Filtered => self.filtered_requests += 1,
        }
    }
}

/// Reasons for blocking a request
#[derive(Debug, Clone)]
pub enum BlockReason {
    RateLimit,
    Complexity,
    Depth,
    Size,
    Introspection,
    Filtered,
}
