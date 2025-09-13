# Catalog Population Scripts

This directory contains scripts and tools for populating data source catalogs with real data from various economic data APIs.

## Overview

The catalog population process involves:
1. Creating a temporary dockerized PostgreSQL database
2. Running database migrations to set up the schema
3. Using the `catalog_crawler` binary to discover and populate series metadata
4. Optionally downloading sample data from each source
5. Exporting the populated catalog data as a Diesel migration

## Files

- `populate_catalogs.sh` - Main orchestration script
- `catalog_crawler` - Rust binary for crawling data sources (in `src/bin/catalog_crawler.rs`)

## Usage

### Quick Start

```bash
# Run with default settings (dry run mode)
./scripts/populate_catalogs.sh --dry-run

# Run with API keys for real data
./scripts/populate_catalogs.sh \
  --api-key FRED=your_fred_api_key \
  --api-key BLS=your_bls_api_key \
  --series-count 10
```

### Command Line Options

```bash
./scripts/populate_catalogs.sh [OPTIONS]

OPTIONS:
    -h, --help              Show help message
    -p, --port PORT         Database port (default: 5434)
    -s, --series-count N    Number of series to download per source (default: 5)
    -d, --dry-run           Dry run mode - don't download actual data
    -k, --skip-data         Skip downloading actual series data
    -o, --output FILE       Output migration file (default: catalog_migration.sql)
    -a, --api-key SOURCE=KEY
                           API key for data source (can be used multiple times)
    --cleanup               Clean up existing container before starting
```

### Examples

#### Dry Run (Recommended for Testing)
```bash
./scripts/populate_catalogs.sh --dry-run
```

#### With API Keys
```bash
./scripts/populate_catalogs.sh \
  --api-key FRED=abc123 \
  --api-key BLS=def456 \
  --api-key CENSUS=ghi789 \
  --series-count 5
```

#### Skip Data Download (Catalog Only)
```bash
./scripts/populate_catalogs.sh --skip-data --api-key FRED=abc123
```

#### Custom Output File
```bash
./scripts/populate_catalogs.sh \
  --output my_catalog.sql \
  --api-key FRED=abc123 \
  --series-count 10
```

## API Keys

Many data sources require API keys for access. Here's how to get them:

### FRED (Federal Reserve Economic Data)
- Website: https://fred.stlouisfed.org/docs/api/api_key.html
- Free registration required
- Rate limit: 120 requests per minute

### BLS (Bureau of Labor Statistics)
- Website: https://data.bls.gov/registrationEngine/
- Free registration required
- Rate limit: 500 requests per day

### Census Bureau
- Website: https://api.census.gov/data/key_signup.html
- Free registration required
- Rate limit: 500 requests per day

### BEA (Bureau of Economic Analysis)
- Website: https://apps.bea.gov/API/signup/
- Free registration required
- Rate limit: 1000 requests per day

### World Bank
- No API key required
- Rate limit: 300 requests per minute

### IMF
- No API key required
- Rate limit: 500 requests per day

### Other Sources
- ECB, OECD, BoE, WTO, BoJ, RBA, BoC, SNB, UN Stats, ILO typically don't require API keys
- Check individual API documentation for current requirements

## Direct Binary Usage

You can also use the `catalog_crawler` binary directly:

### Crawl All Sources
```bash
cargo run --release --bin catalog_crawler -- crawl-all \
  --database-url "postgresql://user:pass@localhost:5432/db" \
  --series-count 5 \
  --api-key FRED=abc123
```

### Crawl Single Source
```bash
cargo run --release --bin catalog_crawler -- crawl-source fred \
  --database-url "postgresql://user:pass@localhost:5432/db" \
  --series-count 10 \
  --api-key abc123
```

### Export Catalog Data
```bash
cargo run --release --bin catalog_crawler -- export-catalog \
  --database-url "postgresql://user:pass@localhost:5432/db" \
  --output-file catalog.sql
```

## Output

The script generates:
1. A SQL file with populated series metadata
2. A Diesel migration file in the `migrations/` directory
3. Log output showing progress and results

### Migration File Structure
```
migrations/
└── YYYY-MM-DD-HHMMSS_populate_catalog_data/
    ├── up.sql      # Contains INSERT statements for series metadata
    └── down.sql    # Contains rollback instructions
```

## Troubleshooting

### Common Issues

1. **Docker not running**
   ```
   [ERROR] Docker is not running. Please start Docker first.
   ```
   Solution: Start Docker Desktop or Docker daemon

2. **Port already in use**
   ```
   [ERROR] Database failed to start
   ```
   Solution: Use a different port with `--port 5435` or stop existing containers

3. **API key invalid**
   ```
   [WARNING] Failed to download series: 401 Unauthorized
   ```
   Solution: Check your API key and ensure it's valid

4. **Rate limiting**
   ```
   [WARNING] Failed to download series: 429 Too Many Requests
   ```
   Solution: The script includes delays between requests, but you may need to reduce `--series-count`

### Debug Mode

For more verbose output, set the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug ./scripts/populate_catalogs.sh --dry-run
```

## Data Sources

The crawler supports the following data sources:

| Source | API Key Required | Rate Limit | Notes |
|--------|------------------|------------|-------|
| FRED | Yes | 120/min | Federal Reserve Economic Data |
| BLS | Yes | 500/day | Bureau of Labor Statistics |
| Census | Yes | 500/day | US Census Bureau |
| BEA | Yes | 1000/day | Bureau of Economic Analysis |
| World Bank | No | 300/min | Global development data |
| IMF | No | 500/day | International Monetary Fund |
| FHFA | No | - | Federal Housing Finance Agency |
| ECB | No | - | European Central Bank |
| OECD | No | - | Organisation for Economic Co-operation |
| Bank of England | No | - | UK central bank |
| WTO | No | - | World Trade Organization |
| Bank of Japan | No | - | Japanese central bank |
| Reserve Bank of Australia | No | - | Australian central bank |
| Bank of Canada | No | - | Canadian central bank |
| Swiss National Bank | No | - | Swiss central bank |
| UN Statistics Division | No | - | United Nations statistics |
| ILO | No | - | International Labour Organization |

## Security Notes

- API keys are passed as command line arguments and may be visible in process lists
- Consider using environment variables for production use
- The script creates temporary containers that are automatically cleaned up
- Database passwords are generated randomly for the temporary container

## Contributing

To add support for new data sources:

1. Implement the discovery logic in `src/services/series_discovery/`
2. Add the source to the `crawl_data_source` function in `catalog_crawler.rs`
3. Update this README with the new source information
4. Test with the dry-run mode first
