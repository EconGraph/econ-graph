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

#### **2.5.2.1 Crawler File Creation API**

**New GraphQL Mutation**: `createSeriesFromFile`
```graphql
mutation CreateSeriesFromFile($input: CreateSeriesFromFileInput!) {
  createSeriesFromFile(input: $input) {
    success
    seriesId
    filePath
    dataPointsProcessed
    errors
  }
}

input CreateSeriesFromFileInput {
  externalId: String!
  title: String!
  description: String
  units: String
  frequency: String!
  seasonalAdjustment: String
  source: String!
  filePath: String!  # Path to Parquet file created by crawler
  expectedDataPoints: Int
  startDate: Date
  endDate: Date
}
```

**Implementation Details**:
- Validate the Parquet file exists and is readable
- Verify file format and schema compatibility
- Extract metadata from the file (data point count, date ranges)
- Register the series in the Iceberg catalog
- Move the file to the proper partitioned directory structure
- Validate data integrity and completeness

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

#### **2.5.2.3 File Validation & Integrity Checks**

**Validation Pipeline**:
1. **File Format Validation**:
   - Verify Parquet file can be opened and read
   - Check required columns exist (date, value, series_id)
   - Validate data types match expected schema

2. **Data Integrity Validation**:
   - Verify date ranges are logical (start ≤ end)
   - Check for duplicate dates within the file
   - Validate numeric values are within reasonable bounds
   - Check for missing values and gaps

3. **Schema Compatibility**:
   - Ensure Parquet schema matches our `DataPoint` model
   - Verify column names and types
   - Check for unexpected columns

4. **Metadata Validation**:
   - Validate external ID format
   - Check frequency and seasonal adjustment values
   - Verify source information

**Error Handling**:
- Return detailed validation errors
- Support partial file processing
- Maintain audit trail of validation failures

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
    pub completeness: f64,  // Actual vs expected data points
    
    // Data Quality
    pub data_quality_score: f64,  // 0.0 to 1.0
    pub gaps: Vec<DataGap>,
    pub outliers: Vec<DataOutlier>,
    
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
    pub severity: GapSeverity,
}

#[derive(async_graphql::Enum)]
pub enum GapType {
    MissingData,      // Expected data points are missing
    DuplicateData,    // Same date appears multiple times
    InvalidDate,      // Date is outside expected range
    StaleData,        // Data is older than expected
}

#[derive(async_graphql::Enum)]
pub enum GapSeverity {
    Low,    // Minor gaps that don't affect analysis
    Medium, // Noticeable gaps that may impact analysis
    High,   // Significant gaps that affect data quality
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
    completeness
    gaps {
      startDate
      endDate
      gapType
      severity
    }
    qualityScore
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

3. **Data Quality Summary**:
```graphql
query DataQualitySummary($seriesIds: [UUID!]) {
  dataQualitySummary(seriesIds: $seriesIds) {
    totalSeries
    averageQualityScore
    seriesWithGaps
    seriesWithErrors
    qualityDistribution {
      excellent  # > 0.9
      good       # 0.7 - 0.9
      fair       # 0.5 - 0.7
      poor       # < 0.5
    }
  }
}
```

#### **2.5.3.3 Data Quality Metrics**

**Quality Score Calculation**:
```rust
pub struct DataQualityMetrics {
    pub completeness_score: f64,    // Actual vs expected data points
    pub consistency_score: f64,     // Frequency consistency
    pub freshness_score: f64,       // How recent is the data
    pub accuracy_score: f64,        // Outlier detection
    pub overall_score: f64,         // Weighted combination
}

impl DataQualityMetrics {
    pub fn calculate(series: &SeriesMetadata, data_points: &[DataPoint]) -> Self {
        let completeness = self.calculate_completeness(series, data_points);
        let consistency = self.calculate_consistency(series, data_points);
        let freshness = self.calculate_freshness(series, data_points);
        let accuracy = self.calculate_accuracy(data_points);
        
        let overall = (completeness * 0.4 + consistency * 0.2 + 
                      freshness * 0.2 + accuracy * 0.2);
        
        Self {
            completeness_score: completeness,
            consistency_score: consistency,
            freshness_score: freshness,
            accuracy_score: accuracy,
            overall_score: overall,
        }
    }
}
```

#### **2.5.3.4 Data Gap Detection**

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
    
    // Check for gaps between consecutive points
    for window in sorted_points.windows(2) {
        let gap_days = window[1].date.signed_duration_since(window[0].date).num_days();
        let expected_gap = expected_frequency.days_between_points();
        
        if gap_days > expected_gap * 2 {  // Allow some tolerance
            gaps.push(DataGap {
                start_date: window[0].date,
                end_date: window[1].date,
                gap_type: GapType::MissingData,
                severity: self.assess_gap_severity(gap_days, expected_gap),
            });
        }
    }
    
    // Check for duplicate dates
    let mut seen_dates = HashSet::new();
    for point in data_points {
        if !seen_dates.insert(point.date) {
            gaps.push(DataGap {
                start_date: point.date,
                end_date: point.date,
                gap_type: GapType::DuplicateData,
                severity: GapSeverity::High,
            });
        }
    }
    
    gaps
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
