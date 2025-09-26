# API Design

## Overview

This appendix covers the API design for the financial data service, including crawler integration, GraphQL APIs, and schema separation.

## Table of Contents

1. [Crawler Write API Design](crawler-write-api-design.md)
2. [Schema Separation Analysis](schema-separation-analysis.md)
3. [Existing Schema Analysis](existing-schema-analysis.md)

## Key Design Principles

### **Clean Separation of Concerns**
- **Financial Data Service**: Only handles clean, validated data
- **Crawler Service**: Handles raw data, validation, transformation
- **No Mixing**: Crawler operational concerns stay in crawler service

### **Dual Iceberg Architecture**
- **Crawler Service Iceberg**: Raw data archive, job history, validation patterns
- **Financial Service Iceberg**: Clean financial series and data points
- **Two Write Endpoints**: Crawler → Raw Archive, Crawler → Clean Data

### **Binary Serialization for Bulk Operations**
- **Format**: Protocol Buffers or custom binary format
- **Benefits**: Efficient transfer of large time series data
- **Use Case**: Crawler bulk updates to financial service

## API Architecture

### **Crawler Write API**
```rust
// Raw data preservation in crawler service
pub struct RawDataArchive {
    pub id: Uuid,
    pub batch_id: String,
    pub source: String,
    pub raw_data: serde_json::Value,
    pub validation_errors: Vec<ValidationError>,
    pub received_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub status: RawDataStatus,
}
```

### **Financial Data Service API**
```rust
// Clean data model - NO crawler fields
pub struct EconomicSeries {
    pub id: Uuid,
    pub source_id: Uuid,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub units: Option<String>,
    pub frequency: String,
    pub seasonal_adjustment: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

## GraphQL Schema

### **Clean API Boundaries**
```graphql
type EconomicSeries {
  id: ID!
  sourceId: ID!
  externalId: String!
  title: String!
  description: String
  units: String
  frequency: String!
  seasonalAdjustment: String
  startDate: Date
  endDate: Date
  isActive: Boolean!
  createdAt: DateTime!
  updatedAt: DateTime!
}

type DataPoint {
  id: ID!
  seriesId: ID!
  date: Date!
  value: Decimal
  revisionDate: Date!
  isOriginalRelease: Boolean!
  createdAt: DateTime!
  updatedAt: DateTime!
}
```

## Benefits of Clean Separation

1. **Maintainability**: Clear boundaries between concerns
2. **Scalability**: Services can evolve independently
3. **Testing**: Easier to test each service in isolation
4. **Data Integrity**: Raw data preserved, clean data validated
5. **Performance**: Optimized storage for each data type

## Implementation Strategy

### **Phase 1**: Standalone Financial Service
- Implement clean data models
- Build crawler write API
- Create GraphQL read API
- Test with existing backend

### **Phase 2**: Full Federation
- Integrate with Apollo Federation
- Implement cross-service authentication
- Production deployment

## Next Steps

1. Review existing schema analysis
2. Design clean data models
3. Implement crawler write API
4. Build GraphQL read API
5. Test integration with existing backend
