# Financial Data Service Examples

This directory contains comprehensive examples demonstrating how to use the Financial Data Service.

## Examples Overview

### 1. Basic Usage (`basic-usage.rs`)

Demonstrates the core functionality of the service:

- **In-Memory Operations**: Working with the in-memory database
- **Parquet Storage**: Using Parquet files for persistent storage
- **Crawler API**: Bulk data ingestion
- **Error Handling**: Proper error handling and validation

**Run:**
```bash
cargo run --example basic-usage
```

**Key Features:**
- Creating economic series
- Adding data points
- Querying data with date ranges
- Arrow schema operations
- Error handling patterns

### 2. GraphQL Client (`graphql-client.rs`)

Shows how to interact with the service using GraphQL:

- **Health Checks**: Service health monitoring
- **Series Management**: Creating and querying series
- **Data Operations**: Adding and retrieving data points
- **Error Handling**: GraphQL error handling

**Prerequisites:**
- Service must be running on `http://localhost:3001`
- Start the service: `cargo run`

**Run:**
```bash
cargo run --example graphql-client
```

**Key Features:**
- HTTP GraphQL requests
- Query and mutation examples
- Error handling
- Service health monitoring

### 3. Performance Testing (`performance-testing.rs`)

Comprehensive performance testing and benchmarking:

- **Single Operations**: Latency testing for individual operations
- **Concurrent Operations**: Throughput testing with multiple clients
- **Bulk Operations**: Large-scale data processing
- **Memory Usage**: Memory consumption analysis
- **Parquet Performance**: Storage layer performance

**Run:**
```bash
cargo run --example performance-testing
```

**Key Features:**
- Throughput measurements
- Latency analysis
- Memory usage profiling
- Concurrent operation testing
- Storage performance testing

## Running Examples

### Prerequisites

1. **Install Dependencies:**
   ```bash
   cargo build
   ```

2. **Start the Service (for GraphQL examples):**
   ```bash
   cargo run
   ```

### Basic Usage Example

```bash
# Run basic usage example
cargo run --example basic-usage

# Expected output:
# 🚀 Financial Data Service - Basic Usage Examples
# ================================================
# 
# 📊 Example 1: In-Memory Database Operations
# Creating in-memory database...
# Creating economic series...
# ✅ Created series: US Gross Domestic Product (123e4567-e89b-12d3-a456-426614174000)
# ...
```

### GraphQL Client Example

```bash
# Terminal 1: Start the service
cargo run

# Terminal 2: Run the GraphQL client
cargo run --example graphql-client

# Expected output:
# 🚀 Financial Data Service - GraphQL Client Examples
# ===================================================
# 
# ⏳ Waiting for service to be ready...
# ✅ Service is ready!
# 🏥 Example 1: Health Check
# ✅ Health check response: "Financial Data Service is healthy"
# ...
```

### Performance Testing Example

```bash
# Run performance testing
cargo run --example performance-testing

# Expected output:
# 🚀 Financial Data Service - Performance Testing Examples
# ======================================================
# 
# ⚡ Example 1: Single Operation Performance
# ✅ Series creation: 1.234ms
# ✅ 1000 data points creation: 5.678ms
# ✅ Series retrieval: 0.123ms
# ✅ 1000 data points retrieval: 2.345ms
# ...
```

## Example Outputs

### Basic Usage Output

```
🚀 Financial Data Service - Basic Usage Examples
=================================================

📊 Example 1: In-Memory Database Operations
Creating in-memory database...
Creating economic series...
✅ Created series: US Gross Domestic Product (123e4567-e89b-12d3-a456-426614174000)
Creating data points...
✅ Created 3 data points
Querying series...
✅ Retrieved series: US Gross Domestic Product (123e4567-e89b-12d3-a456-426614174000)
Querying data points...
✅ Retrieved 3 data points
✅ Retrieved 2 data points in date range

💾 Example 2: Parquet Storage Operations
Creating Parquet storage...
Writing series to Parquet...
✅ Series written to Parquet
Writing data points to Parquet...
✅ Data points written to Parquet
Reading series from Parquet...
✅ Retrieved series: US Unemployment Rate (123e4567-e89b-12d3-a456-426614174000)
Reading data points from Parquet...
✅ Retrieved 2 data points
Creating Arrow schemas...
✅ Series schema: 12 fields
✅ Data points schema: 8 fields
✅ Cleaned up temporary files

🕷️ Example 3: Crawler API Usage
Creating crawler API...
Processing clean data...
✅ Processed 2 data points
✅ Success: true
✅ Message: Data processed successfully

⚠️ Example 4: Error Handling
Demonstrating error handling...
✅ Correctly returned None for non-existent series
✅ Correctly returned empty array for non-existent series
✅ Correctly caught validation error: Validation failed

✅ All examples completed successfully!
```

### GraphQL Client Output

```
🚀 Financial Data Service - GraphQL Client Examples
==================================================

⏳ Waiting for service to be ready...
✅ Service is ready!

🏥 Example 1: Health Check
✅ Health check response: "Financial Data Service is healthy"

📊 Example 2: Create Economic Series
✅ Created series: US Gross Domestic Product (123e4567-e89b-12d3-a456-426614174000)
   Frequency: QUARTERLY
   Active: true

📈 Example 3: Add Data Points
✅ Created 4 data points
   Point 1: 2023-01-01 = 25000.5
   Point 2: 2023-04-01 = 25100.2
   Point 3: 2023-07-01 = 25200.8
   Point 4: 2023-10-01 = 25300.1

🔍 Example 4: Query Data
✅ Retrieved series: US Gross Domestic Product (123e4567-e89b-12d3-a456-426614174000)
   Description: Quarterly GDP data for the United States
   Units: Billions of USD
   Frequency: QUARTERLY
   Seasonal Adjustment: SAAR
   Date Range: 1947-01-01 to 2023-12-31
   Active: true
✅ Retrieved 4 data points
   2023-01-01: 25000.5 (revised: 2023-01-15, original: true)
   2023-04-01: 25100.2 (revised: 2023-04-15, original: true)
   2023-07-01: 25200.8 (revised: 2023-07-15, original: true)
   2023-10-01: 25300.1 (revised: 2023-10-15, original: true)

📋 Example 5: List All Series
✅ Found 1 series
   1: US Gross Domestic Product (QUARTERLY) - Active

⚠️ Example 6: Error Handling
✅ Correctly returned null for non-existent series
✅ Correctly caught validation errors: [{"message":"Invalid UUID format","locations":[{"line":2,"column":3}],"path":["createSeries"]}]

✅ All GraphQL examples completed successfully!
```

### Performance Testing Output

```
🚀 Financial Data Service - Performance Testing Examples
======================================================

⚡ Example 1: Single Operation Performance
✅ Series creation: 1.234ms
✅ 1000 data points creation: 5.678ms
✅ Series retrieval: 0.123ms
✅ 1000 data points retrieval: 2.345ms
✅ Date range query (30 days): 0.456ms
   Found 30 points in date range

🔄 Example 2: Concurrent Operations
Testing 1 concurrent operations...
✅ 1 concurrent writes (100 points each): 12.345ms
✅ 1 concurrent reads: 0.123ms
Testing 5 concurrent operations...
✅ 5 concurrent writes (100 points each): 15.678ms
✅ 5 concurrent reads: 0.234ms
Testing 10 concurrent operations...
✅ 10 concurrent writes (100 points each): 18.901ms
✅ 10 concurrent reads: 0.345ms
Testing 20 concurrent operations...
✅ 20 concurrent writes (100 points each): 25.123ms
✅ 20 concurrent reads: 0.456ms

📊 Example 3: Bulk Data Operations
Testing batch size: 100
✅ 100 data points: 2.345ms (42 points/sec)
✅ Read 100 data points: 0.123ms (813 points/sec)
Testing batch size: 1000
✅ 1000 data points: 12.345ms (81 points/sec)
✅ Read 1000 data points: 1.234ms (810 points/sec)
Testing batch size: 10000
✅ 10000 data points: 98.765ms (101 points/sec)
✅ Read 10000 data points: 8.901ms (1123 points/sec)
Testing batch size: 50000
✅ 50000 data points: 456.789ms (109 points/sec)
✅ Read 50000 data points: 45.678ms (1095 points/sec)

💾 Example 4: Memory Usage Analysis
Memory usage analysis (approximate)...
Testing memory usage with 1000 data points...
✅ 1000 data points: 12.345ms (estimated 0 MB)
Testing memory usage with 10000 data points...
✅ 10000 data points: 98.765ms (estimated 1 MB)
Testing memory usage with 100000 data points...
✅ 100000 data points: 987.654ms (estimated 19 MB)

🗄️ Example 5: Parquet Storage Performance
✅ Series write to Parquet: 2.345ms
Testing Parquet performance with 1000 data points...
✅ Write 1000 data points: 5.678ms (176 points/sec)
✅ Read 1000 data points: 2.345ms (426 points/sec)
Testing Parquet performance with 10000 data points...
✅ Write 10000 data points: 45.678ms (219 points/sec)
✅ Read 10000 data points: 12.345ms (810 points/sec)
Testing Parquet performance with 100000 data points...
✅ Write 100000 data points: 456.789ms (219 points/sec)
✅ Read 100000 data points: 98.765ms (1012 points/sec)
✅ 1000 Arrow schema creations: 12.345ms
✅ RecordBatch conversion (10k points): 5.678ms
✅ Cleaned up temporary files

✅ All performance tests completed successfully!
```

## Customizing Examples

### Modifying Data

You can customize the examples by modifying the data generation functions:

```rust
// In performance-testing.rs
fn create_test_data_points(series_id: Uuid, count: usize) -> Vec<DataPoint> {
    // Customize the data generation logic here
    // ...
}
```

### Adding New Examples

To add a new example:

1. Create a new file in the `examples/` directory
2. Add the example to `Cargo.toml`:

```toml
[[example]]
name = "your-example"
path = "examples/your-example.rs"
```

3. Run with:
```bash
cargo run --example your-example
```

### Environment Variables

You can customize examples using environment variables:

```bash
# Set custom service URL
SERVICE_URL=http://localhost:3001 cargo run --example graphql-client

# Set custom data directory
DATA_DIR=/tmp/econ-graph cargo run --example basic-usage

# Set log level
RUST_LOG=debug cargo run --example performance-testing
```

## Troubleshooting

### Common Issues

1. **Service Not Ready**: Make sure the service is running before running GraphQL examples
2. **Permission Errors**: Check file system permissions for Parquet operations
3. **Memory Issues**: Reduce data sizes in performance examples
4. **Network Issues**: Check service URL and connectivity

### Debug Mode

Run examples in debug mode for more detailed output:

```bash
RUST_LOG=debug cargo run --example basic-usage
```

### Performance Considerations

- **Memory Usage**: Large data sets may require significant memory
- **Disk Space**: Parquet examples create temporary files
- **Network**: GraphQL examples require network connectivity
- **Concurrency**: High concurrency may impact system performance

## Contributing

When adding new examples:

1. **Follow Naming Conventions**: Use kebab-case for file names
2. **Add Documentation**: Include comprehensive comments
3. **Handle Errors**: Implement proper error handling
4. **Test Thoroughly**: Ensure examples work in all environments
5. **Update README**: Document new examples in this file

---

*These examples demonstrate the full capabilities of the Financial Data Service and serve as both learning tools and performance benchmarks.*
