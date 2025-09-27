# EconGraph Services

The business logic and service layer for the EconGraph system, providing comprehensive data processing, external API integrations, and economic data discovery capabilities. This crate serves as the core business logic layer that powers data collection, processing, and analysis across multiple economic data sources.

## Features

- **Data Discovery**: Comprehensive series discovery across multiple economic data sources (FRED, BLS, Census, World Bank, etc.)
- **Crawler Services**: Advanced web crawling and data collection with politeness and rate limiting
- **Search & Analysis**: Global economic analysis and intelligent search functionality
- **API Integration**: Integration with major economic data providers and government APIs
- **Queue Management**: Asynchronous task processing and job scheduling for scalable operations
- **Collaboration**: User collaboration and data sharing features for team workflows

## Testing

The crate includes comprehensive tests to ensure business logic correctness, external API integrations, and data processing accuracy.

### Test Types

#### **Unit Tests**
- **Purpose**: Test individual services and business logic in isolation
- **Examples**: Data processing functions, API client logic, search algorithms, queue operations
- **Benefits**: Fast execution, no external dependencies, catch logic errors early

#### **Integration Tests**
- **Purpose**: Test services with real external API interactions and database operations
- **Examples**: API discovery workflows, crawler operations, search functionality, queue processing
- **Benefits**: Catch integration issues, test real-world scenarios, validate external API handling

#### **Crawler Tests**
- **Purpose**: Validate web crawling functionality and data collection accuracy
- **Examples**: Data source crawling, content parsing, rate limiting, error handling
- **Benefits**: Ensure data quality, validate crawling politeness, test error recovery

#### **API Integration Tests**
- **Purpose**: Test external API integrations and data source connectivity
- **Examples**: FRED API integration, Census data retrieval, World Bank data access
- **Benefits**: Validate API connectivity, test data transformation, ensure error handling

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test services::search
cargo test services::crawler
cargo test integration

# Run with external API testing
EXTERNAL_API_TESTING=true cargo test
```

### Test Infrastructure

- **External API Mocking**: Controlled testing of external API integrations
- **Database Integration**: Real database operations with test data setup and cleanup
- **Crawler Testing**: Isolated web crawling tests with controlled environments
- **Queue Testing**: Asynchronous job processing validation and error handling

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.
