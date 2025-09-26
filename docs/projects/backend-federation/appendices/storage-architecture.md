# Storage Architecture

## Overview

This appendix covers the storage architecture decisions for the financial data service, including alternatives analysis, tiering solutions, and manual control approaches.

## Table of Contents

1. [Storage Alternatives Analysis](storage-alternatives-analysis.md)
2. [Automatic Storage Tiering Solutions](automatic-storage-tiering-solutions.md)
3. [Iceberg Manual Control](iceberg-manual-control.md)

## Key Decisions

### **Primary Storage**: Apache Iceberg + Parquet
- **Why**: Optimized for large time series data, append-only operations, schema evolution
- **Benefits**: ACID transactions, time travel, automatic compaction, schema evolution
- **Local Storage**: MinIO or custom Parquet manager for NVMe-level performance

### **Storage Tiering**: Manual Control + Iceberg
- **Approach**: Hybrid manual/automatic tiering
- **Tiers**: Hot (NVMe) → Warm (SSD) → Cold (HDD) → Archive (Compressed)
- **Control**: Manual triggers for compaction, archival, and tiering

### **Data Lifecycle**: Dual Iceberg Architecture
- **Crawler Service**: Raw data archive, job history, validation patterns
- **Financial Service**: Clean financial series and data points
- **Separation**: Crawler handles raw data, Financial service handles clean data

## Implementation Strategy

### **Phase 1**: Local Storage with Manual Control
```rust
pub struct LocalFinancialDataService {
    hot_storage: NVMeStorage,      // Local NVMe for hot data
    warm_storage: SSDStorage,     // Local SSD for warm data  
    cold_storage: HDDStorage,     // Local HDD for cold data
    archive_storage: CompressedStorage, // Compressed for archive
    tiering_manager: FrequencyTieringManager,
}
```

### **Phase 2**: Production Scale
- **Cloud Storage**: S3 Intelligent Tiering for automatic tiering
- **Local Cache**: NVMe cache for frequently accessed data
- **Hybrid**: Best of both local performance and cloud scalability

## Benefits

1. **Performance**: Local NVMe access for hot data
2. **Cost Control**: Manual tiering based on actual usage patterns
3. **Flexibility**: Can adjust tiering strategy as needed
4. **Data Sovereignty**: Keep sensitive financial data local
5. **Scalability**: Can scale to cloud when needed

## Next Steps

1. Implement local storage with manual tiering
2. Test with real financial data
3. Optimize tiering based on usage patterns
4. Plan cloud migration for production scale
