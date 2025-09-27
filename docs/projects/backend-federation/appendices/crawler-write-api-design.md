# Crawler Write API Design

## Overview

The crawler write API provides a clean interface for bulk data ingestion into the financial data service. This API is designed to handle large-scale data updates from various crawler services while maintaining data integrity and performance.

## Design Principles

### 1. Clean Separation of Concerns
- **Crawler Service**: 
  - Raw data preservation and archiving
  - Data validation and transformation
  - Job scheduling, retry logic, error handling
  - **Owns raw data archive**
- **Financial Data Service**: 
  - Pure data storage and retrieval (Iceberg)
  - **Only receives clean, validated data**
  - **No raw data, no validation errors**
- **Write API**: Clean interface between services

### 2. Data Flow Architecture
```
Raw Data → Crawler Service → Clean Data → Financial Data Service
    ↓              ↓              ↓              ↓
[Iceberg Archive] [Validate/Transform] [Write API] [Iceberg Storage]
    ↓              ↓              ↓              ↓
[Raw Data Tables] [Clean Data]   [REST/GraphQL] [Financial Tables]
```

### 3. Dual Iceberg Architecture
- **Crawler Service Iceberg**: Raw data archive, validation logs, job history
- **Financial Data Service Iceberg**: Clean financial time series data
- **Two Write Endpoints**: 
  - Crawler → Crawler Iceberg (raw data)
  - Crawler → Financial Service (clean data)

### 3. Bulk Operations
- Support for large batch updates
- Efficient data serialization (Protobuf/Apache Arrow)
- Transactional consistency
- Error handling and partial success scenarios

### 4. Data Integrity
- **Crawler Service**: Schema validation and raw data preservation
- **Financial Data Service**: Clean data storage and retrieval
- Duplicate detection and handling
- Revision tracking and conflict resolution
- Audit trails for data changes
- **Never drop data on the floor** - crawler preserves raw input

## API Design

### 1. Series Management API

#### Create/Update Series
```graphql
# GraphQL Mutation
mutation UpsertSeries($input: SeriesUpsertInput!) {
  upsertSeries(input: $input) {
    series {
      id
      externalId
      title
      isActive
    }
    success
    errors
  }
}

# Input Type
input SeriesUpsertInput {
  sourceId: ID!
  externalId: String!
  title: String!
  description: String
  units: String
  frequency: String!
  seasonalAdjustment: String
  startDate: Date
  endDate: Date
  isActive: Boolean = true
}

# Response Type
type SeriesUpsertResponse {
  series: EconomicSeries
  success: Boolean!
  errors: [String!]!
}
```

#### Bulk Series Operations
```graphql
# Bulk series creation/update
mutation BulkUpsertSeries($input: BulkSeriesUpsertInput!) {
  bulkUpsertSeries(input: $input) {
    results {
      seriesId
      success
      errors
    }
    summary {
      totalProcessed
      successful
      failed
    }
  }
}

input BulkSeriesUpsertInput {
  series: [SeriesUpsertInput!]!
  batchId: String  # For tracking and idempotency
  source: String!   # Crawler source identifier
}
```

### 2. Data Points API

#### Single Data Point
```graphql
# Create/Update single data point
mutation UpsertDataPoint($input: DataPointUpsertInput!) {
  upsertDataPoint(input: $input) {
    dataPoint {
      id
      seriesId
      date
      value
      revisionDate
    }
    success
    errors
  }
}

input DataPointUpsertInput {
  seriesId: ID!
  date: Date!
  value: Decimal
  revisionDate: Date!
  isOriginalRelease: Boolean = true
}
```

#### Bulk Data Points
```graphql
# Bulk data point operations
mutation BulkUpsertDataPoints($input: BulkDataPointsInput!) {
  bulkUpsertDataPoints(input: $input) {
    results {
      dataPointId
      success
      errors
    }
    summary {
      totalProcessed
      successful
      failed
      newDataPoints
      updatedDataPoints
    }
  }
}

input BulkDataPointsInput {
  dataPoints: [DataPointUpsertInput!]!
  batchId: String
  source: String!
  seriesId: ID!  # All data points must belong to same series
}
```

### 3. Binary Serialization API

#### Protobuf Endpoint
```http
POST /api/v1/crawler/bulk-data
Content-Type: application/x-protobuf
X-Data-Format: protobuf
X-Batch-ID: {batch_id}
X-Source: {crawler_source}

# Protobuf message containing:
# - Series metadata
# - Data points array
# - Batch metadata
```

#### Apache Arrow Endpoint
```http
POST /api/v1/crawler/bulk-data
Content-Type: application/x-apache-arrow
X-Data-Format: arrow
X-Batch-ID: {batch_id}
X-Source: {crawler_source}

# Arrow table with columns:
# - series_id
# - date
# - value
# - revision_date
# - is_original_release
```

### 4. REST API for Simple Operations

#### Series Management
```http
# Create/Update series
PUT /api/v1/series/{source_id}/{external_id}
Content-Type: application/json

{
  "title": "Real Gross Domestic Product",
  "description": "Real GDP in billions of chained 2017 dollars",
  "units": "Billions of Chained 2017 Dollars",
  "frequency": "Quarterly",
  "seasonalAdjustment": "Seasonally Adjusted Annual Rate",
  "startDate": "1947-01-01",
  "endDate": null,
  "isActive": true
}
```

#### Data Points Batch
```http
# Bulk data points for a series
POST /api/v1/series/{series_id}/data-points
Content-Type: application/json

{
  "batchId": "fred-gdp-2024-01-15",
  "source": "fred-crawler",
  "dataPoints": [
    {
      "date": "2024-01-01",
      "value": "27360.0",
      "revisionDate": "2024-01-15",
      "isOriginalRelease": true
    },
    {
      "date": "2024-02-01", 
      "value": "27420.0",
      "revisionDate": "2024-02-15",
      "isOriginalRelease": true
    }
  ]
}
```

## Data Models

### 1. Series Upsert Model
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SeriesUpsertInput {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesUpsertResponse {
    pub series: Option<EconomicSeries>,
    pub success: bool,
    pub errors: Vec<String>,
}
```

### 2. Data Point Upsert Model
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DataPointUpsertInput {
    pub series_id: Uuid,
    pub date: NaiveDate,
    pub value: Option<BigDecimal>,
    pub revision_date: NaiveDate,
    pub is_original_release: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPointUpsertResponse {
    pub data_point: Option<DataPoint>,
    pub success: bool,
    pub errors: Vec<String>,
}
```

### 3. Bulk Operations Model
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkSeriesUpsertInput {
    pub series: Vec<SeriesUpsertInput>,
    pub batch_id: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkDataPointsInput {
    pub data_points: Vec<DataPointUpsertInput>,
    pub batch_id: Option<String>,
    pub source: String,
    pub series_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkOperationResult {
    pub results: Vec<OperationResult>,
    pub summary: OperationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub id: Option<Uuid>,
    pub success: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationSummary {
    pub total_processed: usize,
    pub successful: usize,
    pub failed: usize,
    pub new_records: Option<usize>,
    pub updated_records: Option<usize>,
}
```

## Protobuf Schema

### 1. Series Message
```protobuf
syntax = "proto3";

message SeriesData {
  string source_id = 1;
  string external_id = 2;
  string title = 3;
  optional string description = 4;
  optional string units = 5;
  string frequency = 6;
  optional string seasonal_adjustment = 7;
  optional string start_date = 8;
  optional string end_date = 9;
  bool is_active = 10;
}
```

### 2. Data Point Message
```protobuf
message DataPointData {
  string series_id = 1;
  string date = 2;
  optional double value = 3;
  string revision_date = 4;
  bool is_original_release = 5;
}
```

### 3. Bulk Data Message
```protobuf
message BulkDataRequest {
  string batch_id = 1;
  string source = 2;
  repeated SeriesData series = 3;
  repeated DataPointData data_points = 4;
  int64 timestamp = 5;
}
```

## Apache Arrow Schema

### 1. Series Arrow Table
```rust
// Arrow schema for series data
let series_schema = Schema::new(vec![
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
]);
```

### 2. Data Points Arrow Table
```rust
// Arrow schema for data points
let data_points_schema = Schema::new(vec![
    Field::new("series_id", DataType::Utf8, false),
    Field::new("date", DataType::Utf8, false),
    Field::new("value", DataType::Float64, true),
    Field::new("revision_date", DataType::Utf8, false),
    Field::new("is_original_release", DataType::Boolean, false),
]);
```

## Raw Data Preservation Strategy

### **Problem: Never Lose Data**
When schema validation fails, we must preserve the raw incoming data for:
- **Debugging**: Understand why validation failed
- **Recovery**: Fix schema issues and reprocess data
- **Audit**: Track what data was received vs. processed
- **Analysis**: Identify patterns in validation failures

### **Solution: Raw Data Archive (Crawler Service Only)**

#### **Crawler Service Raw Data Archive**
```rust
// Raw data preservation in crawler service - NOT in financial data service
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

pub enum RawDataStatus {
    Received,           // Just received, not yet processed
    ValidationFailed,   // Schema validation failed
    ProcessingFailed,  // Processing failed after validation
    Processed,         // Successfully processed
    Archived,         // Archived for long-term storage
}
```

#### **Dual Iceberg Architecture**

##### **Crawler Service Iceberg Tables**
```sql
-- Raw data archive using Iceberg for crawler service
CREATE TABLE crawler_raw_data_archive (
    id STRING,
    batch_id STRING,
    source STRING,
    raw_data STRING,  -- JSON payload
    validation_errors STRING,  -- JSON array of errors
    received_at TIMESTAMP,
    processed_at TIMESTAMP,
    retry_count INT,
    status STRING,
    created_at TIMESTAMP
) USING ICEBERG;

-- Crawler job history
CREATE TABLE crawler_job_history (
    id STRING,
    series_id STRING,
    source STRING,
    status STRING,
    scheduled_for TIMESTAMP,
    attempted_at TIMESTAMP,
    completed_at TIMESTAMP,
    retry_count INT,
    error_message STRING,
    data_points_processed INT,
    created_at TIMESTAMP
) USING ICEBERG;

-- Validation error patterns for analytics
CREATE TABLE crawler_validation_patterns (
    source STRING,
    error_type STRING,
    field_name STRING,
    error_count INT,
    first_seen TIMESTAMP,
    last_seen TIMESTAMP,
    sample_data STRING
) USING ICEBERG;
```

##### **Financial Data Service Iceberg Tables**
```sql
-- Clean financial time series data
CREATE TABLE financial_series (
    id STRING,
    source_id STRING,
    external_id STRING,
    title STRING,
    description STRING,
    units STRING,
    frequency STRING,
    seasonal_adjustment STRING,
    start_date DATE,
    end_date DATE,
    is_active BOOLEAN,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
) USING ICEBERG;

-- Clean data points
CREATE TABLE financial_data_points (
    id STRING,
    series_id STRING,
    date DATE,
    value DECIMAL(20,6),
    revision_date DATE,
    is_original_release BOOLEAN,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
) USING ICEBERG;
```

#### **Two Write Endpoints Architecture**

##### **Endpoint 1: Crawler Raw Data Archive**
```rust
// Crawler service writes raw data to its own Iceberg
pub struct CrawlerRawDataWriter {
    iceberg_client: IcebergClient,
    table_name: String,  // "crawler_raw_data_archive"
}

impl CrawlerRawDataWriter {
    pub async fn write_raw_data(&self, raw_data: RawDataArchive) -> Result<()> {
        // Write to crawler Iceberg table
        // Includes validation errors, retry logic, etc.
    }
    
    pub async fn write_job_history(&self, job: CrawlerJob) -> Result<()> {
        // Write job execution history
    }
}
```

##### **Endpoint 2: Financial Data Service**
```rust
// Crawler service writes clean data to financial service
pub struct FinancialDataWriter {
    financial_service_client: FinancialServiceClient,
}

impl FinancialDataWriter {
    pub async fn write_clean_series(&self, series: EconomicSeries) -> Result<()> {
        // Write clean series data via REST/GraphQL API
    }
    
    pub async fn write_clean_data_points(&self, data_points: Vec<DataPoint>) -> Result<()> {
        // Write clean data points via bulk API
    }
}
```

### **Clean Separation: Financial Data Service Only Gets Clean Data**

#### **Financial Data Service (Iceberg) - Clean Data Only**
```rust
// Financial data service only receives clean, validated data
pub struct CleanFinancialData {
    pub series_id: Uuid,
    pub date: NaiveDate,
    pub value: BigDecimal,
    pub revision_date: NaiveDate,
    pub is_original_release: bool,
    // NO raw data, NO validation errors, NO retry logic
}
```

#### **Crawler Service - Raw Data + Transformation**
```rust
// Crawler service handles raw data and transformation
pub struct CrawlerDataProcessor {
    pub raw_data_archive: RawDataArchive,
    pub transformation_rules: TransformationRules,
    pub validation_schema: ValidationSchema,
}

impl CrawlerDataProcessor {
    // 1. Store raw data first
    pub async fn store_raw_data(&self, raw_data: serde_json::Value) -> Result<Uuid> {
        // Store in raw_data_archive table
    }
    
    // 2. Validate and transform
    pub async fn validate_and_transform(&self, raw_data_id: Uuid) -> Result<CleanFinancialData> {
        // Validate against schema
        // Transform to clean format
        // Return clean data for financial service
    }
    
    // 3. Send clean data to financial service
    pub async fn send_to_financial_service(&self, clean_data: CleanFinancialData) -> Result<()> {
        // Send via write API to financial data service
    }
}
```

### **Error Handling with Raw Data Preservation**

#### **1. Validation Errors with Raw Data**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: String,
    pub value: serde_json::Value,  // Original value that failed
    pub error_type: ValidationErrorType,
    pub message: String,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationErrorType {
    InvalidDateFormat,
    InvalidNumericValue,
    MissingRequiredField,
    InvalidSeriesReference,
    DuplicateDataPoint,
    SchemaMismatch,
    DataTypeMismatch,
}
```

#### **2. Processing Errors with Raw Data**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingError {
    pub error_type: ProcessingErrorType,
    pub message: String,
    pub raw_data_id: Option<Uuid>,  // Reference to preserved raw data
    pub retry_after: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingErrorType {
    DatabaseError,
    SerializationError,
    BatchSizeExceeded,
    RateLimitExceeded,
    ServiceUnavailable,
    SchemaValidationFailed,
}
```

#### **3. Enhanced Bulk Operation Response**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkOperationResponse {
    pub batch_id: String,
    pub total_processed: usize,
    pub successful: usize,
    pub failed: usize,
    pub raw_data_archived: usize,  // How many raw records preserved
    pub errors: Vec<ProcessingError>,
    pub results: Vec<OperationResult>,
    pub raw_data_ids: Vec<Uuid>,  // IDs of preserved raw data
}
```

### **Raw Data Recovery Workflow**

#### **1. Failed Validation Recovery**
```rust
// API to reprocess failed raw data
POST /api/v1/crawler/raw-data/{raw_data_id}/reprocess
PUT /api/v1/crawler/raw-data/{raw_data_id}/fix-and-reprocess

// Fix validation issues and reprocess
pub struct RawDataFix {
    pub raw_data_id: Uuid,
    pub fixes: Vec<DataFix>,
    pub reprocess: bool,
}

pub struct DataFix {
    pub field: String,
    pub original_value: serde_json::Value,
    pub fixed_value: serde_json::Value,
    pub fix_reason: String,
}
```

#### **2. Raw Data Query API**
```rust
// Query raw data archive
GET /api/v1/crawler/raw-data?status=validation_failed&source=fred
GET /api/v1/crawler/raw-data/{batch_id}
GET /api/v1/crawler/raw-data/{raw_data_id}

// Raw data analysis
GET /api/v1/crawler/raw-data/analysis?source=fred&date_range=2024-01-01,2024-01-31
```

### **Implementation Strategy**

#### **Phase 1: Basic Raw Data Preservation**
1. **Store All Incoming Data**: Before any validation
2. **Link Errors to Raw Data**: Reference raw data in error responses
3. **Basic Recovery**: Manual reprocessing of failed data

#### **Phase 2: Advanced Error Handling**
1. **Automatic Retry**: Retry failed validations with backoff
2. **Data Fixing**: API to fix validation issues
3. **Batch Recovery**: Reprocess entire failed batches

#### **Phase 3: Analytics and Monitoring**
1. **Error Pattern Analysis**: Identify common validation failures
2. **Schema Evolution**: Update schemas based on real data patterns
3. **Proactive Monitoring**: Alert on unusual validation failure rates

### **Raw Data Archive Management**

#### **Retention Policy**
```rust
pub struct RawDataRetentionPolicy {
    pub validation_failed_retention_days: i32,  // Keep failed data longer
    pub processed_retention_days: i32,          // Keep processed data shorter
    pub archived_retention_days: i32,           // Long-term archive
    pub compression_enabled: bool,              // Compress old data
}
```

#### **Cleanup Strategy**
```rust
// Automated cleanup of old raw data
pub struct RawDataCleanup {
    pub archive_old_data: bool,
    pub compress_archived_data: bool,
    pub delete_very_old_data: bool,
    pub retention_policy: RawDataRetentionPolicy,
}
```

## Error Handling

### 1. Validation Errors
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationError {
    InvalidDateFormat(String),
    InvalidNumericValue(String),
    MissingRequiredField(String),
    InvalidSeriesReference(String),
    DuplicateDataPoint(String),
}
```

### 2. Processing Errors
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingError {
    DatabaseError(String),
    SerializationError(String),
    BatchSizeExceeded(usize),
    RateLimitExceeded,
    ServiceUnavailable,
}
```

### 3. Partial Success Handling
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkOperationResponse {
    pub batch_id: String,
    pub total_processed: usize,
    pub successful: usize,
    pub failed: usize,
    pub errors: Vec<ProcessingError>,
    pub results: Vec<OperationResult>,
}
```

## Performance Considerations

### 1. Batch Size Limits
- Maximum 10,000 data points per batch
- Maximum 1,000 series per batch
- Configurable limits per source

### 2. Rate Limiting
- 100 requests per minute per source
- 1,000 data points per minute per source
- Burst allowance for large batches

### 3. Caching Strategy
- Series metadata caching
- Recent data points caching
- Batch processing status caching

## Security Considerations

### 1. Authentication
- API key authentication for crawler services
- Source-specific access controls
- Rate limiting per source

### 2. Data Validation
- Schema validation for all inputs
- Sanitization of string fields
- Numeric range validation

### 3. Audit Logging
- All write operations logged
- Source tracking for all changes
- Error logging and monitoring

## Benefits of Dual Iceberg Architecture

### **1. Crawler Service Iceberg Benefits**
- **Raw Data Preservation**: Never lose incoming data, even with validation failures
- **Analytics on Raw Data**: Analyze patterns in validation failures
- **Time Travel**: Replay raw data processing with different validation rules
- **Schema Evolution**: Handle changing data formats from external sources
- **Audit Trail**: Complete history of all crawler operations
- **Performance**: Iceberg's columnar storage for large raw data volumes

### **2. Financial Data Service Iceberg Benefits**
- **Clean Data Storage**: Only validated, transformed data
- **Time Series Optimization**: Optimized for financial data queries
- **Analytics**: Fast analytical queries on clean financial data
- **Federation Ready**: Clean schema for GraphQL federation
- **Performance**: Optimized for read-heavy financial data workloads

### **3. Operational Benefits**
- **Independent Scaling**: Each service scales based on its workload
- **Independent Schema Evolution**: Raw data schema vs. financial data schema
- **Clean Separation**: No mixing of concerns
- **Better Monitoring**: Separate metrics for crawler vs. financial operations
- **Recovery**: Can replay raw data processing without affecting financial data

### **4. Development Benefits**
- **Clear Boundaries**: Each service has distinct responsibilities
- **Independent Testing**: Test crawler logic separately from financial data
- **Schema Flexibility**: Raw data schema can be more flexible than financial schema
- **Debugging**: Easy to trace issues from raw data to clean data

## Implementation Plan

### Phase 1: Basic REST API
1. Implement series upsert endpoints
2. Implement data points batch endpoints
3. Add basic validation and error handling

### Phase 2: GraphQL Integration
1. Add GraphQL mutations for bulk operations
2. Implement DataLoader for efficient queries
3. Add subscription support for real-time updates

### Phase 3: Binary Serialization
1. Implement Protobuf endpoints
2. Implement Apache Arrow endpoints
3. Add compression and optimization

### Phase 4: Advanced Features
1. Add idempotency support
2. Implement conflict resolution
3. Add monitoring and metrics

### Phase 5: Dual Iceberg Implementation
1. Set up crawler service Iceberg for raw data
2. Set up financial service Iceberg for clean data
3. Implement two-write-endpoint architecture
4. Add cross-service analytics and monitoring
