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

## Next Steps

- [ ] Implement Iceberg storage integration
- [ ] Add storage tiering with MinIO
- [ ] Implement comprehensive error handling
- [ ] Add monitoring and metrics
- [ ] Create deployment configuration
