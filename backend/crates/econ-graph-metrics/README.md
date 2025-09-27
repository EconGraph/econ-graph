# EconGraph Metrics

The metrics collection crate for the EconGraph system, providing comprehensive monitoring capabilities for crawlers, performance tracking, and system health using Prometheus. This crate serves as the central metrics layer that enables observability across all EconGraph components, from web crawlers to data processing pipelines.

## Features

- **Crawler Metrics**: Request tracking, duration monitoring, and error reporting
- **Performance Monitoring**: Request duration histograms and throughput metrics  
- **Error Tracking**: Comprehensive error categorization and counting
- **Resource Usage**: Bandwidth and data collection metrics
- **Rate Limiting**: Track rate limit hits and retry attempts

## Testing

The crate includes comprehensive tests to ensure metrics collection works correctly and provides accurate monitoring data.

### Test Types

#### **Unit Tests**
- **Purpose**: Test individual metric collection methods in isolation
- **Examples**: Metric initialization, counter increments, histogram observations, label validation
- **Benefits**: Fast execution, no external dependencies, catch logic errors early

#### **Integration Tests**
- **Purpose**: Test metrics collection with real Prometheus registry interactions
- **Examples**: Metric registration, label value validation, metric export functionality
- **Benefits**: Catch registry-specific issues, test real-world scenarios, validate metric accuracy

#### **Performance Tests**
- **Purpose**: Ensure metrics collection doesn't impact application performance
- **Examples**: High-frequency metric recording, memory usage validation, concurrent access
- **Benefits**: Prevent performance regressions, validate scalability, ensure minimal overhead

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test crawler
cargo test integration

# Run with metrics validation
RUST_LOG=debug cargo test
```

### Test Infrastructure

- **Prometheus Registry**: Each test gets a fresh registry for complete isolation
- **Metric Validation**: Tests verify metric names, labels, and values are correct
- **Concurrent Testing**: Validates thread-safe metric collection under load
- **Memory Testing**: Ensures metrics don't leak memory during long-running operations

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.
