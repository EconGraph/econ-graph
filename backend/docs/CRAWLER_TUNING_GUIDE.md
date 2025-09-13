# Crawler Politeness Tuning Guide

## Quick Reference

This guide provides specific commands and configurations for fine-tuning crawler politeness settings.

## Environment Variables

```bash
# Rate limiting configuration
export FRED_RATE_LIMIT_PER_MINUTE=150
export BLS_RATE_LIMIT_PER_MINUTE=30
export BEA_RATE_LIMIT_PER_MINUTE=40

# Global settings
export CRAWLER_MAX_CONCURRENT_JOBS=3
export CRAWLER_DEFAULT_DELAY_MS=1000
```

## Database Configuration

### Update Rate Limits

```sql
-- Make FRED more aggressive (if API allows)
UPDATE data_sources 
SET rate_limit_per_minute = 150 
WHERE name = 'FRED';

-- Make BLS more conservative
UPDATE data_sources 
SET rate_limit_per_minute = 20 
WHERE name = 'BLS';

-- Disable a problematic source temporarily
UPDATE data_sources 
SET is_enabled = false 
WHERE name = 'PROBLEMATIC_SOURCE';
```

### Check Current Settings

```sql
-- View all rate limits
SELECT name, rate_limit_per_minute, is_enabled 
FROM data_sources 
ORDER BY rate_limit_per_minute DESC;

-- Check recent crawl attempts for errors
SELECT ds.name, ca.status, ca.error_message, COUNT(*) as attempt_count
FROM crawl_attempts ca
JOIN data_sources ds ON ca.data_source_id = ds.id
WHERE ca.created_at > NOW() - INTERVAL '24 hours'
  AND ca.status = 'failed'
GROUP BY ds.name, ca.status, ca.error_message
ORDER BY attempt_count DESC;
```

## Command Line Tuning

### Conservative Crawling

```bash
# Very conservative - only 3 series per source
./scripts/populate_catalogs.sh --series-count 3 --dry-run

# Skip data download, only populate catalogs
./scripts/populate_catalogs.sh --skip-data

# Use custom port to avoid conflicts
./scripts/populate_catalogs.sh --port 5435
```

### Aggressive Crawling (Use with Caution)

```bash
# More series per source (monitor for 429 errors)
./scripts/populate_catalogs.sh --series-count 10

# Multiple API keys for higher limits
./scripts/populate_catalogs.sh \
  --api-key FRED=your_fred_key \
  --api-key BLS=your_bls_key \
  --api-key CENSUS=your_census_key
```

### Testing Mode

```bash
# Test with verbose logging
RUST_LOG=debug ./scripts/populate_catalogs.sh --dry-run

# Test single source
cargo run --bin catalog_crawler crawl-source FRED \
  --database-url postgres://user:pass@localhost:5432/db \
  --api-key your_fred_key \
  --series-count 5
```

## Monitoring and Alerts

### Check for Rate Limit Violations

```bash
# Monitor logs for 429 errors
grep -i "429\|too many requests" /var/log/econ-graph/crawler.log

# Check recent crawl attempts
psql -d econ_graph -c "
SELECT ds.name, 
       COUNT(*) as total_attempts,
       COUNT(CASE WHEN ca.status = 'failed' THEN 1 END) as failed_attempts,
       COUNT(CASE WHEN ca.error_message LIKE '%429%' THEN 1 END) as rate_limit_errors
FROM crawl_attempts ca
JOIN data_sources ds ON ca.data_source_id = ds.id
WHERE ca.created_at > NOW() - INTERVAL '1 hour'
GROUP BY ds.name
ORDER BY rate_limit_errors DESC;
"
```

### Performance Metrics

```bash
# Check average response times
psql -d econ_graph -c "
SELECT ds.name,
       AVG(EXTRACT(EPOCH FROM (ca.ended_at - ca.started_at))) as avg_duration_seconds,
       COUNT(*) as total_crawls
FROM crawl_attempts ca
JOIN data_sources ds ON ca.data_source_id = ds.id
WHERE ca.created_at > NOW() - INTERVAL '24 hours'
  AND ca.status = 'completed'
GROUP BY ds.name
ORDER BY avg_duration_seconds DESC;
"
```

## Troubleshooting Common Issues

### 429 Too Many Requests

**Symptoms:**
```
[WARNING] Failed to download series: 429 Too Many Requests
```

**Solutions:**
1. Reduce rate limits in database
2. Increase delays in code
3. Reduce `--series-count`
4. Check if API key allows higher limits

```bash
# Emergency: Disable problematic source
psql -d econ_graph -c "
UPDATE data_sources SET is_enabled = false WHERE name = 'PROBLEMATIC_SOURCE';
"
```

### Slow Response Times

**Symptoms:**
- Response times > 5 seconds
- Timeout errors
- Connection refused

**Solutions:**
1. Reduce concurrency
2. Increase delays between requests
3. Check network connectivity
4. Verify API endpoint status

### High Error Rates

**Symptoms:**
- Error rate > 10%
- Multiple 5xx errors
- Connection timeouts

**Solutions:**
1. Implement exponential backoff
2. Reduce request frequency
3. Check server status pages
4. Contact API provider

## Code-Level Adjustments

### Increase Delays

```rust
// In catalog_crawler.rs - increase delays
tokio::time::sleep(tokio::time::Duration::from_secs(5)).await; // Was 2 seconds
tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await; // Was 500ms
```

### Adjust Rate Limits

```rust
// In enhanced_crawler_scheduler.rs - more conservative limits
rate_limits.insert(DataSource::FRED, 60); // Was 120
rate_limits.insert(DataSource::BLS, 10);  // Was 25
```

### Reduce Concurrency

```rust
// In enhanced_crawler_scheduler.rs - fewer concurrent jobs
max_concurrent_jobs: 3, // Was 5
```

## Best Practices for Tuning

1. **Start Conservative**: Always begin with lower limits and increase gradually
2. **Monitor Continuously**: Watch for errors and performance degradation
3. **Test in Staging**: Never tune production without testing first
4. **Document Changes**: Keep track of what settings work for each API
5. **Have Rollback Plan**: Be ready to revert changes if issues occur

## Emergency Procedures

### Stop All Crawling

```bash
# Disable all data sources
psql -d econ_graph -c "UPDATE data_sources SET is_enabled = false;"

# Kill running crawler processes
pkill -f catalog_crawler
pkill -f populate_catalogs
```

### Restore Default Settings

```sql
-- Reset to default rate limits
UPDATE data_sources SET rate_limit_per_minute = 120 WHERE name = 'FRED';
UPDATE data_sources SET rate_limit_per_minute = 25 WHERE name = 'BLS';
UPDATE data_sources SET rate_limit_per_minute = 30 WHERE name = 'BEA';
-- ... etc for other sources

-- Re-enable all sources
UPDATE data_sources SET is_enabled = true;
```

---

*For detailed explanations of these mechanisms, see [CRAWLER_POLITENESS.md](./CRAWLER_POLITENESS.md)*
