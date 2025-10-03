# EconGraph Financial Data Service

A financial data service with Iceberg storage and GraphQL API for the EconGraph project.

## Overview

This service provides:
- **GraphQL API** for querying financial time series data
- **Clean data models** without crawler operational concerns
- **Iceberg storage** with tiered storage support
- **Crawler integration** for bulk data ingestion

## Architecture

```
┌────────────────────────────────────────┐
│          Financial Data Service        │
├────────────────────────────────────────┤
│  GraphQL Read API (Apollo Server)     │
│  Clean Data Write API (REST/GraphQL)  │
│  Apache Parquet + Iceberg + Arrow    │
│  Clean Financial Time Series Data    │
└────────────────────────────────────────┘
```

## Features

### **Clean Data Models**
- `EconomicSeries`: Clean financial series metadata
- `DataPoint`: Validated time series data points
- `DataSource`: Source information for series

### **GraphQL API**
- Query economic series and data points
- Date range filtering for time series
- Health check endpoint

### **Crawler Integration**
- Clean data write API for bulk ingestion
- Data validation and processing
- Error handling and reporting

## Development

### **Running Tests**
```bash
cargo test
```

### **Running the Service**
```bash
cargo run
```

The service will start on `http://localhost:3001` with GraphQL endpoint at `/graphql`.

## Configuration

Storage tiering and database configuration will be added in future iterations.

## API Examples

### **Query a Series**
```graphql
query {
  series(id: "123e4567-e89b-12d3-a456-426614174000") {
    id
    title
    description
    units
    frequency
  }
}
```

### **Query Data Points**
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

## Documentation

- **API Documentation**: See `docs/API.md` - Comprehensive API reference with examples
- **GraphQL Schema**: See `docs/GraphQL-Schema.md` - Complete GraphQL schema documentation  
- **Storage Architecture**: See `docs/Storage-Architecture.md` - Detailed storage architecture guide
- **Examples**: See `examples/` directory - Working examples and tutorials
- **Performance**: See `benches/` directory - Performance benchmarks and testing

## Examples

### **Basic Usage**
```bash
cargo run --example basic-usage
```

### **GraphQL Client**
```bash
# Terminal 1: Start service
cargo run

# Terminal 2: Run client
cargo run --example graphql-client
```

### **Performance Testing**
```bash
cargo run --example performance-testing
```

## Features Implemented

✅ **Core Functionality**
- GraphQL API with Query and Mutation operations
- Clean data models for economic series and data points
- In-memory and Parquet storage backends
- Arrow Flight integration for zero-copy data transfer
- Comprehensive error handling and validation

✅ **Storage Architecture**
- Arrow Flight + Parquet (V1) implementation
- Storage abstraction for easy Iceberg (V2) migration
- Memory-mapped files for hot data
- Time series indexing for fast queries

✅ **Development Tools**
- Comprehensive test suite with integration tests
- Performance benchmarks with Criterion
- Docker and deployment configuration
- Monitoring and health checks
- Structured logging with tracing

✅ **Documentation**
- Complete API documentation
- GraphQL schema reference
- Storage architecture guide
- Working examples and tutorials
- Performance testing examples

## Next Steps

- [ ] Implement Iceberg storage integration (V2)
- [ ] Add storage tiering with MinIO
- [ ] Implement real-time streaming with Apache Kafka
- [ ] Add advanced analytics with SQL query support
- [ ] Create multi-region data replication
