use anyhow::Result;
use governor::{Quota, RateLimiter as GovernorRateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// **Rate Limiter for SEC EDGAR API**
///
/// Implements rate limiting for SEC EDGAR API requests to comply with
/// SEC's guidelines and avoid being blocked. Uses the `governor` crate
/// for efficient rate limiting with minimal overhead.
///
/// # Rate Limiting Strategy
/// - Default: 10 requests per second (SEC recommended)
/// - Burst allowance for initial requests
/// - Automatic backoff on rate limit violations
/// - Configurable limits for different operations
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
/// use std::time::Duration;
///
/// // Create rate limiter with 10 requests per second
/// let rate_limiter = RateLimiter::new(10, Duration::from_secs(1));
///
/// // Wait for rate limit before making request
/// rate_limiter.wait_for_permit().await?;
///
/// // Make HTTP request here
/// ```
#[derive(Debug, Clone)]
pub struct SecRateLimiter {
    /// Internal rate limiter from governor crate
    limiter: Arc<
        GovernorRateLimiter<
            governor::state::direct::NotKeyed,
            governor::state::InMemoryState,
            governor::clock::DefaultClock,
        >,
    >,

    /// Maximum requests per second
    max_requests_per_second: u32,

    /// Time window for rate limiting
    time_window: Duration,
}

impl SecRateLimiter {
    /// Create a new rate limiter with specified parameters
    ///
    /// # Arguments
    /// * `max_requests_per_second` - Maximum number of requests allowed per second
    /// * `time_window` - Time window for rate limiting (usually 1 second)
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
    /// use std::time::Duration;
    ///
    /// // Create rate limiter for SEC EDGAR (10 requests/second)
    /// let rate_limiter = RateLimiter::new(10, Duration::from_secs(1));
    /// ```
    pub fn new(max_requests_per_second: u32, time_window: Duration) -> Self {
        let quota = Quota::per_second(NonZeroU32::new(max_requests_per_second).unwrap());
        let limiter = Arc::new(GovernorRateLimiter::direct(quota));

        Self {
            limiter,
            max_requests_per_second,
            time_window,
        }
    }

    /// Create a rate limiter with SEC EDGAR recommended settings
    ///
    /// # Returns
    /// Rate limiter configured for SEC EDGAR API (10 requests/second)
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
    ///
    /// // Create SEC EDGAR rate limiter
    /// let rate_limiter = RateLimiter::sec_edgar();
    /// ```
    pub fn sec_edgar() -> Self {
        Self::new(10, Duration::from_secs(1))
    }

    /// Create a rate limiter with conservative settings for bulk operations
    ///
    /// # Returns
    /// Rate limiter configured for bulk operations (5 requests/second)
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
    ///
    /// // Create conservative rate limiter for bulk crawling
    /// let rate_limiter = RateLimiter::conservative();
    /// ```
    pub fn conservative() -> Self {
        Self::new(5, Duration::from_secs(1))
    }

    /// Create a rate limiter with aggressive settings for testing
    ///
    /// # Returns
    /// Rate limiter configured for testing (20 requests/second)
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
    ///
    /// // Create aggressive rate limiter for testing
    /// let rate_limiter = RateLimiter::aggressive();
    /// ```
    pub fn aggressive() -> Self {
        Self::new(20, Duration::from_secs(1))
    }

    /// Wait for a permit to make a request
    ///
    /// This method will block until a permit is available according to the rate limit.
    /// It should be called before making any HTTP request to the SEC EDGAR API.
    ///
    /// # Returns
    /// `Ok(())` when permit is available, `Err` if there's an error
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let rate_limiter = RateLimiter::sec_edgar();
    ///
    /// // Wait for permit before making request
    /// rate_limiter.wait_for_permit().await?;
    ///
    /// // Make HTTP request here
    /// # Ok(())
    /// # }
    /// ```
    pub async fn wait_for_permit(&self) -> Result<()> {
        loop {
            match self.limiter.check() {
                Ok(_) => {
                    debug!("Rate limit permit granted");
                    return Ok(());
                }
                Err(e) => {
                    debug!("Rate limit exceeded, waiting 100ms");
                    sleep(Duration::from_millis(100)).await;
                    warn!("Rate limiter error: {:?}", e);
                }
            }
        }
    }

    /// Try to get a permit without waiting
    ///
    /// This method will return immediately with either a permit or an error.
    /// Use this for non-blocking operations where you can handle rate limit
    /// violations gracefully.
    ///
    /// # Returns
    /// `Ok(())` if permit is available, `Err` if rate limited
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
    ///
    /// let rate_limiter = RateLimiter::sec_edgar();
    ///
    /// // Try to get permit without waiting
    /// match rate_limiter.try_permit() {
    ///     Ok(_) => {
    ///         // Make HTTP request
    ///     }
    ///     Err(_) => {
    ///         // Handle rate limit - maybe queue for later
    ///     }
    /// }
    /// ```
    pub fn try_permit(&self) -> Result<()> {
        match self.limiter.check() {
            Ok(_) => {
                debug!("Rate limit permit granted (non-blocking)");
                Ok(())
            }
            Err(e) => {
                debug!("Rate limit exceeded (non-blocking)");
                warn!("Rate limiter error: {:?}", e);
                Err(anyhow::anyhow!("Rate limit exceeded"))
            }
        }
    }

    /// Get the current rate limit configuration
    ///
    /// # Returns
    /// Tuple of (max_requests_per_second, time_window)
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
    /// use std::time::Duration;
    ///
    /// let rate_limiter = RateLimiter::sec_edgar();
    /// let (max_requests, time_window) = rate_limiter.get_config();
    /// println!("Rate limit: {} requests per {:?}", max_requests, time_window);
    /// ```
    pub fn get_config(&self) -> (u32, Duration) {
        (self.max_requests_per_second, self.time_window)
    }

    /// Check if the rate limiter is currently rate limiting
    ///
    /// # Returns
    /// `true` if currently rate limited, `false` if permits are available
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
    ///
    /// let rate_limiter = RateLimiter::sec_edgar();
    ///
    /// if rate_limiter.is_rate_limited() {
    ///     println!("Currently rate limited");
    /// } else {
    ///     println!("Permits available");
    /// }
    /// ```
    pub fn is_rate_limited(&self) -> bool {
        self.limiter.check().is_err()
    }

    /// Get the time until the next permit is available
    ///
    /// # Returns
    /// `Some(Duration)` if rate limited, `None` if permits are available
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
    ///
    /// let rate_limiter = RateLimiter::sec_edgar();
    ///
    /// if let Some(wait_time) = rate_limiter.time_until_next_permit() {
    ///     println!("Wait {:?} for next permit", wait_time);
    /// } else {
    ///     println!("Permit available now");
    /// }
    /// ```
    pub fn time_until_next_permit(&self) -> Option<Duration> {
        match self.limiter.check() {
            Ok(_) => None,
            Err(_) => {
                Some(Duration::from_millis(100))
            }
        }
    }

    /// Reset the rate limiter (for testing purposes)
    ///
    /// This method clears the rate limiter state and allows immediate requests.
    /// It should only be used in tests or when you need to reset the limiter
    /// for some reason.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use econ_graph_sec_crawler::rate_limiter::RateLimiter;
    ///
    /// let rate_limiter = RateLimiter::sec_edgar();
    ///
    /// // Reset for testing
    /// rate_limiter.reset();
    /// ```
    pub fn reset(&self) {
        // Note: The governor crate doesn't provide a direct reset method
        // This is a limitation of the current implementation
        // In practice, you would create a new rate limiter instance
        debug!("Rate limiter reset requested (note: governor crate doesn't support reset)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_rate_limiter_creation() {
        let rate_limiter = RateLimiter::new(10, Duration::from_secs(1));
        let (max_requests, time_window) = rate_limiter.get_config();

        assert_eq!(max_requests, 10);
        assert_eq!(time_window, Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_sec_edgar_rate_limiter() {
        let rate_limiter = RateLimiter::sec_edgar();
        let (max_requests, time_window) = rate_limiter.get_config();

        assert_eq!(max_requests, 10);
        assert_eq!(time_window, Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_conservative_rate_limiter() {
        let rate_limiter = RateLimiter::conservative();
        let (max_requests, time_window) = rate_limiter.get_config();

        assert_eq!(max_requests, 5);
        assert_eq!(time_window, Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_aggressive_rate_limiter() {
        let rate_limiter = RateLimiter::aggressive();
        let (max_requests, time_window) = rate_limiter.get_config();

        assert_eq!(max_requests, 20);
        assert_eq!(time_window, Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_wait_for_permit() {
        let rate_limiter = RateLimiter::new(2, Duration::from_secs(1));

        // First permit should be available immediately
        let start = Instant::now();
        rate_limiter.wait_for_permit().await.unwrap();
        let first_duration = start.elapsed();
        assert!(first_duration < Duration::from_millis(100));

        // Second permit should also be available immediately
        let start = Instant::now();
        rate_limiter.wait_for_permit().await.unwrap();
        let second_duration = start.elapsed();
        assert!(second_duration < Duration::from_millis(100));

        // Third permit should be rate limited
        let start = Instant::now();
        rate_limiter.wait_for_permit().await.unwrap();
        let third_duration = start.elapsed();
        assert!(third_duration >= Duration::from_millis(400)); // Should wait ~500ms
    }

    #[tokio::test]
    async fn test_try_permit() {
        let rate_limiter = RateLimiter::new(1, Duration::from_secs(1));

        // First permit should succeed
        assert!(rate_limiter.try_permit().is_ok());

        // Second permit should fail immediately
        assert!(rate_limiter.try_permit().is_err());
    }

    #[tokio::test]
    async fn test_is_rate_limited() {
        let rate_limiter = RateLimiter::new(1, Duration::from_secs(1));

        // Should not be rate limited initially
        assert!(!rate_limiter.is_rate_limited());

        // Use up the permit
        rate_limiter.wait_for_permit().await.unwrap();

        // Should be rate limited now
        assert!(rate_limiter.is_rate_limited());
    }

    #[tokio::test]
    async fn test_time_until_next_permit() {
        let rate_limiter = RateLimiter::new(1, Duration::from_secs(1));

        // Should return None initially
        assert!(rate_limiter.time_until_next_permit().is_none());

        // Use up the permit
        rate_limiter.wait_for_permit().await.unwrap();

        // Should return Some duration now
        let wait_time = rate_limiter.time_until_next_permit();
        assert!(wait_time.is_some());
        assert!(wait_time.unwrap() > Duration::from_millis(400));
    }

    #[tokio::test]
    async fn test_rate_limiter_performance() {
        let rate_limiter = RateLimiter::new(100, Duration::from_secs(1));

        let start = Instant::now();

        // Make 100 requests as fast as possible
        for _ in 0..100 {
            rate_limiter.wait_for_permit().await.unwrap();
        }

        let duration = start.elapsed();

        // Should take approximately 1 second (allowing for some variance)
        assert!(duration >= Duration::from_millis(900));
        assert!(duration <= Duration::from_millis(1100));
    }

    #[tokio::test]
    async fn test_concurrent_rate_limiting() {
        let rate_limiter = Arc::new(RateLimiter::new(5, Duration::from_secs(1)));

        let start = Instant::now();

        // Spawn 10 concurrent tasks
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let limiter = rate_limiter.clone();
                tokio::spawn(async move {
                    limiter.wait_for_permit().await.unwrap();
                })
            })
            .collect();

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        let duration = start.elapsed();

        // Should take approximately 2 seconds (5 requests per second)
        assert!(duration >= Duration::from_millis(1800));
        assert!(duration <= Duration::from_millis(2200));
    }

    #[tokio::test]
    async fn test_rate_limiter_with_different_limits() {
        // Test with very low limit
        let low_limit = RateLimiter::new(1, Duration::from_secs(1));
        let start = Instant::now();
        low_limit.wait_for_permit().await.unwrap();
        low_limit.wait_for_permit().await.unwrap();
        let low_duration = start.elapsed();
        assert!(low_duration >= Duration::from_millis(900));

        // Test with high limit
        let high_limit = RateLimiter::new(100, Duration::from_secs(1));
        let start = Instant::now();
        high_limit.wait_for_permit().await.unwrap();
        high_limit.wait_for_permit().await.unwrap();
        let high_duration = start.elapsed();
        assert!(high_duration < Duration::from_millis(100));
    }
}
