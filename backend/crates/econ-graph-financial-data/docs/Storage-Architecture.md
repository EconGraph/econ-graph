# Storage Architecture Documentation

## Overview

The Financial Data Service uses a sophisticated storage architecture built on Apache Arrow Flight and Parquet files, designed for high-performance time series data operations. The architecture is designed to be upgradeable from direct Parquet files (V1) to Apache Iceberg (V2) without changing the API.

## Architecture Principles

### 1. Zero-Copy Data Transfer
- **Arrow Flight**: Sub-millisecond data transfer using Arrow's columnar format
- **Memory Efficiency**: Direct memory access without serialization overhead
- **Network Optimization**: Efficient binary protocol for data transfer

### 2. Columnar Storage
- **Parquet Format**: Optimized for analytical workloads
- **Compression**: Automatic compression for storage efficiency
- **Schema Evolution**: Support for schema changes over time

### 3. Storage Abstraction
- **Trait-Based Design**: Clean separation between API and storage implementation
- **V1/V2 Compatibility**: Same API for Parquet and Iceberg backends
- **Future-Proof**: Easy migration to advanced data lake features

## V1: Arrow Flight + Parquet

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    Financial Data Service                  │
├─────────────────────────────────────────────────────────────┤
│  GraphQL API  │  Business Logic  │  Storage Abstraction   │
├─────────────────────────────────────────────────────────────┤
│                    Arrow Flight Layer                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │   Zero-Copy     │  │   Schema        │  │   Memory    │ │
│  │   Transfer      │  │   Validation    │  │   Mapping   │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                    Parquet Storage                         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │   Series        │  │   Data Points   │  │   Indexes   │ │
│  │   Metadata      │  │   Time Series  │  │   Hot Data  │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Key Components

#### 1. Storage Abstraction Layer

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

**Benefits:**
- Clean separation of concerns
- Easy testing with in-memory implementations
- Future-proof for Iceberg integration

#### 2. Arrow Flight Integration

```rust
pub struct ParquetStorage {
    data_dir: PathBuf,
    flight_server: Arc<RwLock<Option<FlightServiceServer<ParquetFlightService>>>>,
}
```

**Features:**
- Zero-copy data transfer
- Sub-millisecond latency
- Memory-mapped file support
- Automatic schema validation

#### 3. Parquet File Operations

**Series Storage:**
- One Parquet file per series
- Arrow schema validation
- Automatic compression
- Memory-mapped access for hot data

**Data Points Storage:**
- One Parquet file per series
- Time series indexing
- Date range optimization
- Batch operations support

### Arrow Schemas

#### EconomicSeries Schema

```rust
Schema::new(vec![
    Field::new("id", DataType::Utf8, false),                    // UUID as string
    Field::new("source_id", DataType::Utf8, false),            // Source UUID
    Field::new("external_id", DataType::Utf8, false),           // External identifier
    Field::new("title", DataType::Utf8, false),                // Series title
    Field::new("description", DataType::Utf8, true),            // Optional description
    Field::new("units", DataType::Utf8, true),                  // Optional units
    Field::new("frequency", DataType::Utf8, false),             // Data frequency
    Field::new("seasonal_adjustment", DataType::Utf8, true),    // Optional adjustment
    Field::new("start_date", DataType::Utf8, true),             // Start date string
    Field::new("end_date", DataType::Utf8, true),                // End date string
    Field::new("is_active", DataType::Boolean, false),          // Active status
    Field::new("created_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
    Field::new("updated_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
])
```

#### DataPoint Schema

```rust
Schema::new(vec![
    Field::new("id", DataType::Utf8, false),                    // UUID as string
    Field::new("series_id", DataType::Utf8, false),            // Parent series UUID
    Field::new("date", DataType::Date32, false),               // Date as days since epoch
    Field::new("value", DataType::Float64, true),             // Decimal value as float
    Field::new("revision_date", DataType::Date32, false),      // Revision date
    Field::new("is_original_release", DataType::Boolean, false), // Original release flag
    Field::new("created_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
    Field::new("updated_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
])
```

### Performance Characteristics

#### Single Operations
- **Series Write**: ~2ms for 1 series
- **Data Points Write**: ~5ms for 1,000 points
- **Series Read**: ~1ms for 1 series
- **Data Points Read**: ~3ms for 1,000 points

#### Concurrent Operations
- **Concurrent Writes**: Linear scaling up to 20 concurrent operations
- **Concurrent Reads**: Excellent scaling with minimal contention
- **Mixed Workloads**: Stable performance under mixed read/write loads

#### Memory Usage
- **Base Service**: ~50MB
- **Per 1M Data Points**: ~100MB
- **Arrow Buffers**: Efficient memory management with zero-copy operations

## V2: Arrow Flight + Iceberg (Future)

### Planned Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Financial Data Service                  │
├─────────────────────────────────────────────────────────────┤
│  GraphQL API  │  Business Logic  │  Storage Abstraction   │
├─────────────────────────────────────────────────────────────┤
│                    Arrow Flight Layer                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │   Zero-Copy     │  │   Schema        │  │   Memory    │ │
│  │   Transfer      │  │   Evolution     │  │   Mapping   │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                    Apache Iceberg                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │   Schema        │  │   Time Travel   │  │   ACID      │ │
│  │   Evolution     │  │   & Snapshots   │  │   Trans.    │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │   Partitioning  │  │   Compaction    │  │   Metadata  │ │
│  │   by Date       │  │   & Cleanup     │  │   Catalog   │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Advanced Features

#### 1. Schema Evolution
- **Automatic Updates**: Schema changes without data migration
- **Backward Compatibility**: Read old data with new schemas
- **Forward Compatibility**: Handle future schema changes

#### 2. Time Travel
- **Historical Queries**: Access data at any point in time
- **Snapshot Management**: Automatic snapshot creation
- **Rollback Support**: Revert to previous data states

#### 3. ACID Transactions
- **Data Consistency**: Guaranteed data integrity
- **Concurrent Safety**: Safe concurrent operations
- **Rollback Support**: Transaction rollback on failure

#### 4. Advanced Partitioning
- **Date Partitioning**: Automatic partitioning by date
- **Query Optimization**: Partition pruning for faster queries
- **Storage Efficiency**: Reduced storage costs

#### 5. Compaction & Cleanup
- **Automatic Compaction**: Merge small files for efficiency
- **Data Cleanup**: Remove old or duplicate data
- **Storage Optimization**: Optimize storage layout

### Migration Strategy

#### Phase 1: Parallel Implementation
- Implement `IcebergStorage` alongside `ParquetStorage`
- Use feature flags to switch between implementations
- Maintain API compatibility

#### Phase 2: Data Migration
- Migrate existing Parquet files to Iceberg tables
- Validate data integrity
- Performance testing

#### Phase 3: Cutover
- Switch default storage to Iceberg
- Remove Parquet-only code paths
- Full Iceberg feature utilization

## Storage Tiering (Future Enhancement)

### MinIO Integration

```yaml
# Storage tiering configuration
storage:
  tiers:
    - name: "hot"
      type: "nvme"
      path: "/data/hot"
      max_age_days: 30
    - name: "warm"
      type: "ssd"
      path: "/data/warm"
      max_age_days: 365
    - name: "cold"
      type: "hdd"
      path: "/data/cold"
      max_age_days: 3650
    - name: "archive"
      type: "s3"
      endpoint: "s3.amazonaws.com"
      bucket: "econ-graph-archive"
```

### Automatic Tiering

- **Access Pattern Analysis**: Monitor data access patterns
- **Automatic Movement**: Move data between tiers based on usage
- **Cost Optimization**: Balance performance and storage costs
- **Manual Override**: Allow manual tier management

## Data Lifecycle Management

### Retention Policies

```rust
pub struct RetentionPolicy {
    pub hot_data_days: u32,      // Keep in hot storage
    pub warm_data_days: u32,     // Keep in warm storage
    pub cold_data_days: u32,     // Keep in cold storage
    pub archive_data_days: u32,  // Keep in archive
    pub delete_after_days: u32,  // Delete after this period
}
```

### Automatic Cleanup

- **Data Aging**: Automatic data movement between tiers
- **Compression**: Compress old data for storage efficiency
- **Deletion**: Remove data beyond retention period
- **Audit Trail**: Track all data lifecycle operations

## Performance Optimization

### Caching Strategy

#### Hot Data Cache
- **Memory-Mapped Files**: Fast access to frequently used data
- **LRU Eviction**: Automatic cache management
- **Preloading**: Predictive data loading

#### Query Optimization
- **Index Usage**: Leverage Parquet metadata for fast queries
- **Predicate Pushdown**: Filter data at storage level
- **Column Pruning**: Only read needed columns

### Monitoring

#### Storage Metrics
- **File Count**: Number of Parquet files
- **Storage Size**: Total storage usage
- **Access Patterns**: Data access frequency
- **Performance**: Read/write latencies

#### Health Checks
- **Storage Availability**: Check storage accessibility
- **Data Integrity**: Validate data consistency
- **Performance Monitoring**: Track operation latencies

## Security Considerations

### Data Encryption

- **At Rest**: Encrypt Parquet files on disk
- **In Transit**: Secure Arrow Flight connections
- **Key Management**: Secure key storage and rotation

### Access Control

- **Authentication**: Verify user identity
- **Authorization**: Control data access permissions
- **Audit Logging**: Track all data access

### Compliance

- **Data Retention**: Meet regulatory requirements
- **Data Privacy**: Protect sensitive information
- **Audit Trails**: Maintain comprehensive logs

## Development & Testing

### Local Development

```bash
# Start with in-memory storage
cargo run

# Use Parquet storage
DATA_DIR=./data cargo run

# Use Iceberg storage (V2)
STORAGE_TYPE=iceberg cargo run
```

### Testing

```bash
# Test storage implementations
cargo test storage

# Test Parquet operations
cargo test parquet_integration

# Test Arrow Flight
cargo test arrow_flight
```

### Benchmarking

```bash
# Run storage benchmarks
cargo bench --bench performance

# Test specific operations
cargo bench --bench performance -- --test storage_operations
```

## Configuration

### Environment Variables

```bash
# Storage configuration
DATA_DIR=./data                    # Storage directory
STORAGE_TYPE=parquet              # Storage type (parquet, iceberg)
FLIGHT_SERVER_PORT=8080          # Arrow Flight server port

# Performance tuning
CACHE_SIZE_MB=1024               # Cache size in MB
BATCH_SIZE=1000                  # Batch size for operations
MAX_CONCURRENT_OPERATIONS=20     # Max concurrent operations

# Monitoring
METRICS_ENABLED=true             # Enable metrics collection
HEALTH_CHECK_INTERVAL=30s       # Health check interval
```

### Configuration Files

```yaml
# config/storage.yaml
storage:
  type: "parquet"
  data_dir: "./data"
  cache:
    size_mb: 1024
    eviction_policy: "lru"
  performance:
    batch_size: 1000
    max_concurrent: 20
  monitoring:
    metrics_enabled: true
    health_check_interval: "30s"
```

## Troubleshooting

### Common Issues

#### Storage Errors
- **Permission Denied**: Check file system permissions
- **Disk Full**: Monitor storage usage
- **Corrupted Files**: Validate Parquet file integrity

#### Performance Issues
- **Slow Queries**: Check date range optimization
- **Memory Usage**: Monitor cache size
- **Concurrent Operations**: Adjust concurrency limits

#### Arrow Flight Issues
- **Connection Errors**: Check network connectivity
- **Schema Mismatches**: Validate Arrow schemas
- **Memory Issues**: Monitor Arrow buffer usage

### Debugging Tools

```bash
# Check storage status
curl http://localhost:3001/health

# View metrics
curl http://localhost:3001/metrics

# Validate Parquet files
parquet-tools validate ./data/series_*.parquet
```

## Future Enhancements

### Planned Features

1. **Real-time Streaming**: Apache Kafka integration
2. **Advanced Analytics**: SQL query support
3. **Machine Learning**: ML pipeline integration
4. **Data Versioning**: Git-like data versioning
5. **Multi-Region**: Cross-region data replication

### Research Areas

1. **Query Optimization**: Advanced query planning
2. **Compression**: Better compression algorithms
3. **Indexing**: Advanced indexing strategies
4. **Caching**: Intelligent caching algorithms
5. **Security**: Enhanced security features

---

*This storage architecture documentation is maintained alongside the codebase and reflects the current implementation and future plans.*
