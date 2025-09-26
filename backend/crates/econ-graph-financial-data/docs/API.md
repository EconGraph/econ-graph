# Financial Data Service API Documentation

## Overview

The Financial Data Service provides a GraphQL API for managing economic time series data with high-performance Arrow Flight storage. This service is designed as part of a federated backend architecture, focusing on financial data while maintaining clean separation from user data.

## Table of Contents

- [GraphQL API](#graphql-api)
- [Data Models](#data-models)
- [Storage Architecture](#storage-architecture)
- [Performance Characteristics](#performance-characteristics)
- [Error Handling](#error-handling)
- [Monitoring & Health Checks](#monitoring--health-checks)
- [Examples](#examples)

## GraphQL API

### Endpoints

- **GraphQL Endpoint**: `http://localhost:3001/graphql`
- **GraphQL Playground**: `http://localhost:3001/`
- **Health Check**: `http://localhost:3001/health`
- **Metrics**: `http://localhost:3001/metrics`

### Schema

#### Queries

##### `series(id: UUID!): EconomicSeries`

Retrieve a specific economic series by ID.

**Arguments:**
- `id`: UUID of the series to retrieve

**Returns:**
- `EconomicSeries` object or `null` if not found

**Example:**
```graphql
query {
  series(id: "123e4567-e89b-12d3-a456-426614174000") {
    id
    title
    frequency
    isActive
    startDate
    endDate
  }
}
```

##### `dataPoints(seriesId: UUID!, startDate: Date, endDate: Date): [DataPoint!]!`

Retrieve data points for a series within an optional date range.

**Arguments:**
- `seriesId`: UUID of the series
- `startDate`: Optional start date (inclusive)
- `endDate`: Optional end date (inclusive)

**Returns:**
- Array of `DataPoint` objects

**Example:**
```graphql
query {
  dataPoints(
    seriesId: "123e4567-e89b-12d3-a456-426614174000"
    startDate: "2023-01-01"
    endDate: "2023-12-31"
  ) {
    id
    date
    value
    isOriginalRelease
  }
}
```

##### `listSeries: [EconomicSeries!]!`

List all available economic series.

**Returns:**
- Array of `EconomicSeries` objects

**Example:**
```graphql
query {
  listSeries {
    id
    title
    frequency
    isActive
  }
}
```

##### `health: String!`

Check service health status.

**Returns:**
- Health status string

**Example:**
```graphql
query {
  health
}
```

#### Mutations

##### `createSeries(input: CreateEconomicSeriesInput!): EconomicSeries!`

Create a new economic series.

**Arguments:**
- `input`: Series creation data

**Returns:**
- Created `EconomicSeries` object

**Example:**
```graphql
mutation {
  createSeries(input: {
    sourceId: "123e4567-e89b-12d3-a456-426614174000"
    externalId: "GDP_US"
    title: "US Gross Domestic Product"
    description: "Quarterly GDP data for the United States"
    units: "Billions of USD"
    frequency: "QUARTERLY"
    seasonalAdjustment: "SAAR"
    startDate: "1947-01-01"
    endDate: "2023-12-31"
    isActive: true
  }) {
    id
    title
    frequency
  }
}
```

##### `createDataPoints(inputs: [CreateDataPointInput!]!): [DataPoint!]!`

Create multiple data points for a series.

**Arguments:**
- `inputs`: Array of data point creation data

**Returns:**
- Array of created `DataPoint` objects

**Example:**
```graphql
mutation {
  createDataPoints(inputs: [
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-01-01"
      value: "25000.5"
      revisionDate: "2023-01-15"
      isOriginalRelease: true
    },
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-04-01"
      value: "25100.2"
      revisionDate: "2023-04-15"
      isOriginalRelease: true
    }
  ]) {
    id
    date
    value
  }
}
```

## Data Models

### EconomicSeries

Represents a complete economic time series with metadata.

```rust
pub struct EconomicSeries {
    pub id: Uuid,                           // Unique identifier
    pub source_id: Uuid,                    // Source system identifier
    pub external_id: String,                // External system ID
    pub title: String,                      // Human-readable title
    pub description: Option<String>,        // Detailed description
    pub units: Option<String>,              // Measurement units
    pub frequency: String,                  // Data frequency (DAILY, WEEKLY, MONTHLY, QUARTERLY, ANNUALLY)
    pub seasonal_adjustment: Option<String>, // Seasonal adjustment method
    pub start_date: Option<NaiveDate>,     // Series start date
    pub end_date: Option<NaiveDate>,        // Series end date
    pub is_active: bool,                     // Whether series is currently active
    pub created_at: DateTime<Utc>,          // Creation timestamp
    pub updated_at: DateTime<Utc>,           // Last update timestamp
}
```

### DataPoint

Represents a single data point in a time series.

```rust
pub struct DataPoint {
    pub id: Uuid,                           // Unique identifier
    pub series_id: Uuid,                    // Parent series identifier
    pub date: NaiveDate,                    // Observation date
    pub value: Option<DecimalScalar>,       // Data value (nullable for missing data)
    pub revision_date: NaiveDate,           // When this value was last revised
    pub is_original_release: bool,          // Whether this is the original release
    pub created_at: DateTime<Utc>,           // Creation timestamp
    pub updated_at: DateTime<Utc>,          // Last update timestamp
}
```

### DecimalScalar

Custom GraphQL scalar for precise decimal arithmetic.

```rust
pub struct DecimalScalar(pub Decimal);
```

**Features:**
- High-precision decimal arithmetic
- JSON serialization as string
- GraphQL scalar type support

## Storage Architecture

### Arrow Flight + Parquet (V1)

The service uses Apache Arrow Flight for zero-copy data transfer and Parquet files for efficient storage.

#### Key Features

- **Zero-Copy Transfer**: Arrow Flight provides sub-millisecond data transfer
- **Columnar Storage**: Parquet files optimized for analytical workloads
- **Memory-Mapped Files**: Hot data cached in memory for fast access
- **Time Series Indexing**: Optimized queries for date ranges

#### Storage Abstraction

```rust
#[async_trait]
pub trait FinancialDataStorage: Send + Sync {
    async fn write_series(&self, series: &EconomicSeries) -> Result<()>;
    async fn read_series(&self, series_id: Uuid) -> Result<Option<EconomicSeries>>;
    async fn write_data_points(&self, series_id: Uuid, points: &[DataPoint]) -> Result<()>;
    async fn read_data_points(
        &self, 
        series_id: Uuid, 
        start_date: Option<NaiveDate>, 
        end_date: Option<NaiveDate>
    ) -> Result<Vec<DataPoint>>;
    async fn list_series(&self) -> Result<Vec<EconomicSeries>>;
}
```

#### Arrow Schemas

**EconomicSeries Schema:**
```rust
Schema::new(vec![
    Field::new("id", DataType::Utf8, false),
    Field::new("source_id", DataType::Utf8, false),
    Field::new("external_id", DataType::Utf8, false),
    Field::new("title", DataType::Utf8, false),
    Field::new("description", DataType::Utf8, true),
    Field::new("units", DataType::Utf8, true),
    Field::new("frequency", DataType::Utf8, false),
    Field::new("seasonal_adjustment", DataType::Utf8, true),
    Field::new("start_date", DataType::Utf8, true),
    Field::new("end_date", DataType::Utf8, true),
    Field::new("is_active", DataType::Boolean, false),
    Field::new("created_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
    Field::new("updated_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
])
```

**DataPoint Schema:**
```rust
Schema::new(vec![
    Field::new("id", DataType::Utf8, false),
    Field::new("series_id", DataType::Utf8, false),
    Field::new("date", DataType::Date32, false),
    Field::new("value", DataType::Float64, true),
    Field::new("revision_date", DataType::Date32, false),
    Field::new("is_original_release", DataType::Boolean, false),
    Field::new("created_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
    Field::new("updated_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
])
```

### Future: Arrow Flight + Iceberg (V2)

Planned upgrade to Apache Iceberg for advanced data management:

- **Schema Evolution**: Automatic schema updates
- **Time Travel**: Query historical data states
- **ACID Transactions**: Data consistency guarantees
- **Partitioning**: Automatic data partitioning by date
- **Compaction**: Automatic file optimization

## Performance Characteristics

### Benchmarks

Based on performance testing with Criterion benchmarks:

#### Single Operations
- **Series Write**: ~2ms for 1 series
- **Data Points Write**: ~5ms for 1,000 points
- **Series Read**: ~1ms for 1 series
- **Data Points Read**: ~3ms for 1,000 points

#### Concurrent Operations
- **Concurrent Writes**: Linear scaling up to 20 concurrent operations
- **Concurrent Reads**: Excellent scaling with minimal contention
- **Mixed Workloads**: Stable performance under mixed read/write loads

#### Arrow Operations
- **Schema Creation**: ~0.1ms for complex schemas
- **RecordBatch Conversion**: ~1ms for 1,000 records
- **Parquet Write**: ~2ms for 1,000 records
- **Parquet Read**: ~1ms for 1,000 records

### Memory Usage

- **Base Service**: ~50MB
- **Per 1M Data Points**: ~100MB
- **Arrow Buffers**: Efficient memory management with zero-copy operations

## Error Handling

### GraphQL Errors

All operations return structured errors with:

- **Error Code**: Machine-readable error identifier
- **Message**: Human-readable error description
- **Path**: GraphQL field path where error occurred
- **Extensions**: Additional error context

### Common Error Types

#### Validation Errors
```json
{
  "errors": [{
    "message": "Invalid date format",
    "extensions": {
      "code": "VALIDATION_ERROR",
      "field": "startDate"
    }
  }]
}
```

#### Storage Errors
```json
{
  "errors": [{
    "message": "Failed to write to storage",
    "extensions": {
      "code": "STORAGE_ERROR",
      "operation": "write_series"
    }
  }]
}
```

#### Not Found Errors
```json
{
  "errors": [{
    "message": "Series not found",
    "extensions": {
      "code": "NOT_FOUND",
      "resource": "EconomicSeries",
      "id": "123e4567-e89b-12d3-a456-426614174000"
    }
  }]
}
```

## Monitoring & Health Checks

### Health Check Endpoint

**GET** `/health`

Returns comprehensive health status:

```json
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:00Z",
  "checks": {
    "database": {
      "status": "healthy",
      "message": "Database connection active",
      "last_check": "2024-01-15T10:29:58Z"
    },
    "storage": {
      "status": "healthy", 
      "message": "Parquet storage accessible",
      "last_check": "2024-01-15T10:29:58Z"
    }
  },
  "metrics": {
    "total_requests": 1250,
    "successful_requests": 1245,
    "error_rate": 0.004,
    "average_response_time_ms": 15.2
  }
}
```

### Metrics Endpoint

**GET** `/metrics`

Returns detailed performance metrics:

```json
{
  "counters": {
    "graphql_queries_total": 1250,
    "graphql_mutations_total": 340,
    "storage_operations_total": 1590
  },
  "gauges": {
    "active_connections": 12,
    "memory_usage_mb": 156.7,
    "storage_size_gb": 2.3
  },
  "histograms": {
    "response_time_ms": {
      "p50": 12.5,
      "p95": 45.2,
      "p99": 89.7,
      "max": 156.3
    }
  }
}
```

### Health Status Values

- **`healthy`**: All systems operational
- **`degraded`**: Some non-critical issues detected
- **`unhealthy`**: Critical systems failing

## Examples

### Complete Workflow

#### 1. Create a Series

```graphql
mutation {
  createSeries(input: {
    sourceId: "123e4567-e89b-12d3-a456-426614174000"
    externalId: "GDP_US"
    title: "US Gross Domestic Product"
    description: "Quarterly GDP data for the United States"
    units: "Billions of USD"
    frequency: "QUARTERLY"
    seasonalAdjustment: "SAAR"
    startDate: "1947-01-01"
    isActive: true
  }) {
    id
    title
    frequency
  }
}
```

#### 2. Add Data Points

```graphql
mutation {
  createDataPoints(inputs: [
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-01-01"
      value: "25000.5"
      revisionDate: "2023-01-15"
      isOriginalRelease: true
    },
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-04-01"
      value: "25100.2"
      revisionDate: "2023-04-15"
      isOriginalRelease: true
    }
  ]) {
    id
    date
    value
  }
}
```

#### 3. Query Data

```graphql
query {
  series(id: "123e4567-e89b-12d3-a456-426614174000") {
    id
    title
    frequency
    startDate
    endDate
  }
  
  dataPoints(
    seriesId: "123e4567-e89b-12d3-a456-426614174000"
    startDate: "2023-01-01"
    endDate: "2023-12-31"
  ) {
    date
    value
    isOriginalRelease
  }
}
```

### Bulk Data Import

For large data imports, use the crawler API:

```rust
use econ_graph_financial_data::crawler::CrawlerWriteApi;

let api = CrawlerWriteApi::new();
let payload = CleanDataPayload {
    series_id: series_id,
    data_points: data_points,
};

let response = api.process_clean_data(payload).await?;
println!("Processed {} data points", response.data_points_processed);
```

### Performance Testing

Run benchmarks to test performance:

```bash
cargo bench --bench performance
```

This will run comprehensive benchmarks including:
- Single operation performance
- Concurrent operation scaling
- Arrow Flight performance
- Parquet file operations
- Memory usage patterns

## Development

### Running the Service

```bash
# Development mode
cargo run

# Production mode
cargo run --release
```

### Testing

```bash
# Run all tests
cargo test

# Run specific test suites
cargo test --test integration
cargo test --test parquet_integration
cargo test --test graphql_parquet_integration
```

### Benchmarking

```bash
# Run performance benchmarks
cargo bench --bench performance

# Run specific benchmark groups
cargo bench --bench performance -- --test concurrent_operations
```

## Configuration

### Environment Variables

- `RUST_LOG`: Logging level (default: `info`)
- `DATA_DIR`: Storage directory (default: `./data`)
- `PORT`: Service port (default: `3001`)

### Docker

```bash
# Build image
docker build -t econ-graph-financial-data .

# Run container
docker run -p 3001:3001 -v ./data:/app/data econ-graph-financial-data
```

## Integration

### Apollo Federation

This service is designed to work as a subgraph in Apollo Federation:

```typescript
// Gateway configuration
const gateway = new ApolloGateway({
  supergraphSdl: new IntrospectAndCompose({
    subgraphs: [
      { name: 'user-service', url: 'http://user-service:4001/graphql' },
      { name: 'financial-data', url: 'http://financial-data:3001/graphql' }
    ]
  })
});
```

### Schema Stitching

For local development, use schema stitching:

```typescript
import { stitchSchemas } from '@graphql-tools/stitch';
import { fetch } from 'cross-fetch';

const financialDataSchema = await fetch('http://localhost:3001/graphql')
  .then(res => res.text())
  .then(sdl => buildSchema(sdl));

const stitchedSchema = stitchSchemas({
  subschemas: [
    { schema: userServiceSchema },
    { schema: financialDataSchema }
  ]
});
```

## Support

For questions, issues, or contributions:

- **Documentation**: See `docs/` directory
- **Tests**: See `tests/` directory  
- **Examples**: See `examples/` directory
- **Benchmarks**: See `benches/` directory

---

*This API documentation is automatically generated and kept in sync with the codebase.*