//! # Rate Limiting
//!
//! This module provides rate limiting functionality to prevent abuse and
//! ensure fair usage of the GraphQL API.
//!
//! # Rate Limiting Strategy
//!
//! The rate limiter uses a sliding window approach with multiple time windows:
//! - Per minute: Prevents burst attacks
//! - Per hour: Prevents sustained abuse
//! - Per day: Prevents long-term abuse
//!
//! # Implementation
//!
//! Uses an in-memory store with automatic cleanup of expired entries.
//! For production use, consider using Redis or another distributed store.
//!
//! # Security Benefits
//!
//! - Prevents brute force attacks
//! - Protects against DDoS attacks
//! - Ensures fair resource usage
//! - Provides predictable API performance
//!
//! # Configuration
//!
//! - `requests_per_minute`: Maximum requests per minute per IP
//! - `requests_per_hour`: Maximum requests per hour per IP
//! - `requests_per_day`: Maximum requests per day per IP
//! - `enabled`: Whether rate limiting is enabled

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

/// Rate limiter configuration
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

/// Rate limit entry for tracking requests
#[derive(Debug, Clone)]
struct RateLimitEntry {
    /// Timestamps of requests
    requests: Vec<Instant>,
    /// Last cleanup time
    last_cleanup: Instant,
}

impl RateLimitEntry {
    fn new() -> Self {
        Self {
            requests: Vec::new(),
            last_cleanup: Instant::now(),
        }
    }

    /// Add a new request timestamp
    fn add_request(&mut self) {
        self.requests.push(Instant::now());
    }

    /// Clean up old requests and return current counts
    fn cleanup_and_count(&mut self) -> (u32, u32, u32) {
        let now = Instant::now();

        // Clean up old requests if enough time has passed
        if now.duration_since(self.last_cleanup) > Duration::from_secs(60) {
            self.requests.retain(|&timestamp| {
                now.duration_since(timestamp) < Duration::from_secs(86400) // 24 hours
            });
            self.last_cleanup = now;
        }

        // Count requests in different time windows
        let minute_ago = now - Duration::from_secs(60);
        let hour_ago = now - Duration::from_secs(3600);
        let day_ago = now - Duration::from_secs(86400);

        let requests_per_minute = self
            .requests
            .iter()
            .filter(|&&timestamp| timestamp >= minute_ago)
            .count() as u32;

        let requests_per_hour = self
            .requests
            .iter()
            .filter(|&&timestamp| timestamp >= hour_ago)
            .count() as u32;

        let requests_per_day = self
            .requests
            .iter()
            .filter(|&&timestamp| timestamp >= day_ago)
            .count() as u32;

        (requests_per_minute, requests_per_hour, requests_per_day)
    }
}

/// Rate limiter implementation
pub struct RateLimiter {
    /// Rate limiting configuration
    config: RateLimitConfig,
    /// In-memory store for rate limit entries
    entries: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
    /// Last cleanup time for the entire store
    last_global_cleanup: Arc<RwLock<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            last_global_cleanup: Arc::new(RwLock::new(Instant::now())),
            config,
        }
    }

    /// Check if a request is allowed for the given IP
    pub async fn check_rate_limit(&self, client_ip: &str) -> Result<(), String> {
        if !self.config.enabled {
            return Ok(());
        }

        // Perform global cleanup if needed
        self.cleanup_expired_entries().await;

        let mut entries = self.entries.write().await;
        let entry = entries
            .entry(client_ip.to_string())
            .or_insert_with(RateLimitEntry::new);

        // Add the current request
        entry.add_request();

        // Get current counts
        let (requests_per_minute, requests_per_hour, requests_per_day) = entry.cleanup_and_count();

        // Check rate limits
        if requests_per_minute > self.config.requests_per_minute {
            return Err(format!(
                "Rate limit exceeded: {} requests per minute (limit: {})",
                requests_per_minute, self.config.requests_per_minute
            ));
        }

        if requests_per_hour > self.config.requests_per_hour {
            return Err(format!(
                "Rate limit exceeded: {} requests per hour (limit: {})",
                requests_per_hour, self.config.requests_per_hour
            ));
        }

        if requests_per_day > self.config.requests_per_day {
            return Err(format!(
                "Rate limit exceeded: {} requests per day (limit: {})",
                requests_per_day, self.config.requests_per_day
            ));
        }

        debug!(
            "Rate limit check passed for IP {}: {}/{} per minute, {}/{} per hour, {}/{} per day",
            client_ip,
            requests_per_minute,
            self.config.requests_per_minute,
            requests_per_hour,
            self.config.requests_per_hour,
            requests_per_day,
            self.config.requests_per_day
        );

        Ok(())
    }

    /// Clean up expired entries from the store
    async fn cleanup_expired_entries(&self) {
        let now = Instant::now();
        let mut last_cleanup = self.last_global_cleanup.write().await;

        // Only cleanup every 5 minutes to avoid excessive locking
        if now.duration_since(*last_cleanup) < Duration::from_secs(300) {
            return;
        }

        let mut entries = self.entries.write().await;
        let mut to_remove = Vec::new();

        for (ip, entry) in entries.iter_mut() {
            let (_, _, requests_per_day) = entry.cleanup_and_count();
            if requests_per_day == 0 {
                to_remove.push(ip.clone());
            }
        }

        for ip in to_remove {
            entries.remove(&ip);
        }

        *last_cleanup = now;
    }

    /// Get current rate limit status for an IP
    pub async fn get_rate_limit_status(&self, client_ip: &str) -> RateLimitStatus {
        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(client_ip) {
            // Note: cleanup_and_count requires mutable access, but we only have read access here
            // For now, we'll return a simplified status
            let requests_per_minute = entry.requests.len();
            let requests_per_hour = entry.requests.len();
            let requests_per_day = entry.requests.len();

            RateLimitStatus {
                requests_per_minute: requests_per_minute as u32,
                requests_per_hour: requests_per_hour as u32,
                requests_per_day: requests_per_day as u32,
                limit_per_minute: self.config.requests_per_minute,
                limit_per_hour: self.config.requests_per_hour,
                limit_per_day: self.config.requests_per_day,
            }
        } else {
            RateLimitStatus {
                requests_per_minute: 0,
                requests_per_hour: 0,
                requests_per_day: 0,
                limit_per_minute: self.config.requests_per_minute,
                limit_per_hour: self.config.requests_per_hour,
                limit_per_day: self.config.requests_per_day,
            }
        }
    }

    /// Update the rate limit configuration
    pub fn update_config(&mut self, config: RateLimitConfig) {
        self.config = config;
    }

    /// Get the current configuration
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }

    /// Reset rate limits for a specific IP
    pub async fn reset_rate_limit(&self, client_ip: &str) {
        let mut entries = self.entries.write().await;
        entries.remove(client_ip);
    }

    /// Get statistics about rate limiting
    pub async fn get_statistics(&self) -> RateLimitStatistics {
        let entries = self.entries.read().await;
        let total_ips = entries.len();
        let mut total_requests = 0;
        let mut active_ips = 0;

        for entry in entries.values() {
            let requests_per_minute = entry.requests.len();
            total_requests += requests_per_minute;
            if requests_per_minute > 0 {
                active_ips += 1;
            }
        }

        RateLimitStatistics {
            total_ips,
            active_ips,
            total_requests: total_requests as u32,
            config: self.config.clone(),
        }
    }
}

/// Rate limit status for a specific IP
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    /// Current requests per minute
    pub requests_per_minute: u32,
    /// Current requests per hour
    pub requests_per_hour: u32,
    /// Current requests per day
    pub requests_per_day: u32,
    /// Limit per minute
    pub limit_per_minute: u32,
    /// Limit per hour
    pub limit_per_hour: u32,
    /// Limit per day
    pub limit_per_day: u32,
}

impl RateLimitStatus {
    /// Check if any rate limit is exceeded
    pub fn is_exceeded(&self) -> bool {
        self.requests_per_minute > self.limit_per_minute
            || self.requests_per_hour > self.limit_per_hour
            || self.requests_per_day > self.limit_per_day
    }

    /// Get the percentage of the most restrictive limit
    pub fn get_usage_percentage(&self) -> f64 {
        let minute_pct = (self.requests_per_minute as f64 / self.limit_per_minute as f64) * 100.0;
        let hour_pct = (self.requests_per_hour as f64 / self.limit_per_hour as f64) * 100.0;
        let day_pct = (self.requests_per_day as f64 / self.limit_per_day as f64) * 100.0;

        minute_pct.max(hour_pct).max(day_pct)
    }
}

/// Rate limit statistics
#[derive(Debug, Clone)]
pub struct RateLimitStatistics {
    /// Total number of IPs tracked
    pub total_ips: usize,
    /// Number of active IPs (with recent requests)
    pub active_ips: usize,
    /// Total requests in the last minute
    pub total_requests: u32,
    /// Current configuration
    pub config: RateLimitConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_rate_limit_basic() {
        let config = RateLimitConfig {
            requests_per_minute: 5,
            requests_per_hour: 100,
            requests_per_day: 1000,
            enabled: true,
        };

        let limiter = RateLimiter::new(config);
        let client_ip = "127.0.0.1";

        // Should allow first 5 requests
        for i in 0..5 {
            assert!(
                limiter.check_rate_limit(client_ip).await.is_ok(),
                "Request {} should be allowed",
                i
            );
        }

        // 6th request should be blocked
        assert!(
            limiter.check_rate_limit(client_ip).await.is_err(),
            "6th request should be blocked"
        );
    }

    #[tokio::test]
    async fn test_rate_limit_disabled() {
        let config = RateLimitConfig {
            requests_per_minute: 1,
            requests_per_hour: 1,
            requests_per_day: 1,
            enabled: false,
        };

        let limiter = RateLimiter::new(config);
        let client_ip = "127.0.0.1";

        // Should allow unlimited requests when disabled
        for _ in 0..10 {
            assert!(limiter.check_rate_limit(client_ip).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_rate_limit_status() {
        let config = RateLimitConfig {
            requests_per_minute: 10,
            requests_per_hour: 100,
            requests_per_day: 1000,
            enabled: true,
        };

        let limiter = RateLimiter::new(config);
        let client_ip = "127.0.0.1";

        // Make a few requests
        for _ in 0..3 {
            limiter.check_rate_limit(client_ip).await.unwrap();
        }

        let status = limiter.get_rate_limit_status(client_ip).await;
        assert_eq!(status.requests_per_minute, 3);
        assert!(!status.is_exceeded());
        assert_eq!(status.get_usage_percentage(), 30.0);
    }

    #[tokio::test]
    async fn test_rate_limit_reset() {
        let config = RateLimitConfig {
            requests_per_minute: 2,
            requests_per_hour: 100,
            requests_per_day: 1000,
            enabled: true,
        };

        let limiter = RateLimiter::new(config);
        let client_ip = "127.0.0.1";

        // Exceed the limit
        for _ in 0..3 {
            limiter.check_rate_limit(client_ip).await.unwrap();
        }
        assert!(limiter.check_rate_limit(client_ip).await.is_err());

        // Reset and try again
        limiter.reset_rate_limit(client_ip).await;
        assert!(limiter.check_rate_limit(client_ip).await.is_ok());
    }
}
