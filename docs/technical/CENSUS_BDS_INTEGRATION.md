# Census Bureau BDS Integration Documentation

> **Status (2026):** the code described here was ported to the Census adapter in
> `backend/crates/econ-graph-crawler/src/sources/census.rs`; the old `series_discovery` module and
> `catalog_crawler` binary were removed. Run it via the crawl queue:
> `crawler discover --source CENSUS` / `crawler enqueue --source CENSUS --series <id>`, drained by `crawler-worker`
> (see [CRAWLER_DEPLOYMENT_GUIDE.md](./CRAWLER_DEPLOYMENT_GUIDE.md)). Paths below are historical.

## Overview

This document describes the integration with the U.S. Census Bureau's Business Dynamics Statistics (BDS) dataset through their Data API. The BDS provides comprehensive statistics on business establishments, firms, and job creation/destruction patterns.

## API Endpoints

### Base URL
```
https://api.census.gov/data/timeseries/bds
```

### Key Endpoints
- **Variables**: `/variables.json` - Lists all available BDS variables
- **Geography**: `/geography.json` - Lists available geographic levels
- **Data**: `/?get=VARIABLES&for=GEOGRAPHY&YEAR=YEARS` - Retrieves actual data

## Data Source Configuration

The Census Bureau data source is configured as:
- **Name**: "U.S. Census Bureau"
- **API Key Required**: `false` (no authentication needed)
- **Base URL**: `https://api.census.gov/data/timeseries/bds`
- **Enabled**: `true`
- **Visible**: `true`

## BDS Variables

### Economic Indicators
The integration focuses on key economic variables:
- `ESTAB` - Number of establishments
- `FIRM` - Number of firms
- `JOB_CREATION` - Job creation
- `JOB_DESTRUCTION` - Job destruction
- `NET_JOB_CREATION` - Net job creation
- `REALLOCATION` - Job reallocation
- `BIRTH` - Establishment births
- `DEATH` - Establishment deaths
- `ENTRY` - Firm entry
- `EXIT` - Firm exit

### Geographic Levels
- `us` - United States (national)
- `state` - State level
- `county` - County level
- `metro` - Metropolitan areas
- `cbsa` - Core Based Statistical Areas

### Time Coverage
- **Start Date**: 1978
- **End Date**: 2022 (latest available)
- **Frequency**: Annual

## API Response Format

The Census API returns data in a JSON array format:
```json
[
  ["ESTAB", "YEAR", "YEAR", "us"],
  ["7206748", "2020", "2020", "1"]
]
```

### Response Structure
- **First row**: Headers (variable names, geography codes)
- **Subsequent rows**: Data values
- **Last column**: Geography code (numeric)
- **YEAR column**: May appear duplicated (API quirk)

## Integration Components

### 1. CensusQueryBuilder
Constructs API requests with proper parameter formatting:
```rust
let query = CensusQueryBuilder::new()
    .variables(&["ESTAB", "FIRM", "YEAR"])
    .for_geography("us")
    .year_range(2020, 2021);
```

### 2. Response Parser
Converts Census API responses to structured data:
```rust
pub struct BdsDataPoint {
    pub variable: String,
    pub year: i32,
    pub value: Option<i64>,
    pub geography: String,
}
```

### 3. Series Discovery
Automatically discovers and catalogs BDS series:
- Fetches available variables and geography levels
- Filters for economic indicators
- Records each series in `series_metadata` (fetching then creates the `economic_series` rows)
- Generates external IDs: `CENSUS_BDS_{VARIABLE}_{GEOGRAPHY}`

## Usage Examples

The BDS integration is the `CensusAdapter` source adapter
(`backend/crates/econ-graph-crawler/src/sources/census.rs`). It is driven through the crawl queue
rather than called directly:

```bash
# Enqueue a discovery job; crawler-worker then records BDS series
# (economic variables x geographies) in series_metadata
crawler discover --source CENSUS

# Enqueue a series for fetching; crawler-worker drains the queue
crawler enqueue --source CENSUS --series CENSUS_BDS_ESTAB_us
```

Notes:
- Discovery produces external IDs `CENSUS_BDS_{VARIABLE}_{GEOGRAPHY}` for every economic variable
  and geography level, but only national series (`..._us`) can be fetched; others are rejected
  as permanent errors without making a request.
- Set `CENSUS_API_KEY` to send an API key; requests also work without one at lower rate limits.

## Crawler Integration

### Command Line Usage
```bash
# Enqueue Census catalog discovery; crawler-worker drains the queue
crawler discover --source CENSUS

# Enqueue data fetches for specific series
crawler enqueue --source CENSUS --series <series-id>
```

### Programmatic Usage
```rust
// econ_graph_crawler::sources::census (SourceAdapter): discover() / fetch_series()
let registry = econ_graph_crawler::sources::default_registry();
let census = registry.get(SourceId::Census).expect("census adapter");
let discovered = census.discover(&ctx).await?;
```

## Known Limitations

### API Constraints
1. **Multiple Years**: Requests with multiple years may return 204 No Content
2. **Rate Limiting**: No documented limits, but be respectful
3. **Data Availability**: Some variables may not be available for all geographic levels
4. **Response Format**: YEAR column may be duplicated in responses

### Data Quality
1. **Missing Values**: Some data points may be missing or suppressed
2. **Geographic Codes**: Numeric codes require mapping to human-readable names
3. **Time Lags**: Data may have 1-2 year publication delays

## Error Handling

### Common Error Conditions
- **204 No Content**: Usually indicates invalid parameters or no data available
- **400 Bad Request**: Invalid parameter format or unsupported combinations
- **Network Timeouts**: API may be slow or temporarily unavailable
- **Malformed Responses**: JSON parsing errors (rare)

### Error Handling Strategy
```rust
match fetch_bds_data(&client, &variables, geography, year_start, year_end, &None).await {
    Ok(data_points) => {
        // Process successful response
        println!("Fetched {} data points", data_points.len());
    }
    Err(AppError::ExternalApiError(msg)) => {
        // Handle API errors gracefully
        if msg.contains("204") {
            println!("No data available for this query");
        } else {
            println!("API error: {}", msg);
        }
    }
    Err(e) => {
        // Handle other errors
        println!("Unexpected error: {}", e);
    }
}
```

## Testing

### Adapter Tests
`sources/census.rs` tests run against a mocked Census API, alongside the shared adapter contract
tests:
- `discover_crosses_economic_variables_with_geographies` - Discovery from `variables.json` and `geography.json`
- `discover_needs_both_metadata_files` - Discovery fails cleanly if either metadata file is missing
- `fetch_parses_rows_and_sends_key` / `fetch_works_without_a_key_and_applies_since` - Data fetching, API key, incremental `since`
- `fetch_rejects_non_national_and_foreign_ids_without_requests` - ID validation
- `census_specific_errors` - Census error classification
- `row_parsing_rules` - BDS row parsing

### Running Tests
```bash
# Run the Census adapter tests (mocked API, no network needed)
cargo test -p econ-graph-crawler --all-features sources::census

# Run a specific test
cargo test -p econ-graph-crawler --all-features sources::census::tests::fetch_parses_rows_and_sends_key
```

## Performance Considerations

### API Efficiency
- **Single Requests**: Most efficient for single variable/year combinations
- **Batch Requests**: May fail for multiple years (API limitation)
- **Geographic Scope**: Larger geographic areas may return more data

### Database Performance
- **Series Creation**: Creates one series per variable/geography combination
- **Bulk Operations**: Uses batch inserts for series metadata
- **Indexing**: External IDs are indexed for fast lookups

## Deployment Notes

### Environment Variables
No API keys required for Census Bureau integration.

### Database Migrations
Ensure the `api_key_name` column exists in the `data_sources` table:
```sql
ALTER TABLE data_sources ADD COLUMN api_key_name VARCHAR(255);
```

### Monitoring
- Monitor API response times (may be slow)
- Track 204 responses (indicates API limitations)
- Watch for parsing errors in logs

## Future Enhancements

### Potential Improvements
1. **Multiple Dataset Support**: Extend to other Census datasets (ACS, Economic Census)
2. **Geographic Mapping**: Add human-readable geography names
3. **Data Validation**: Enhanced validation of response data
4. **Caching**: Implement response caching for frequently accessed data
5. **Batch Processing**: Optimize for bulk data downloads

### API Evolution
- Monitor Census API changes and deprecations
- Adapt to new response formats if they change
- Consider alternative endpoints if available

## Troubleshooting

### Common Issues

#### "No data available" (204 responses)
- Check if the variable exists for the requested geography
- Verify the year range is within available data (1978-2022)
- Try single year requests instead of ranges

#### Parsing errors
- Check for unexpected response format changes
- Verify JSON structure matches expected format
- Look for API service announcements

#### Slow responses
- Census API can be slow during peak hours
- Consider implementing retry logic with exponential backoff
- Monitor API status page for known issues

### Debug Mode
Enable debug logging to see detailed API interactions:
```rust
env::set_var("RUST_LOG", "debug");
```

## References

- [Census Data API Documentation](https://www.census.gov/data/developers/data-sets.html)
- [BDS Dataset Information](https://www.census.gov/programs-surveys/bds.html)
- [API Rate Limiting Guidelines](https://www.census.gov/data/developers/guidance/api-user-guide.html)
