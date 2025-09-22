# XBRL Financial Statement Integration Tests

This document describes the comprehensive integration tests for the XBRL financial statement features in the EconGraph project.

## Overview

The integration tests verify the complete end-to-end flow from SEC EDGAR crawling through XBRL parsing, DTS management, and financial data processing. They ensure proper integration between:

- SEC EDGAR crawler and XBRL instance discovery
- XBRL parser and DTS (Discoverable Taxonomy Set) management
- Financial statement generation from XBRL facts
- Financial ratio calculation and analysis
- Frontend components and backend APIs

## Test Structure

### Backend Integration Tests

#### 1. XBRL Financial Integration Tests (`integration_xbrl_financial_tests.rs`)

**Test: `test_xbrl_taxonomy_schema_storage`**
- Verifies XBRL taxonomy schema storage and retrieval
- Tests database operations for taxonomy schemas
- Validates schema metadata (namespace, prefix, version, file type)

**Test: `test_xbrl_instance_processing`**
- Tests XBRL instance document processing end-to-end
- Verifies company creation, instance storage, and fact extraction
- Validates XBRL facts storage and retrieval

**Test: `test_financial_statement_generation`**
- Tests financial statement generation from XBRL data
- Verifies balance sheet equation (Assets = Liabilities + Equity)
- Tests hierarchical line item structure

**Test: `test_sec_crawler_xbrl_integration`**
- Tests SEC crawler integration with XBRL processing
- Verifies crawler configuration and setup
- Tests company discovery and crawling progress

**Test: `test_xbrl_parser_dts_integration`**
- Tests XBRL parser integration with DTS manager
- Verifies local DTS support and taxonomy caching
- Tests parsing of complex XBRL documents

**Test: `test_financial_ratio_calculation`**
- Tests financial ratio calculation from XBRL data
- Verifies key ratios (ROE, profit margin, debt-to-equity)
- Tests percentile ranking and industry benchmarking

**Test: `test_xbrl_dts_dependency_management`**
- Tests XBRL DTS dependency management
- Verifies taxonomy schema relationships
- Tests dependency tracking and resolution

**Test: `test_xbrl_instance_dts_references`**
- Tests XBRL instance DTS reference tracking
- Verifies taxonomy schema references
- Tests reference resolution and validation

#### 2. SEC Crawler XBRL Pipeline Tests (`integration_sec_crawler_xbrl_pipeline.rs`)

**Test: `test_sec_crawler_xbrl_pipeline`**
- Tests complete SEC crawler pipeline with XBRL processing
- Verifies end-to-end flow from crawling to financial analysis
- Tests all pipeline stages:
  1. Company discovery and creation
  2. XBRL instance discovery
  3. DTS reference discovery
  4. XBRL parsing
  5. Financial statement generation
  6. Financial line item extraction
  7. Financial ratio calculation

**Test: `test_dts_management_pipeline`**
- Tests DTS management and taxonomy schema processing
- Verifies taxonomy component storage
- Tests DTS dependency creation and management

**Test: `test_xbrl_parser_dts_integration`**
- Tests XBRL parser integration with DTS manager
- Verifies parsing with local DTS support
- Tests complex XBRL document parsing

**Test: `test_financial_statement_generation_pipeline`**
- Tests financial statement generation from XBRL facts
- Verifies balance sheet equation validation
- Tests hierarchical line item structure

**Test: `test_financial_ratio_calculation_pipeline`**
- Tests financial ratio calculation pipeline
- Verifies key financial ratios
- Tests ratio storage and retrieval

### Frontend Integration Tests

#### XBRL Financial Integration Tests (`xbrl-financial-integration.test.tsx`)

**Test Suite: Financial Dashboard Integration**
- Tests company financial data loading from XBRL sources
- Verifies financial ratios display
- Tests benchmark comparisons with industry data

**Test Suite: Financial Statement Viewer Integration**
- Tests balance sheet data display from XBRL parsing
- Tests income statement data display
- Tests hierarchical line item display

**Test Suite: Benchmark Comparison Integration**
- Tests industry benchmark data display for XBRL-derived ratios
- Tests industry distribution percentiles
- Verifies performance indicators

**Test Suite: Trend Analysis Integration**
- Tests trend analysis for XBRL-derived financial ratios
- Tests trend direction and strength indicators
- Verifies historical data display

**Test Suite: XBRL Data Processing Integration**
- Tests XBRL instance document processing status
- Tests XBRL taxonomy schema references
- Verifies data processing pipeline

**Test Suite: Error Handling Integration**
- Tests XBRL parsing error handling
- Tests missing XBRL taxonomy schema handling
- Verifies graceful error recovery

**Test Suite: Performance Integration**
- Tests large XBRL dataset handling
- Verifies performance requirements
- Tests data loading efficiency

## Test Data

### Mock Company Data
```typescript
const mockCompany = {
  id: 'test-company-1',
  cik: '0000320193',
  name: 'Apple Inc.',
  ticker: 'AAPL',
  industry: 'Computer Hardware',
  sector: 'Technology'
};
```

### Mock Financial Statements
- Balance Sheet with Assets, Liabilities, and Stockholders' Equity
- Income Statement with Revenues and Net Income
- Hierarchical line item structure with proper parent-child relationships

### Mock Financial Ratios
- Return on Equity (ROE): 14.7%
- Net Profit Margin: 25.3%
- Industry benchmarks and percentile rankings

### Mock Benchmark Data
- Industry distribution percentiles (P10, P25, Median, P75, P90)
- Performance indicators and confidence levels
- Trend analysis data

## Running the Tests

### Backend Integration Tests

```bash
# Run all backend integration tests
cd backend
./scripts/run-integration-tests.sh

# Run specific test file
cargo test --test integration_xbrl_financial_tests -- --nocapture

# Run specific test
cargo test test_xbrl_taxonomy_schema_storage --test integration_xbrl_financial_tests -- --nocapture
```

### Frontend Integration Tests

```bash
# Run all frontend integration tests
cd frontend
./scripts/run-integration-tests.sh

# Run specific test file
npm test -- --testPathPattern="xbrl-financial-integration.test.tsx" --watchAll=false

# Run specific test suite
npm test -- --testPathPattern="xbrl-financial-integration.test.tsx" --testNamePattern="Financial Dashboard Integration" --watchAll=false
```

## Test Environment Setup

### Database Requirements
- PostgreSQL database running on localhost:5432
- Test database: `econgraph_test`
- User: `econgraph` with password `password`

### Dependencies
- Rust toolchain with Cargo
- Node.js and npm
- PostgreSQL client tools
- Diesel CLI for database migrations

### Environment Variables
```bash
export DATABASE_URL="postgresql://econgraph:password@localhost/econgraph_test"
export REACT_APP_API_URL="http://localhost:8080"
export REACT_APP_ENVIRONMENT="test"
```

## Test Coverage

The integration tests cover:

### Backend Coverage
- ✅ XBRL taxonomy schema storage and retrieval
- ✅ XBRL instance document processing
- ✅ DTS reference discovery and management
- ✅ XBRL parsing with local DTS support
- ✅ Financial statement generation from XBRL facts
- ✅ Financial ratio calculation and analysis
- ✅ SEC crawler integration
- ✅ Database operations and data integrity
- ✅ Error handling and edge cases

### Frontend Coverage
- ✅ Financial dashboard component integration
- ✅ Financial statement viewer integration
- ✅ Benchmark comparison integration
- ✅ Trend analysis integration
- ✅ XBRL data processing integration
- ✅ Error handling and user feedback
- ✅ Performance with large datasets
- ✅ API integration and data flow

## Test Validation

### Data Integrity Checks
- Balance sheet equation validation (Assets = Liabilities + Equity)
- XBRL fact accuracy and completeness
- Financial ratio calculation accuracy
- Industry benchmark data validation

### Performance Requirements
- Large dataset processing (< 2 seconds)
- Memory usage optimization
- Database query efficiency
- Frontend rendering performance

### Error Handling
- XBRL parsing error recovery
- Missing taxonomy schema handling
- Network error handling
- Database connection error handling

## Continuous Integration

The integration tests are designed to run in CI/CD pipelines:

1. **Database Setup**: Automated test database creation and migration
2. **Test Execution**: Parallel test execution where possible
3. **Cleanup**: Automated test database cleanup
4. **Reporting**: Detailed test results and coverage reports

## Troubleshooting

### Common Issues

**Database Connection Errors**
- Ensure PostgreSQL is running
- Verify database credentials
- Check network connectivity

**XBRL Parsing Errors**
- Verify test data format
- Check DTS schema availability
- Validate XML structure

**Frontend Test Failures**
- Ensure backend API is accessible
- Check mock data consistency
- Verify component dependencies

### Debug Mode

Run tests with verbose output for debugging:
```bash
cargo test -- --nocapture --test-threads=1
npm test -- --verbose --watchAll=false
```

## Future Enhancements

### Planned Test Additions
- Multi-company comparison tests
- Historical data trend analysis tests
- Real-time data update tests
- Cross-browser compatibility tests
- Mobile responsiveness tests

### Performance Testing
- Load testing with large datasets
- Stress testing with concurrent users
- Memory leak detection
- Database performance optimization

### Security Testing
- XBRL document validation
- SQL injection prevention
- XSS protection
- API security validation
