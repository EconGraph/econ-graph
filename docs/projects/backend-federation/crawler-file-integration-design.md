# Crawler File Integration Design (Phase 2.5.2 & 2.5.3)

**Date**: September 26, 2025  
**Status**: Design Phase  
**Branch**: `backend-federation-crawler-integration`

## Overview

This document outlines the detailed design for implementing crawler file integration capabilities in the financial data service. This phase focuses on enabling crawlers to create Parquet files directly and providing comprehensive data discovery APIs.

## Current State Analysis

### ✅ **Already Implemented**
- **GraphQL Data Discovery API**: We already have comprehensive data discovery queries:
  - `catalog_stats` - Overall catalog statistics
  - `find_series_by_date_range` - Find series with data in specific date ranges
  - `find_series_by_external_id` - Find series by external identifier
  - `data_freshness` - Check data staleness and last update times
  - `list_series` - List all available series
- **Iceberg Catalog Foundation**: Placeholder `IcebergCatalog` with full metadata management
- **Data Models**: Complete `EconomicSeries`, `DataPoint`, and `DataSource` models
- **Validation System**: Comprehensive input validation and sanitization
- **Storage Abstraction**: `IcebergStorage` with custom partitioning support

### 🔄 **What We Need to Implement**

## Phase 2.5.2: Crawler File Creation

### **Goal**
Enable crawlers to create Parquet files directly in the financial data service's storage directory with proper metadata registration.

### **Current Gaps**
1. **No Direct File Creation API**: Crawlers currently use the GraphQL mutation API, not direct file creation
2. **No File Validation Pipeline**: No validation of Parquet files created by crawlers
3. **No File Discovery System**: No way to discover and catalog new files
4. **No File Integrity Checks**: No verification of file format and data integrity

### **Detailed Implementation Plan**

#### **2.5.2.1 Crawler Bulk Upload API**

**New GraphQL Mutation**: `submitDataBatch`
```graphql
mutation SubmitDataBatch($input: DataBatchInput!) {
  submitDataBatch(input: $input) {
    batchId
    status
    seriesProcessed
    totalDataPoints
    errors
    warnings
    processingTimeMs
  }
}

input DataBatchInput {
  batchId: String!           # Crawler-generated batch identifier
  source: String!            # Data source identifier
  files: [DataFileInput!]!   # Multiple files in this batch
  metadata: BatchMetadataInput
}

input DataFileInput {
  externalId: String!        # Series external identifier
  seriesMetadata: SeriesMetadataInput!
  data: DataInput!           # Arrow data or file path
}

input DataInput {
  format: DataFormat!        # Data format
  # Either inline Arrow data or file path
  arrowData: String          # Base64-encoded Arrow RecordBatch
  filePath: String           # Path to data file (Parquet or Arrow)
  # For streaming Arrow data
  arrowStreamData: String    # Base64-encoded Arrow Stream data
}

enum DataFormat {
  ARROW_RECORD_BATCH        # Single Arrow RecordBatch (inline)
  ARROW_STREAM              # Arrow Stream format (inline)
  PARQUET_FILE              # Parquet file (file path)
  ARROW_FILE                # Arrow file (file path)
}

input SeriesMetadataInput {
  title: String!
  description: String
  units: String
  frequency: String!
  seasonalAdjustment: String
  expectedDataPoints: Int
  startDate: Date
  endDate: Date
}

input BatchMetadataInput {
  crawlerVersion: String
  processingTimestamp: DateTime
  sourceApiVersion: String
  batchSize: Int
  compressionType: String
}
```

**Implementation Details**:
- Process multiple series in a single batch operation
- Support both inline Arrow data and file-based uploads
- Validate Arrow RecordBatch schemas for inline data
- Perform minimal schema compatibility checks
- Register all series in the Iceberg catalog atomically
- Convert inline Arrow data to Parquet files for storage
- Provide batch-level success/failure reporting
- Support partial batch success (some series succeed, others fail)

**Benefits of Arrow Format Support**:
- **Zero-Copy**: Direct Arrow RecordBatch processing without file I/O
- **Efficiency**: No need to write temporary files for small datasets
- **Streaming**: Support for Arrow Stream format for large datasets
- **Flexibility**: Crawlers can choose between inline data or file uploads
- **Performance**: Faster processing for real-time data ingestion

#### **2.5.2.2 File Naming Conventions & Organization**

**Directory Structure**:
```
data/
├── series/
│   ├── year=2024/
│   │   ├── month=01/
│   │   │   ├── day=15/
│   │   │   │   ├── series_<uuid>_<external_id>.parquet
│   │   │   │   └── series_<uuid>_<external_id>.metadata.json
│   │   │   └── day=16/
│   │   └── month=02/
│   └── year=2025/
└── catalog/
    ├── series_index.json
    └── file_registry.json
```

**File Naming Convention**:
- **Data Files**: `series_{uuid}_{external_id}.parquet`
- **Metadata Files**: `series_{uuid}_{external_id}.metadata.json`
- **Example**: `series_123e4567-e89b-12d3-a456-426614174000_GDP_Q1_2024.parquet`

#### **2.5.2.3 Minimal File Validation**

**Basic Schema Validation** (Crawler handles data validation):
1. **File Format Check**:
   - Verify Parquet file can be opened and read
   - Check required columns exist (date, value, series_id)
   - Validate basic data types (date is date, value is numeric)

2. **Minimal Schema Compatibility**:
   - Ensure Parquet schema has expected column names
   - Verify basic data types match our `DataPoint` model
   - Reject files with completely incompatible schemas

3. **Basic Metadata Validation**:
   - Validate external ID format (alphanumeric with underscores/hyphens)
   - Check frequency is one of our supported values
   - Verify source information is provided

**Error Handling**:
- Return basic validation errors for schema mismatches
- Trust crawler for data quality validation
- Focus on system-level compatibility, not data quality

#### **2.5.2.4 File Discovery & Cataloging**

**Automatic File Discovery**:
- Scan data directory for new Parquet files
- Parse metadata files for series information
- Register new files in the catalog
- Update series statistics and date ranges

**File Registry**:
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct FileRegistry {
    pub file_id: Uuid,
    pub series_id: Uuid,
    pub file_path: PathBuf,
    pub file_size: u64,
    pub created_at: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub data_points: u64,
    pub date_range: (NaiveDate, NaiveDate),
    pub checksum: String,
    pub status: FileStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FileStatus {
    Pending,      // File discovered but not yet processed
    Validating,   // Currently being validated
    Valid,        // Validation passed
    Invalid,      // Validation failed
    Processed,    // Successfully integrated into catalog
    Error,        // Processing failed
}
```

## Phase 2.5.3: Enhanced Data Discovery API

### **Goal**
Provide comprehensive data discovery capabilities for crawlers and users to understand what data is available.

### **Current Gaps**
1. **Limited Series Information**: Current `SeriesInfo` has placeholder values for data point counts
2. **No Data Quality Metrics**: No completeness or quality indicators
3. **No File-Level Discovery**: Can't query which files contain specific data
4. **No Data Coverage Analysis**: No way to understand data gaps or overlaps

### **Detailed Implementation Plan**

#### **2.5.3.1 Enhanced Series Information**

**Enhanced `SeriesInfo` Response**:
```rust
#[derive(async_graphql::SimpleObject)]
pub struct EnhancedSeriesInfo {
    // Basic Information
    pub id: Uuid,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub frequency: String,
    pub seasonal_adjustment: Option<String>,
    pub source: String,
    
    // Data Coverage
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub total_data_points: u64,
    pub expected_data_points: Option<u64>,
    
    // Gap Tracking (TCP Selective ACK Model)
    pub known_gaps: Vec<DataGap>,
    pub gap_count: u32,
    
    // File Information
    pub files: Vec<FileInfo>,
    pub total_file_size: u64,
    
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub last_data_point: Option<DateTime<Utc>>,
}

#[derive(async_graphql::SimpleObject)]
pub struct DataGap {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub gap_type: GapType,
    pub gap_status: GapStatus,
    pub expected_count: u32,    // How many data points should be here
    pub actual_count: u32,      // How many we actually have (0 for missing)
    pub first_reported: DateTime<Utc>,
    pub last_checked: DateTime<Utc>,
}

#[derive(async_graphql::Enum)]
pub enum GapType {
    MissingData,      // Expected data points are missing (normal)
    DuplicateData,    // Same date appears multiple times (error)
    InvalidDate,      // Date is outside expected range (error)
}

#[derive(async_graphql::Enum)]
pub enum GapStatus {
    KnownMissing,     // We know this data is missing (like TCP selective ACK)
    DataError,        // This is an actual data error (duplicate, invalid)
    PendingRetry,     // We're going to try to get this data again
    PermanentlyMissing, // Source confirmed this data doesn't exist
    RecentlyFilled,   // This gap was recently filled by new data
}

#[derive(async_graphql::SimpleObject)]
pub struct FileInfo {
    pub file_id: Uuid,
    pub file_path: String,
    pub file_size: u64,
    pub data_points: u64,
    pub date_range: (NaiveDate, NaiveDate),
    pub created_at: DateTime<Utc>,
    pub status: String,
}
```

#### **2.5.3.2 Advanced Discovery Queries**

**New GraphQL Queries**:

1. **Data Coverage Analysis**:
```graphql
query DataCoverage($seriesId: UUID!, $startDate: Date, $endDate: Date) {
  dataCoverage(seriesId: $seriesId, startDate: $startDate, endDate: $endDate) {
    totalExpected
    totalActual
    knownGaps {
      startDate
      endDate
      gapType
      gapStatus
      expectedCount
      actualCount
      firstReported
    }
  }
}
```

2. **File Discovery**:
```graphql
query FindFiles($seriesId: UUID, $dateRange: DateRange, $status: FileStatus) {
  findFiles(seriesId: $seriesId, dateRange: $dateRange, status: $status) {
    fileId
    seriesId
    filePath
    dataPoints
    dateRange
    fileSize
    status
    createdAt
  }
}
```

3. **Gap Summary**:
```graphql
query GapSummary($seriesIds: [UUID!]) {
  gapSummary(seriesIds: $seriesIds) {
    totalSeries
    totalKnownGaps
    seriesWithGaps
    seriesWithErrors
    gapStatusDistribution {
      knownMissing
      dataError
      pendingRetry
      permanentlyMissing
      recentlyFilled
    }
  }
}
```

#### **2.5.3.3 Data Availability Monitoring**

**Critical Data Availability Tracking**:
```rust
pub struct DataAvailabilityMetrics {
    pub data_availability_status: DataAvailabilityStatus,
    pub last_successful_update: Option<DateTime<Utc>>,
    pub expected_update_frequency: Duration,
    pub time_since_last_update: Duration,
    pub is_stale: bool,                    // Data is older than expected
    pub is_critical_missing: bool,         // Critical data is missing
    pub requires_immediate_action: bool,   // Must be addressed ASAP
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DataAvailabilityStatus {
    Current,           // Data is up-to-date
    Stale,            // Data is older than expected but not critical
    Missing,          // Expected data is missing
    Critical,         // Critical data is missing - immediate action required
    Error,            // Data source error - immediate action required
}

impl DataAvailabilityMetrics {
    pub fn calculate(
        series: &SeriesMetadata, 
        last_data_point: Option<DateTime<Utc>>,
        expected_frequency: &Frequency,
    ) -> Self {
        let now = Utc::now();
        let time_since_last = last_data_point
            .map(|last| now.signed_duration_since(last))
            .unwrap_or(Duration::days(365)); // If no data, consider it very stale
        
        let expected_interval = expected_frequency.expected_update_interval();
        let is_stale = time_since_last > expected_interval * 2; // 2x expected interval
        let is_critical = time_since_last > expected_interval * 7; // 1 week past expected
        
        let status = if is_critical {
            DataAvailabilityStatus::Critical
        } else if is_stale {
            DataAvailabilityStatus::Stale
        } else {
            DataAvailabilityStatus::Current
        };
        
        Self {
            data_availability_status: status,
            last_successful_update: last_data_point,
            expected_update_frequency: expected_interval,
            time_since_last_update: time_since_last,
            is_stale,
            is_critical_missing: is_critical,
            requires_immediate_action: is_critical,
        }
    }
}
```

**Alert System for Missing Data**:
```rust
pub struct DataAvailabilityAlert {
    pub alert_id: Uuid,
    pub series_id: Uuid,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub created_at: DateTime<Utc>,
    pub acknowledged: bool,
    pub resolved: bool,
    pub resolution_notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AlertType {
    DataMissing,        // Expected data is missing
    DataStale,         // Data is older than expected
    SourceError,       // Data source is reporting errors
    CriticalGap,       // Critical time period has no data
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,              // Data is slightly delayed
    Warning,           // Data is stale but not critical
    Critical,          // Critical data is missing - immediate action required
    Emergency,         // System-level data availability issue
}
```

#### **2.5.3.4 Data Gap Detection (TCP Selective ACK Model)**

**Philosophy**: Data gaps are acceptable as long as we maintain precise tracking of what's missing. Similar to TCP selective ACK, we can receive future chunks with missing data in the middle, but we need to know exactly what's up with that.

**Gap Detection Algorithm**:
```rust
pub fn detect_gaps(
    series: &SeriesMetadata,
    data_points: &[DataPoint],
    expected_frequency: &Frequency,
) -> Vec<DataGap> {
    let mut gaps = Vec::new();
    
    // Sort data points by date
    let mut sorted_points = data_points.to_vec();
    sorted_points.sort_by_key(|dp| dp.date);
    
    // Build a complete expected timeline
    let expected_timeline = build_expected_timeline(
        series.start_date, 
        series.end_date, 
        expected_frequency
    );
    
    // Find gaps between expected and actual data
    for window in sorted_points.windows(2) {
        let gap_start = window[0].date + expected_frequency.days_between_points();
        let gap_end = window[1].date - expected_frequency.days_between_points();
        
        // If there's a gap, create a precise gap record
        if gap_start <= gap_end {
            gaps.push(DataGap {
                start_date: gap_start,
                end_date: gap_end,
                gap_type: GapType::MissingData,
                severity: GapSeverity::Tracked, // Not an error, just tracked
                expected_count: count_expected_points(gap_start, gap_end, expected_frequency),
                actual_count: 0,
                gap_status: GapStatus::KnownMissing,
            });
        }
    }
    
    // Check for duplicate dates (these are actual errors)
    let mut seen_dates = HashSet::new();
    for point in data_points {
        if !seen_dates.insert(point.date) {
            gaps.push(DataGap {
                start_date: point.date,
                end_date: point.date,
                gap_type: GapType::DuplicateData,
                severity: GapSeverity::High, // This is an actual error
                expected_count: 1,
                actual_count: 2, // Duplicate found
                gap_status: GapStatus::DataError,
            });
        }
    }
    
    gaps
}

/// Build expected timeline for gap detection
fn build_expected_timeline(
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>, 
    frequency: &Frequency
) -> Vec<NaiveDate> {
    // Implementation depends on frequency type
    // This creates the "expected" data points timeline
    todo!("Implement timeline generation based on frequency")
}
```

**Enhanced Gap Tracking**:
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct DataGap {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub gap_type: GapType,
    pub severity: GapSeverity,
    pub expected_count: u32,    // How many data points should be here
    pub actual_count: u32,      // How many we actually have
    pub gap_status: GapStatus,  // Current status of this gap
    pub first_reported: DateTime<Utc>,  // When we first detected this gap
    pub last_checked: DateTime<Utc>,    // When we last checked this gap
    pub resolution_attempts: u32,       // How many times we've tried to fill this gap
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GapStatus {
    KnownMissing,     // We know this data is missing (like TCP selective ACK)
    DataError,        // This is an actual data error (duplicate, invalid)
    PendingRetry,     // We're going to try to get this data again
    PermanentlyMissing, // Source confirmed this data doesn't exist
    RecentlyFilled,   // This gap was recently filled by new data
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GapSeverity {
    Tracked,          // Normal gap tracking (not an error)
    Low,             // Minor issues
    Medium,          // Noticeable issues  
    High,            // Significant data problems
    Critical,        // Data integrity issues
}
```

**Gap Resolution Tracking**:
```rust
/// Track attempts to resolve gaps (like TCP retry logic)
pub struct GapResolutionTracker {
    pub gap_id: Uuid,
    pub series_id: Uuid,
    pub gap: DataGap,
    pub retry_schedule: Vec<DateTime<Utc>>,  // When to retry
    pub retry_count: u32,
    pub max_retries: u32,
    pub last_retry_result: Option<RetryResult>,
}

#[derive(Debug)]
pub enum RetryResult {
    Success,           // Gap was filled
    PartialSuccess,    // Some data was retrieved
    Failed,           // No data available
    SourceError,      // Source had an error
    DataInvalid,      // Retrieved data was invalid
}
```

## Implementation Strategy

### **Phase 1: File Creation Foundation (Week 1)**
1. Implement `createSeriesFromFile` GraphQL mutation
2. Add file validation pipeline
3. Implement file naming conventions and directory structure
4. Add basic file registry

### **Phase 2: Enhanced Discovery (Week 2)**
1. Enhance `SeriesInfo` with quality metrics
2. Implement data coverage analysis
3. Add gap detection algorithms
4. Create advanced discovery queries

### **Phase 3: Quality Metrics (Week 3)**
1. Implement comprehensive quality scoring
2. Add outlier detection
3. Create data quality summary APIs
4. Add file-level discovery capabilities

### **Testing Strategy**
1. **Unit Tests**: File validation, quality metrics, gap detection
2. **Integration Tests**: End-to-end file creation and discovery
3. **Performance Tests**: Large file processing and catalog queries
4. **Concurrent Tests**: Multiple crawlers creating files simultaneously

## Success Criteria

### **Functional Requirements**
- [ ] Crawlers can create Parquet files directly via GraphQL API
- [ ] Files are automatically validated and cataloged
- [ ] Comprehensive data discovery APIs are available
- [ ] Data quality metrics are calculated and exposed

### **Performance Requirements**
- [ ] File validation completes within 5 seconds for files up to 100MB
- [ ] Discovery queries return results within 100ms
- [ ] System can handle 100+ concurrent file creation requests

### **Quality Requirements**
- [ ] 100% test coverage for new file creation and validation code
- [ ] All quality metrics are accurate and meaningful
- [ ] Error handling provides clear, actionable feedback

## Integration Points

### **With Existing System**
- **Iceberg Catalog**: New files are automatically registered
- **GraphQL API**: New mutations and enhanced queries
- **Storage Layer**: Files are stored in proper partitioned structure
- **Monitoring**: New metrics for file processing and quality

### **With Crawler Services**
- **File Creation**: Crawlers can create files directly
- **Status Updates**: Real-time feedback on file processing status
- **Error Reporting**: Detailed validation errors for crawler debugging

## Future Enhancements

### **Phase 2.7: Advanced File Management**
- File versioning and history
- Automatic file cleanup and archival
- File compression and optimization
- Distributed file storage across multiple nodes

### **Phase 2.8: Real-time Data Streaming**
- Streaming file processing
- Real-time quality monitoring
- Live data gap detection
- Instant catalog updates

This design provides a comprehensive foundation for crawler file integration while maintaining the clean separation of concerns and high performance characteristics of our current architecture.
