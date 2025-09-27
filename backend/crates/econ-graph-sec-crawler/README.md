# EconGraph SEC Crawler

SEC EDGAR XBRL crawler for the EconGraph system, providing comprehensive functionality for crawling SEC EDGAR filings and downloading XBRL financial data. This crate includes advanced features like rate limiting, retry logic, progress tracking, and financial ratio calculation for reliable financial data acquisition.

## Features

- **SEC EDGAR Crawling**: Comprehensive crawling of SEC EDGAR filings and XBRL data
- **XBRL Parsing**: Advanced XBRL document parsing and financial data extraction
- **Rate Limiting**: Intelligent rate limiting to respect SEC policies
- **Retry Logic**: Robust retry mechanisms with exponential backoff
- **Financial Analysis**: Automated financial ratio calculation and analysis
- **Data Validation**: Comprehensive data validation and quality checks
- **Progress Tracking**: Real-time progress monitoring and status reporting

## Testing

The crate includes comprehensive tests to ensure SEC crawling functionality, XBRL parsing accuracy, and financial analysis work correctly.

### Test Types

#### **Unit Tests**
- **Purpose**: Test individual SEC crawling components in isolation
- **Examples**: XBRL parsing, rate limiting, retry logic, financial ratio calculations
- **Benefits**: Fast execution, no external dependencies, catch logic errors early

#### **Integration Tests**
- **Purpose**: Test SEC crawling with real EDGAR filings and XBRL documents
- **Examples**: End-to-end crawling workflows, XBRL document processing, financial data extraction
- **Benefits**: Catch integration issues, test real-world scenarios, validate data accuracy

#### **XBRL Parsing Tests**
- **Purpose**: Validate XBRL document parsing and financial data extraction
- **Examples**: Document parsing, taxonomy handling, financial statement extraction, ratio calculations
- **Benefits**: Ensure parsing accuracy, validate financial data, test complex XBRL structures

#### **Financial Analysis Tests**
- **Purpose**: Validate financial ratio calculations and analysis accuracy
- **Examples**: Ratio calculations, benchmark comparisons, financial statement analysis
- **Benefits**: Ensure calculation accuracy, validate financial analysis, test edge cases

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test xbrl_parser
cargo test financial_ratio_calculator
cargo test integration

# Run with SEC EDGAR testing
SEC_EDGAR_TESTING=true cargo test
```

### Test Infrastructure

- **SEC EDGAR Mocking**: Controlled testing of SEC EDGAR interactions
- **XBRL Document Testing**: Comprehensive XBRL document parsing and validation
- **Financial Analysis Testing**: Automated financial ratio calculation validation
- **Rate Limiting Testing**: SEC politeness compliance and rate limiting validation

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.
