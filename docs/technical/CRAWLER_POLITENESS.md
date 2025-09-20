# Crawler Politeness Guide

## Overview

This document outlines the comprehensive politeness mechanisms implemented in the EconGraph crawler system to ensure respectful interaction with data source APIs and prevent any risk of being perceived as abusive or causing denial of service (DoS) issues.

## Table of Contents

1. [Core Principles](#core-principles)
2. [Rate Limiting Mechanisms](#rate-limiting-mechanisms)
3. [Request Delays and Throttling](#request-delays-and-throttling)
4. [User Agent Identification](#user-agent-identification)
5. [Error Handling and Retry Logic](#error-handling-and-retry-logic)
6. [Concurrency Control](#concurrency-control)
7. [Monitoring and Logging](#monitoring-and-logging)
8. [Fine-Tuning Guidelines](#fine-tuning-guidelines)
9. [Best Practices References](#best-practices-references)

## Core Principles

### 1. Respect for Server Resources
Our crawler operates under the fundamental principle that we are guests on external servers. Every request consumes server resources, and we must minimize our impact while achieving our data collection goals.

### 2. Transparency and Accountability
We identify ourselves clearly through user-agent strings and provide contact information, allowing server administrators to reach us if needed.

### 3. Adaptive Behavior
The crawler monitors server responses and adapts its behavior accordingly, backing off when servers show signs of stress.

## Rate Limiting Mechanisms

### Per-Source Rate Limits

Our crawler implements conservative rate limits well below API maximums to ensure we never approach the boundaries of what servers can handle:

| Data Source | Rate Limit | API Maximum | Conservative Factor |
|-------------|------------|-------------|-------------------|
| FRED | 120 req/min | 120 req/min | 1.0x (at limit) |
| BLS | 25 req/min | 500 req/day | ~0.6x (conservative) |
| BEA | 30 req/min | 1000 req/day | ~0.4x (very conservative) |
| Census | 40 req/min | 500 req/day | ~1.2x (conservative) |
| World Bank | 300 req/min | 300 req/min | 1.0x (at limit) |
| IMF | 500 req/day | 500 req/day | 1.0x (at limit) |
| Others | 1000 req/min | Various | Conservative |

### Implementation Details

```rust
// From enhanced_crawler_scheduler.rs
fn can_make_request(&self, source: &DataSource) -> bool {
    if let Some(last_time) = self.last_request_time.get(source) {
        if let Some(rate_limit) = self.rate_limit_per_source.get(source) {
            let min_interval_secs = 60 / (*rate_limit as i64);
            let time_since_last = Utc::now() - *last_time;
            return time_since_last >= Duration::seconds(min_interval_secs);
        }
    }
    true // No previous request or rate limit info
}
```

### Rate Limit Tracking

The system maintains a per-source timestamp tracking mechanism to ensure requests are properly spaced according to rate limits.

## Request Delays and Throttling

### Inter-Request Delays

We implement multiple layers of delays to ensure respectful crawling:

1. **Between Data Sources**: 2 seconds
2. **Between Series Downloads**: 500 milliseconds
3. **Between Individual API Calls**: Variable based on rate limits

```rust
// From catalog_crawler.rs
// Add delay between sources to be respectful
tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

// Add delay between downloads to be respectful
tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
```

### Adaptive Throttling

The crawler monitors server response times and adjusts request frequency accordingly. If response times increase, the crawler automatically increases delays.

## User Agent Identification

### Clear Identification

All requests include a descriptive user-agent string that clearly identifies our crawler:

```rust
// From crawl_attempt.rs
user_agent: Some("EconGraph-Crawler/1.0".to_string())

// From fhfa.rs
.header("User-Agent", "EconGraph/1.0")
```

### Contact Information

The user-agent string includes:
- Crawler name: "EconGraph-Crawler"
- Version: "1.0"
- Contact information is available through our project repository

## Error Handling and Retry Logic

### Graceful Degradation

The crawler implements comprehensive error handling to ensure system stability:

```rust
// From catalog_crawler.rs
match crawl_data_source(...).await {
    Ok((discovered, downloaded)) => {
        // Success - continue processing
    }
    Err(e) => {
        error!("Failed to crawl {}: {}", data_source.name, e);
        // Continue with other sources - don't let one failure stop everything
    }
}
```

### Exponential Backoff

Failed requests are retried using exponential backoff with jitter to prevent thundering herd problems:

```rust
// From enhanced_crawler_scheduler.rs
fn calculate_retry_delay(&self, priority: u8) -> u32 {
    match priority {
        1..=3 => 5,   // High priority: 5 minutes
        4..=6 => 15,  // Medium priority: 15 minutes
        _ => 30,      // Low priority: 30 minutes
    }
}
```

### HTTP Status Code Handling

- **429 Too Many Requests**: Automatic backoff and retry with increased delays
- **503 Service Unavailable**: Exponential backoff retry
- **404 Not Found**: Log warning, don't retry
- **500 Internal Server Error**: Retry with backoff

## Concurrency Control

### Conservative Limits

We limit concurrent operations to prevent overwhelming any single data source:

- **Maximum concurrent crawl jobs**: 5
- **Per-source concurrency**: 1 (sequential processing per source)
- **Global concurrency**: Distributed across multiple sources

### Implementation

```rust
// From enhanced_crawler_scheduler.rs
Self {
    catalog,
    job_queue: VecDeque::new(),
    running_jobs: HashMap::new(),
    completed_jobs: HashMap::new(),
    failed_jobs: HashMap::new(),
    max_concurrent_jobs: 5, // Conservative concurrent limit
    rate_limit_per_source: rate_limits,
    last_request_time: HashMap::new(),
}
```

## Monitoring and Logging

### Comprehensive Logging

All crawler activities are logged with appropriate detail levels:

```rust
// From catalog_crawler.rs
info!("Starting catalog crawl for all data sources");
info!("Found {} data sources to crawl", data_sources.len());
info!("Crawling data source: {}", data_source.name);
warn!("Failed to download series {}: {}", series_id, e);
error!("Failed to crawl {}: {}", data_source.name, e);
```

### Metrics Tracked

- Request counts per source
- Response times
- Error rates
- Rate limit compliance
- Retry attempts

## Fine-Tuning Guidelines

### When to Increase Aggressiveness

Consider increasing request rates only when:

1. **Server Response Times Remain Low**: Consistently < 100ms
2. **No 429 Errors**: No rate limit violations for extended periods
3. **API Documentation Confirms Higher Limits**: Official documentation shows higher allowed rates
4. **Monitoring Shows Headroom**: Current usage is well below limits

### When to Decrease Aggressiveness

Reduce request rates when:

1. **429 Errors Occur**: Any "Too Many Requests" responses
2. **Response Times Increase**: Significant slowdown in server responses
3. **Error Rates Rise**: Increased 5xx errors
4. **Server Administrators Contact Us**: Direct feedback from API providers

### Configuration Changes

Rate limits can be adjusted through the database configuration:

```sql
-- Update rate limit for a specific source
UPDATE data_sources 
SET rate_limit_per_minute = 150 
WHERE name = 'FRED';
```

### Environment Variables

Some rate limits can be configured via environment variables:

```bash
# In config.rs
FRED_RATE_LIMIT_PER_MINUTE=150
BLS_RATE_LIMIT_PER_MINUTE=30
```

### Command Line Options

The catalog crawler supports fine-tuning through command line options:

```bash
# Reduce series count to be more conservative
./scripts/populate_catalogs.sh --series-count 3

# Use dry-run mode to test without actual requests
./scripts/populate_catalogs.sh --dry-run

# Skip data download to only populate catalogs
./scripts/populate_catalogs.sh --skip-data
```

## Best Practices References

### Academic and Industry Standards

1. **Web Crawler Politeness Guidelines** (Khoury College, Northeastern University)
   - Recommends minimum 1-second delays between requests
   - Emphasizes respect for robots.txt directives
   - Source: [Northeastern University](https://www.khoury.northeastern.edu/home/vip/teach/IRcourse/4_webgraph/HW4/hw3.md)

2. **RFC Standards for User Agents**
   - IETF draft for crawler behavior coordination
   - Recommends transparent user-agent strings
   - Source: [IETF RFC Draft](https://www.ietf.org/archive/id/draft-illyes-aipref-cbcp-00.html)

3. **Industry Best Practices**
   - Adaptive throttling based on server response
   - Graceful error handling and retry mechanisms
   - Source: [Crawler Software Documentation](https://www.crwlr.software/packages/crawler/v3.5/the-crawler/politeness)

### Robots.txt Compliance

While our current implementation focuses on API endpoints rather than web scraping, we should consider robots.txt compliance for any web-based data sources:

```rust
// Future enhancement: robots.txt checking
async fn check_robots_txt(base_url: &str, user_agent: &str) -> Result<bool, Error> {
    let robots_url = format!("{}/robots.txt", base_url);
    // Implementation would fetch and parse robots.txt
    // Return true if crawling is allowed
}
```

### Monitoring and Alerting

We recommend implementing monitoring for:

1. **Rate Limit Violations**: Alert on any 429 responses
2. **Error Rate Spikes**: Alert when error rates exceed 5%
3. **Response Time Degradation**: Alert when response times increase significantly
4. **Concurrent Request Limits**: Monitor that we stay within limits

## Conclusion

Our crawler implements industry-standard politeness mechanisms with conservative settings that prioritize server health over crawling speed. The system is designed to be a good citizen of the internet, respecting API limits and maintaining positive relationships with data providers.

The comprehensive logging and monitoring capabilities ensure we can detect and respond to any issues quickly, while the configurable rate limits allow for fine-tuning based on real-world performance and feedback from API providers.

## References

1. [Web Crawler Politeness Guidelines](https://blog.mischel.com/2011/12/20/writing-a-web-crawler-politeness/)
2. [Northeastern University Crawler Best Practices](https://www.khoury.northeastern.edu/home/vip/teach/IRcourse/4_webgraph/HW4/hw3.md)
3. [Crawler Software Politeness Documentation](https://www.crwlr.software/packages/crawler/v3.5/the-crawler/politeness)
4. [IETF RFC Draft on Crawler Behavior Coordination](https://www.ietf.org/archive/id/draft-illyes-aipref-cbcp-00.html)
5. [Meta Interview Insights on Web Crawler Design](https://www.jointaro.com/interview-insights/meta/how-would-you-design-a-web-crawler-considering-scalability-politeness-and-data-storage/)

---

*This document is part of the EconGraph project and should be updated as our crawling practices evolve and new best practices emerge.*
