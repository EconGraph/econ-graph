# Crawler Integration Design

## Overview

This document designs a very oversimplified API for crawlers to provide data to the financial data service, along with a data discovery system. The goal is to create a minimal, working system that can be extended later.

## Design Principles

1. **Oversimplified**: Start with the absolute minimum viable API
2. **File-based**: Use simple file operations instead of complex service integration
3. **Self-contained**: Crawler manages its own data, service discovers it
4. **Extensible**: Design for future enhancement without breaking changes

## Architecture Options

### Option 1: File-Based Integration (Recommended for V1)

**Crawler Side:**
- Crawler creates Parquet files directly in a shared directory
- Crawler maintains a simple JSON catalog file
- Crawler updates catalog when new data is available

**Financial Data Service Side:**
- Service watches for new files in the data directory
- Service reads the catalog to discover available series
- Service provides GraphQL API for data discovery and access

**Benefits:**
- No network dependencies between crawler and service
- Simple to implement and debug
- Crawler can work independently
- Easy to test and validate

### Option 2: HTTP API Integration

**Crawler Side:**
- Crawler makes HTTP POST requests to financial data service
- Crawler sends data in JSON format
- Service validates and stores data

**Financial Data Service Side:**
- Service provides REST endpoints for data ingestion
- Service validates incoming data
- Service stores data and updates catalog

**Benefits:**
- Standard HTTP interface
- Can be extended to multiple crawlers
- Service controls data validation

**Drawbacks:**
- More complex error handling
- Network dependencies
- Requires service to be running

### Option 3: Message Queue Integration

**Crawler Side:**
- Crawler publishes messages to a queue (Redis, RabbitMQ)
- Messages contain data and metadata

**Financial Data Service Side:**
- Service consumes messages from queue
- Service processes and stores data

**Benefits:**
- Decoupled architecture
- Can handle high volume
- Built-in retry mechanisms

**Drawbacks:**
- Additional infrastructure complexity
- Overkill for prototype phase

## Recommended Approach: File-Based Integration

For the prototype phase, we recommend **Option 1: File-Based Integration** because:

1. **Simplicity**: No network dependencies or complex protocols
2. **Reliability**: Files are persistent and can be retried
3. **Debugging**: Easy to inspect files and catalog
4. **Performance**: Direct file access is fast
5. **Scalability**: Can be enhanced later without breaking changes

## Detailed Design

### 1. Directory Structure

```
/data/
├── catalog.json                 # Master catalog of available series
├── series/                      # Individual series data
│   ├── {series_id}/
│   │   ├── metadata.json       # Series metadata
│   │   ├── data.parquet        # Latest data
│   │   ├── data_2024-01.parquet # Historical data by month
│   │   └── data_2024-02.parquet
│   └── {another_series_id}/
│       ├── metadata.json
│       └── data.parquet
└── temp/                        # Temporary files during processing
    └── {crawler_id}/
        ├── pending_series.json
        └── processing.log
```

### 2. Catalog Format

```json
{
  "version": "1.0",
  "last_updated": "2024-12-19T10:30:00Z",
  "series": {
    "123e4567-e89b-12d3-a456-426614174000": {
      "id": "123e4567-e89b-12d3-a456-426614174000",
      "external_id": "GDP_USA",
      "title": "US Gross Domestic Product",
      "description": "Quarterly GDP data for the United States",
      "frequency": "Quarterly",
      "units": "Billions of USD",
      "source": "BEA",
      "data_range": {
        "start_date": "2020-01-01",
        "end_date": "2024-10-01",
        "last_updated": "2024-12-19T10:30:00Z"
      },
      "files": [
        {
          "filename": "data.parquet",
          "date_range": {
            "start_date": "2020-01-01",
            "end_date": "2024-10-01"
          },
          "size_bytes": 1024000,
          "created_at": "2024-12-19T10:30:00Z"
        }
      ],
      "status": "active"
    }
  }
}
```

### 3. Series Metadata Format

```json
{
  "series_id": "123e4567-e89b-12d3-a456-426614174000",
  "external_id": "GDP_USA",
  "title": "US Gross Domestic Product",
  "description": "Quarterly GDP data for the United States",
  "frequency": "Quarterly",
  "units": "Billions of USD",
  "seasonal_adjustment": "Seasonally Adjusted Annual Rate",
  "source": {
    "name": "BEA",
    "url": "https://www.bea.gov/data/gdp",
    "crawler_id": "bea_gdp_crawler"
  },
  "data_range": {
    "start_date": "2020-01-01",
    "end_date": "2024-10-01",
    "last_updated": "2024-12-19T10:30:00Z",
    "total_points": 20
  },
  "quality": {
    "completeness": 0.95,
    "last_validation": "2024-12-19T10:30:00Z",
    "issues": []
  }
}
```

### 4. Crawler API (Oversimplified)

#### 4.1 Data Ingestion

**File Creation:**
```bash
# Crawler creates Parquet file
crawler create-parquet \
  --series-id "123e4567-e89b-12d3-a456-426614174000" \
  --data-file "data.parquet" \
  --metadata-file "metadata.json"
```

**Catalog Update:**
```bash
# Crawler updates catalog
crawler update-catalog \
  --series-id "123e4567-e89b-12d3-a456-426614174000" \
  --action "add" \
  --data-range "2020-01-01,2024-10-01"
```

#### 4.2 Data Validation

**Schema Validation:**
```bash
# Validate Parquet file against schema
crawler validate-parquet \
  --file "data.parquet" \
  --schema "economic_series_schema.json"
```

**Data Quality Check:**
```bash
# Check data quality
crawler check-quality \
  --file "data.parquet" \
  --rules "quality_rules.json"
```

### 5. Financial Data Service API

#### 5.1 Data Discovery

**GraphQL Queries:**
```graphql
# List all available series
query {
  discoverSeries {
    id
    externalId
    title
    description
    frequency
    dataRange {
      startDate
      endDate
      lastUpdated
    }
    status
  }
}

# Get series by external ID
query {
  discoverSeriesByExternalId(externalId: "GDP_USA") {
    id
    title
    dataRange {
      startDate
      endDate
    }
    files {
      filename
      dateRange {
        startDate
        endDate
      }
      sizeBytes
    }
  }
}

# Check data availability for date range
query {
  checkDataAvailability(
    seriesId: "123e4567-e89b-12d3-a456-426614174000"
    startDate: "2024-01-01"
    endDate: "2024-12-31"
  ) {
    available
    coverage
    gaps {
      startDate
      endDate
    }
  }
}
```

#### 5.2 Data Access

**GraphQL Queries:**
```graphql
# Get series data
query {
  series(id: "123e4567-e89b-12d3-a456-426614174000") {
    id
    title
    dataPoints(startDate: "2024-01-01", endDate: "2024-12-31") {
      date
      value
      revisionDate
      isOriginalRelease
    }
  }
}

# Get series metadata
query {
  seriesMetadata(id: "123e4567-e89b-12d3-a456-426614174000") {
    id
    externalId
    title
    description
    frequency
    units
    source {
      name
      url
      crawlerId
    }
    dataRange {
      startDate
      endDate
      lastUpdated
      totalPoints
    }
    quality {
      completeness
      lastValidation
      issues
    }
  }
}
```

### 6. Implementation Plan

#### 6.1 Phase 1: Basic File Operations

**Crawler Side:**
- [ ] Create Parquet file writer
- [ ] Create catalog updater
- [ ] Add basic validation

**Financial Data Service Side:**
- [ ] Add file watcher for new data
- [ ] Implement catalog reader
- [ ] Add basic GraphQL queries for discovery

#### 6.2 Phase 2: Data Discovery

**Financial Data Service Side:**
- [ ] Implement series discovery queries
- [ ] Add data availability checking
- [ ] Add metadata queries

#### 6.3 Phase 3: Data Quality

**Both Sides:**
- [ ] Add data validation
- [ ] Add quality metrics
- [ ] Add error handling

### 7. Error Handling

#### 7.1 Crawler Errors

**File Creation Errors:**
- Invalid Parquet format
- Schema validation failures
- Disk space issues
- Permission problems

**Catalog Update Errors:**
- JSON format errors
- Concurrent access issues
- Invalid series IDs

#### 7.2 Service Errors

**File Reading Errors:**
- Corrupted Parquet files
- Missing metadata
- Schema mismatches

**Discovery Errors:**
- Catalog corruption
- Missing series
- Invalid date ranges

### 8. Monitoring and Observability

#### 8.1 Crawler Monitoring

**Metrics:**
- Files created per hour
- Data points processed
- Validation failures
- Catalog update frequency

**Logs:**
- File creation events
- Validation results
- Error conditions
- Performance metrics

#### 8.2 Service Monitoring

**Metrics:**
- Series discovered
- Data availability queries
- File read operations
- GraphQL query performance

**Logs:**
- Discovery events
- Data access patterns
- Error conditions
- Performance metrics

### 9. Future Enhancements

#### 9.1 Scalability

**Horizontal Scaling:**
- Multiple crawler instances
- Distributed file storage
- Load balancing

**Performance:**
- File compression
- Indexing
- Caching

#### 9.2 Advanced Features

**Data Management:**
- Data versioning
- Rollback capabilities
- Data archiving

**Integration:**
- Webhook notifications
- Event streaming
- Real-time updates

### 10. Testing Strategy

#### 10.1 Unit Tests

**Crawler Tests:**
- Parquet file creation
- Catalog updates
- Validation logic

**Service Tests:**
- File discovery
- Catalog reading
- GraphQL queries

#### 10.2 Integration Tests

**End-to-End Tests:**
- Crawler creates data
- Service discovers data
- GraphQL queries work
- Data access is correct

**Performance Tests:**
- Large file handling
- Concurrent access
- Query performance

### 11. Deployment Considerations

#### 11.1 File System

**Requirements:**
- Shared file system between crawler and service
- Sufficient disk space
- Backup and recovery

**Options:**
- Local file system (development)
- NFS (staging)
- Cloud storage (production)

#### 11.2 Security

**Access Control:**
- File permissions
- Directory isolation
- Audit logging

**Data Protection:**
- Encryption at rest
- Secure transmission
- Access monitoring

## Conclusion

This design provides a very oversimplified but functional approach to crawler integration. It focuses on:

1. **Simplicity**: File-based operations with minimal complexity
2. **Reliability**: Persistent files with retry capabilities
3. **Extensibility**: Clear path for future enhancements
4. **Testability**: Easy to test and validate

The system can be implemented incrementally, starting with basic file operations and gradually adding more sophisticated features as needed.
