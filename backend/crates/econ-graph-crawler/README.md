# EconGraph Crawler

Data acquisition and crawling functionality for the EconGraph system, providing comprehensive tools for crawling external data sources with advanced features like rate limiting, retry logic, and data validation. This crate serves as the data collection layer that enables systematic acquisition of economic data from various sources.

## Features

- **Multi-Source Crawling**: Support for various economic data sources and APIs
- **Rate Limiting**: Intelligent rate limiting to respect source policies
- **Retry Logic**: Robust retry mechanisms with exponential backoff
- **Data Validation**: Comprehensive data validation and quality checks
- **Progress Tracking**: Real-time progress monitoring and status reporting
- **Error Handling**: Comprehensive error handling and recovery mechanisms

## Testing

The crate includes comprehensive tests to ensure crawling functionality, data validation, and error handling work correctly.

### Test Types

#### **Unit Tests**
- **Purpose**: Test individual crawling components in isolation
- **Examples**: Rate limiting logic, retry mechanisms, data validation, error handling
- **Benefits**: Fast execution, no external dependencies, catch logic errors early

#### **Integration Tests**
- **Purpose**: Test crawling with real external data sources and APIs
- **Examples**: End-to-end crawling workflows, data source integration, progress tracking
- **Benefits**: Catch integration issues, test real-world scenarios, validate data quality

#### **Rate Limiting Tests**
- **Purpose**: Validate rate limiting functionality and politeness compliance
- **Examples**: Rate limit enforcement, backoff behavior, source policy compliance
- **Benefits**: Ensure respectful crawling, prevent source blocking, validate politeness

#### **Data Validation Tests**
- **Purpose**: Validate data quality and parsing accuracy
- **Examples**: Data format validation, content parsing, quality checks, error detection
- **Benefits**: Ensure data accuracy, validate parsing logic, catch data quality issues

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test crawler
cargo test rate_limiting
cargo test integration

# Run with external data source testing
EXTERNAL_SOURCE_TESTING=true cargo test
```

### Test Infrastructure

- **External Source Mocking**: Controlled testing of external data source interactions
- **Rate Limiting Testing**: Automated rate limiting behavior validation
- **Data Quality Testing**: Comprehensive data validation and quality assurance
- **Error Recovery Testing**: Robust error handling and recovery mechanism validation

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.
