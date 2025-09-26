# V1 Implementation Plan - Financial Data Service Experiment

## Overview

Build a financial data service as an experimental branch that can be optionally integrated with the existing monolithic backend. This allows us to validate the time series database approach without disrupting the current system.

## V1 Goals

1. **Financial Data Service**: Standalone service with Parquet/Iceberg/Arrow stack
2. **Crawler Write API**: Dedicated API for crawlers to write financial data
3. **GraphQL Read API**: Read-only GraphQL API for financial data
4. **Optional Federation**: Experimental integration with existing backend
5. **Validation**: Test time series database performance and capabilities

## Architecture

### **Financial Data Service (New)**
```
┌────────────────────────────────────────┐
│          Financial Data Service        │
├────────────────────────────────────────┤
│  GraphQL Read API (Apollo Server)      │
│  Clean Data Write API (REST/GraphQL)   │
│  Apache Parquet + Iceberg + Arrow      │
│  Clean Financial Time Series Data      │
└────────────────────────────────────────┘
```

### **Crawler Service (New)**
```
┌────────────────────────────────────────┐
│             Crawler Service            │
├────────────────────────────────────────┤
│  Raw Data Archive (Iceberg)            │
│  Job Management & Scheduling           │
│  Data Validation & Transformation      │
│  Error Handling & Retry Logic          │
└────────────────────────────────────────┘
```

### **Existing Monolithic Backend (Unchanged)**
```
┌────────────────────────────────────────┐
│         Monolithic Backend             │
├────────────────────────────────────────┤
│  GraphQL API (Existing)                │
│  PostgreSQL Database                   │
│  All Current Functionality             │
└────────────────────────────────────────┘
```

### **Optional Federation (Experimental)**
```
┌────────────────────────────────────────┐
│           Apollo Gateway               │
├────────────────────────────────────────┤
│  Monolithic Backend (User Data)        │
│  Financial Data Service (Financial)    │
└────────────────────────────────────────┘
```

## Implementation Steps

### **Step 1: Financial Data Service Foundation**

**Create New Service:**
```bash
# New directory structure
backend/
├── financial-data-service/          # New service
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── graphql/                 # GraphQL schema and resolvers
│   │   ├── crawler/                 # Crawler write API
│   │   ├── database/                # Parquet/Iceberg/Arrow integration
│   │   └── models/                  # Data models
│   └── migrations/                  # Database migrations
└── backend/                         # Existing monolithic backend
    └── (unchanged)
```

**Technology Stack:**
- **Rust**: Same language as existing backend
- **GraphQL**: Apollo Server for read API
- **Storage**: Apache Arrow Flight + Parquet (V1) → Arrow Flight + Iceberg (V2)
- **Crawler API**: REST or GraphQL mutations

### **Step 2: Arrow Flight Storage Layer**

**Arrow Flight + Parquet Integration (V1):**
```rust
// Storage abstraction using Arrow Flight
pub struct FinancialDataService {
    arrow_flight_client: ArrowFlightClient,
    storage_backend: Box<dyn FinancialDataStorage>,
}

// Storage trait - same interface for V1 and V2
#[async_trait]
pub trait FinancialDataStorage: Send + Sync {
    async fn write_series(&self, series: &EconomicSeries) -> Result<()>;
    async fn read_series(&self, id: Uuid) -> Result<Option<EconomicSeries>>;
    async fn write_data_points(&self, series_id: Uuid, points: &[DataPoint]) -> Result<()>;
    async fn read_data_points(&self, series_id: Uuid, start: Option<NaiveDate>, end: Option<NaiveDate>) -> Result<Vec<DataPoint>>;
}

// V1: Direct Parquet files via Arrow Flight
pub struct ParquetStorage {
    flight_server: ArrowFlightServer,
    data_dir: PathBuf,
}

// V2: Iceberg-managed Parquet files via Arrow Flight  
pub struct IcebergStorage {
    flight_server: ArrowFlightServer,
    iceberg_catalog: IcebergCatalog,
}
```

**Benefits of Arrow Flight Approach:**
- **Zero-copy data transfer**: Direct memory-to-memory operations
- **Sub-millisecond latency**: No SQL parsing overhead
- **Future-proof**: Same API for V1 (Parquet) and V2 (Iceberg)
- **Standard protocol**: Arrow Flight is a standard, not custom code
- **No throwaway work**: V1 implementation directly evolves to V2

**Arrow Flight Schema (V1):**
```rust
// Arrow schema for financial series
let series_schema = Schema::new(vec![
    Field::new("id", DataType::Utf8, false),
    Field::new("source_id", DataType::Utf8, false),
    Field::new("external_id", DataType::Utf8, false),
    Field::new("title", DataType::Utf8, false),
    Field::new("description", DataType::Utf8, true),
    Field::new("frequency", DataType::Utf8, false),
    Field::new("is_active", DataType::Boolean, false),
    Field::new("created_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
]);

// Arrow schema for data points
let data_points_schema = Schema::new(vec![
    Field::new("id", DataType::Utf8, false),
    Field::new("series_id", DataType::Utf8, false),
    Field::new("date", DataType::Date32, false),
    Field::new("value", DataType::Float64, true),
    Field::new("revision_date", DataType::Date32, false),
    Field::new("is_original_release", DataType::Boolean, false),
]);
```

**V2 Iceberg Schema (Future):**
```sql
-- Same data, but managed by Iceberg
CREATE TABLE financial_series (
    id UUID,
    source_id UUID,
    external_id VARCHAR,
    title VARCHAR,
    description VARCHAR,
    frequency VARCHAR,
    is_active BOOLEAN,
    created_at TIMESTAMP
) USING ICEBERG;
```

### **Step 3: Crawler Write API**

**REST API for Crawlers:**
```rust
// Crawler write endpoints
POST /api/v1/financial-data/bulk
PUT /api/v1/financial-data/series/{id}
DELETE /api/v1/financial-data/series/{id}
```

**GraphQL Mutations for Crawlers:**
```graphql
type Mutation {
  bulkUploadFinancialData(data: [FinancialDataInput!]!): BulkUploadResult!
  updateFinancialSeries(id: ID!, data: FinancialDataInput!): FinancialSeries!
  deleteFinancialSeries(id: ID!): Boolean!
}
```

### **Step 4: GraphQL Read API**

**GraphQL Schema:**
```graphql
type Query {
  financialSeries(id: ID!): FinancialSeries
  financialSeriesList(filter: SeriesFilter): [FinancialSeries!]!
  financialDataPoints(seriesId: ID!, timeRange: TimeRange!): [DataPoint!]!
}

type FinancialSeries {
  id: ID!
  name: String!
  description: String
  dataPoints(timeRange: TimeRange!): [DataPoint!]!
  metadata: JSON
}

type DataPoint {
  timestamp: DateTime!
  value: Decimal!
  metadata: JSON
}
```

### **Step 5: Optional Federation Integration**

**Apollo Gateway Configuration:**
```typescript
// Gateway configuration
const gateway = new ApolloGateway({
  serviceList: [
    { name: 'monolithic-backend', url: 'http://localhost:8000/graphql' },
    { name: 'financial-data-service', url: 'http://localhost:8001/graphql' }
  ]
});
```

**Schema Composition:**
```graphql
# Monolithic backend schema (existing)
type User {
  id: ID!
  name: String!
  # ... existing fields
}

# Financial data service schema (new)
extend type User {
  financialSeries: [FinancialSeries!]!
}

type FinancialSeries {
  id: ID!
  name: String!
  # ... financial data fields
}
```

## Development Workflow

### **Local Development**
1. **Start Monolithic Backend**: Existing functionality unchanged
2. **Start Financial Data Service**: New service on different port
3. **Optional Gateway**: Test federation integration
4. **Crawler Testing**: Test write API with sample data

### **Testing Strategy**
1. **Unit Tests**: Test financial data service components
2. **Integration Tests**: Test Parquet/Iceberg/Arrow integration
3. **API Tests**: Test GraphQL and REST APIs
4. **Federation Tests**: Test optional federation integration

## Configuration

### **Environment Variables**
```bash
# Financial Data Service
FINANCIAL_DATA_SERVICE_PORT=8001
PARQUET_STORAGE_PATH=/data/parquet
ICEBERG_CATALOG_PATH=/data/iceberg
ARROW_MEMORY_LIMIT=1GB

# Federation (Optional)
GATEWAY_PORT=8002
MONOLITHIC_BACKEND_URL=http://localhost:8000/graphql
FINANCIAL_DATA_SERVICE_URL=http://localhost:8001/graphql
```

### **Docker Compose**
```yaml
version: '3.8'
services:
  monolithic-backend:
    # Existing backend configuration
    ports:
      - "8000:8000"
  
  financial-data-service:
    build: ./financial-data-service
    ports:
      - "8001:8001"
    volumes:
      - ./data/parquet:/data/parquet
      - ./data/iceberg:/data/iceberg
    environment:
      - PARQUET_STORAGE_PATH=/data/parquet
      - ICEBERG_CATALOG_PATH=/data/iceberg
  
  gateway:
    # Optional Apollo Gateway
    ports:
      - "8002:8002"
    environment:
      - MONOLITHIC_BACKEND_URL=http://monolithic-backend:8000/graphql
      - FINANCIAL_DATA_SERVICE_URL=http://financial-data-service:8001/graphql
```

## Success Criteria

### **V1 Success Metrics**
1. **Financial Data Service**: Operational with Parquet/Iceberg/Arrow
2. **Crawler Write API**: Functional for bulk data uploads
3. **GraphQL Read API**: Working for financial data queries
4. **Optional Federation**: Can integrate with existing backend
5. **Performance**: Time series database performs well
6. **Data Integrity**: Financial data is accurate and consistent

### **Validation Points**
1. **Data Storage**: Parquet storage is efficient and fast
2. **Query Performance**: Arrow processing is fast for analytics
3. **Schema Evolution**: Iceberg handles schema changes well
4. **Crawler Integration**: Write API handles bulk operations
5. **Federation**: Optional integration works smoothly

## Next Steps After V1

### **V2: Full Federation**
- Extract user service from monolithic backend
- Implement full Apollo Supergraph
- Add caching and performance optimizations

### **V3: Advanced Analytics**
- Add OData API for structured queries
- Implement complex analytical capabilities
- Add SQL-based query engines

## Clean Separation of Concerns

### **Problem with Current Schemas**
The existing schemas tightly couple crawler operational concerns with data models:
- `EconomicSeries` includes `last_crawled_at`, `crawl_status`, `crawl_error_message`
- Crawler job management mixed with data storage
- GraphQL API exposes crawler concerns to clients

### **V1 Solution: Clean Separation**

#### **Financial Data Service (Pure Data - Iceberg)**
```rust
// Clean data model - NO crawler fields, NO raw data
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
    // NO: last_crawled_at, crawl_status, crawl_error_message
    // NO: raw_data, validation_errors, retry_logic
}

// Clean data points - only validated data
pub struct DataPoint {
    pub id: Uuid,
    pub series_id: Uuid,
    pub date: NaiveDate,
    pub value: Option<BigDecimal>,
    pub revision_date: NaiveDate,
    pub is_original_release: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // NO: raw_data, validation_errors
}
```

#### **Crawler Service (Raw Data + Transformation)**
```rust
// Crawler service handles raw data and transformation
pub struct RawDataArchive {
    pub id: Uuid,
    pub batch_id: String,
    pub source: String,
    pub raw_data: serde_json::Value,  // Original payload
    pub validation_errors: Vec<ValidationError>,
    pub received_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub status: RawDataStatus,
}

// Separate crawler job management
pub struct CrawlerJob {
    pub id: Uuid,
    pub series_id: Uuid,  // Reference to financial data
    pub source: String,
    pub status: CrawlStatus,
    pub scheduled_for: DateTime<Utc>,
    pub retry_count: i32,
    pub error_message: Option<String>,
}
```

#### **Clean API Boundaries**
```graphql
# Financial Data Service - Pure Data API
type EconomicSeries {
  id: ID!
  title: String!
  description: String
  units: String
  frequency: String!
  # NO crawler fields exposed
}

# Crawler Write API - Bulk Operations
mutation BulkUpsertDataPoints($input: BulkDataPointsInput!) {
  bulkUpsertDataPoints(input: $input) {
    summary {
      totalProcessed
      successful
      failed
    }
  }
}
```

### **Benefits of Clean Separation**

1. **Independent Services**: Financial data and crawler services can scale independently
2. **Clean Federation**: Only data concerns in federated schema
3. **Better Testing**: Services can be tested in isolation
4. **Simplified Development**: Clear boundaries between concerns
5. **Future-Proof**: Easy to add new data sources or crawler types

### **Benefits of Dual Iceberg Architecture**

#### **Crawler Service Iceberg**
- **Raw Data Preservation**: Never lose incoming data, even with validation failures
- **Analytics on Raw Data**: Analyze patterns in validation failures
- **Time Travel**: Replay raw data processing with different validation rules
- **Schema Evolution**: Handle changing data formats from external sources
- **Audit Trail**: Complete history of all crawler operations

#### **Financial Data Service Iceberg**
- **Clean Data Storage**: Only validated, transformed data
- **Time Series Optimization**: Optimized for financial data queries
- **Analytics**: Fast analytical queries on clean financial data
- **Federation Ready**: Clean schema for GraphQL federation
- **Performance**: Optimized for read-heavy financial data workloads

#### **Two Write Endpoints**
- **Crawler → Crawler Iceberg**: Raw data, validation errors, job history
- **Crawler → Financial Service**: Clean, validated financial data
- **Independent Scaling**: Each service scales based on its workload
- **Clean Separation**: No mixing of raw data concerns with financial data

## Benefits of This Approach

1. **Low Risk**: Existing system unchanged
2. **Experimental**: Test time series database approach
3. **Incremental**: Build and validate step by step
4. **Optional**: Federation can be enabled/disabled
5. **Learning**: Understand performance characteristics
6. **Validation**: Prove the architecture works
7. **Clean Architecture**: Proper separation of concerns from the start

## Conclusion

This V1 implementation plan provides a safe, experimental approach to building the financial data service while keeping the existing system intact. It allows us to validate the time series database approach and test federation integration before committing to a full architectural change.
