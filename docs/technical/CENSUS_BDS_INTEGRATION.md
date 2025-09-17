# Census Bureau BDS Integration Documentation

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
- Creates `EconomicSeries` records in database
- Generates external IDs: `CENSUS_BDS_{VARIABLE}_{GEOGRAPHY}`

## Usage Examples

### Basic Data Fetching
```rust
use econ_graph_backend::services::series_discovery::census::fetch_bds_data;

let client = Client::new();
let variables = vec!["ESTAB".to_string(), "YEAR".to_string()];
let data_points = fetch_bds_data(&client, &variables, "us", 2020, 2021, &None).await?;
```

### Series Discovery
```rust
use econ_graph_backend::services::series_discovery::census::discover_census_series;

let pool = database_pool;
let discovered_series = discover_census_series(&pool).await?;
```

### Query Builder Usage
```rust
use econ_graph_backend::services::series_discovery::census::CensusQueryBuilder;

let query = CensusQueryBuilder::new()
    .variables(&["ESTAB", "FIRM", "YEAR"])
    .for_geography("state")
    .year_range(2020, 2022);

let url = query.build_url()?;
let data = execute_structured(&client, &query).await?;
```

## Crawler Integration

### Command Line Usage
```bash
# Crawl all data sources (includes Census)
./catalog_crawler crawl-all --database-url postgresql://... --series-count 10

# Crawl only Census Bureau
./catalog_crawler crawl-source "U.S. Census Bureau" --database-url postgresql://... --series-count 5
```

### Programmatic Usage
```rust
let discovery_service = SeriesDiscoveryService::new(None, None, None, None);
let census_series = discovery_service.discover_census_series(&pool).await?;
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

### Integration Tests
The integration includes comprehensive tests:
- `test_census_bds_integration_happy_path` - Basic functionality
- `test_census_bds_query_builder_integration` - Query builder validation
- `test_census_bds_sample_data_integration` - Sample data fetching
- `test_census_discovery_integration` - Series discovery
- `test_census_api_error_conditions` - Error handling
- `test_census_api_rate_limiting` - Rate limiting behavior

### Running Tests
```bash
# Run all Census integration tests
cargo test --lib services::series_discovery::census::integration_tests

# Run specific test
cargo test --lib test_census_bds_integration_happy_path
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
