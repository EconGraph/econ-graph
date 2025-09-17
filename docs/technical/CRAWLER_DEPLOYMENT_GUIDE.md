# Crawler Deployment Guide

## Quick Start for End-to-End Testing

This guide provides the essential commands and configuration needed to test the Census Bureau BDS integration with the crawler.

## Prerequisites

1. **Database**: PostgreSQL with migrations applied
2. **Environment**: Rust toolchain installed
3. **Network**: Internet access for Census API calls

## Database Setup

### 1. Ensure Migrations are Applied
```bash
# From backend directory
diesel migration run
```

### 2. Verify Data Sources Table
```sql
-- Check that Census data source exists and is properly configured
SELECT name, api_key_required, api_key_name, is_enabled, is_visible 
FROM data_sources 
WHERE name = 'U.S. Census Bureau';
```

Expected result:
```
name: "U.S. Census Bureau"
api_key_required: false
api_key_name: NULL
is_enabled: true
is_visible: true
```

## Crawler Commands

### 1. Test Census Bureau Integration Only
```bash
# Crawl only Census Bureau (no API key needed)
./target/release/catalog_crawler crawl-source "U.S. Census Bureau" \
  --database-url "postgresql://user:pass@localhost/econ_graph_dev" \
  --series-count 5 \
  --dry-run
```

### 2. Crawl All Data Sources (Including Census)
```bash
# Crawl all sources, including Census
./target/release/catalog_crawler crawl-all \
  --database-url "postgresql://user:pass@localhost/econ_graph_dev" \
  --series-count 3 \
  --dry-run
```

### 3. Export Results to SQL Migration
```bash
# Export discovered series to SQL for review
./target/release/catalog_crawler export-catalog \
  --database-url "postgresql://user:pass@localhost/econ_graph_dev" \
  --output-file "census_series_migration.sql" \
  --bulk-insert-limit 100
```

## Expected Output

### Successful Census Crawl
```
INFO Discovering series for U.S. Census Bureau
INFO 📊 Found 45 variables and 5 geography levels
INFO 🔍 Filtering economic indicators...
INFO 📈 Creating series for 23 economic indicators across 5 geographic levels
INFO 📝 Created 10 series so far...
INFO 📝 Created 20 series so far...
INFO ✅ Discovered 115 Census series total
INFO Successfully discovered 115 series for U.S. Census Bureau
```

### Series Examples
The crawler should create series with external IDs like:
- `CENSUS_BDS_ESTAB_us` - Establishments at US level
- `CENSUS_BDS_FIRM_state` - Firms at state level
- `CENSUS_BDS_JOB_CREATION_county` - Job creation at county level

## Verification

### 1. Check Series in Database
```sql
-- Count Census series
SELECT COUNT(*) FROM economic_series 
WHERE external_id LIKE 'CENSUS_BDS_%';

-- View sample series
SELECT external_id, title, description 
FROM economic_series 
WHERE external_id LIKE 'CENSUS_BDS_%' 
LIMIT 10;
```

### 2. Check Data Source Configuration
```sql
-- Verify Census data source
SELECT * FROM data_sources WHERE name = 'U.S. Census Bureau';
```

## Troubleshooting

### Common Issues

#### "No such data source found"
- Ensure the data source exists: `SELECT * FROM data_sources WHERE name = 'U.S. Census Bureau';`
- Check case sensitivity in the command

#### "API request failed"
- Verify internet connectivity
- Check Census API status: https://api.census.gov/data/timeseries/bds
- Try with `--dry-run` first

#### "Database connection failed"
- Verify database URL format
- Ensure database is running
- Check user permissions

### Debug Mode
```bash
# Enable debug logging
RUST_LOG=debug ./target/release/catalog_crawler crawl-source "U.S. Census Bureau" \
  --database-url "postgresql://user:pass@localhost/econ_graph_dev" \
  --series-count 5
```

## Performance Expectations

### Typical Results
- **Discovery Time**: 30-60 seconds for full BDS catalog
- **Series Created**: 100-200 series (depending on filtering)
- **API Calls**: 3-5 calls (variables, geography, sample data)
- **Memory Usage**: < 100MB

### Rate Limiting
- Census API has no documented rate limits
- Crawler includes 100ms delays between requests
- Should complete without rate limiting issues

## Production Considerations

### Scaling
- For production, consider running crawler as scheduled job
- Monitor API response times and adjust timeouts
- Implement retry logic for transient failures

### Monitoring
- Log series discovery counts
- Monitor API error rates
- Track data freshness (last_crawled_at timestamps)

### Security
- No API keys required for Census Bureau
- Ensure database credentials are secure
- Consider network restrictions if needed

## Integration with Deployment

### Docker
```dockerfile
# Add to Dockerfile
COPY target/release/catalog_crawler /usr/local/bin/
RUN chmod +x /usr/local/bin/catalog_crawler
```

### Kubernetes CronJob
```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: census-crawler
spec:
  schedule: "0 2 * * *"  # Daily at 2 AM
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: crawler
            image: econ-graph-backend:latest
            command:
            - catalog_crawler
            - crawl-source
            - "U.S. Census Bureau"
            - --database-url
            - $(DATABASE_URL)
            - --series-count
            - "10"
```

## Next Steps

After successful crawler integration:

1. **Verify Data Quality**: Check that discovered series have proper metadata
2. **Test Data Retrieval**: Use the series downloader to fetch actual data points
3. **Monitor Performance**: Track crawler execution times and success rates
4. **Extend to Other Sources**: Apply same pattern to other data sources
5. **Automate Scheduling**: Set up regular crawling schedules

## Support

For issues with the Census Bureau integration:
- Check the integration tests: `cargo test --lib services::series_discovery::census`
- Review the detailed documentation: `docs/CENSUS_BDS_INTEGRATION.md`
- Monitor Census API status and announcements
