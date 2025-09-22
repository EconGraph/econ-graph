# XBRL Financial CI Integration Tests Documentation

## Overview

This document describes the comprehensive CI integration test structure for XBRL financial statement features, organized within the existing CI framework using testcontainers for proper isolation and reproducibility.

## Architecture

The integration tests are organized into two main components:

### Backend Integration Tests
- **Location**: `backend/crates/econ-graph-sec-crawler/tests/xbrl_financial_integration_tests.rs`
- **Technology**: Rust with testcontainers for PostgreSQL
- **Scope**: End-to-end XBRL processing pipeline from SEC EDGAR crawling through financial data processing

### Frontend Integration Tests
- **Location**: `frontend/src/__tests__/integration/xbrl-financial/`
- **Technology**: React Testing Library with Jest
- **Scope**: UI component integration with XBRL financial data APIs

## Backend Integration Tests

### Test Structure

The backend integration tests use testcontainers to provide isolated PostgreSQL instances for each test run, ensuring:

1. **Isolation**: Each test runs with a clean database state
2. **Reproducibility**: Tests produce consistent results across environments
3. **Parallel Execution**: Multiple tests can run simultaneously without conflicts

### Key Test Categories

#### 1. XBRL Taxonomy Schema Storage and Retrieval
```rust
#[tokio::test]
#[serial]
async fn test_xbrl_taxonomy_schema_storage() {
    let container = TestContainer::new().await;
    container.clean_database().await.unwrap();
    // Test taxonomy schema storage and retrieval
}
```

#### 2. XBRL Instance Document Processing
```rust
#[tokio::test]
#[serial]
async fn test_xbrl_instance_processing() {
    // Test complete XBRL instance processing pipeline
}
```

#### 3. Financial Statement Generation
```rust
#[tokio::test]
#[serial]
async fn test_financial_statement_generation() {
    // Test generation of financial statements from XBRL facts
}
```

#### 4. SEC Crawler Integration
```rust
#[tokio::test]
#[serial]
async fn test_sec_crawler_xbrl_integration() {
    // Test SEC crawler with XBRL processing
}
```

#### 5. Complete Pipeline Testing
```rust
#[tokio::test]
#[serial]
async fn test_complete_sec_crawler_xbrl_pipeline() {
    // Test end-to-end pipeline from crawling to financial analysis
}
```

### Running Backend Tests

```bash
# Run all XBRL financial integration tests
cargo test --package econ-graph-sec-crawler --test "xbrl_financial_integration_tests"

# Run with verbose output
cargo test --package econ-graph-sec-crawler --test "xbrl_financial_integration_tests" -- --nocapture

# Run using the optimized test runner
./scripts/run-tests-optimized.sh
```

## Frontend Integration Tests

### Test Structure

The frontend integration tests are organized within the CI structure and use the existing test framework:

- **Configuration**: `frontend/ci/configs/xbrl-financial-integration.config.ts`
- **Setup**: `frontend/ci/configs/setup/xbrl-financial-global-setup.ts`
- **Teardown**: `frontend/ci/configs/setup/xbrl-financial-global-teardown.ts`

### Key Test Categories

#### 1. Financial Dashboard Integration
```typescript
describe('Financial Dashboard Integration', () => {
  it('should load and display company financial data from XBRL sources', async () => {
    // Test dashboard with XBRL data
  });
});
```

#### 2. Financial Statement Viewer Integration
```typescript
describe('Financial Statement Viewer Integration', () => {
  it('should display balance sheet data from XBRL parsing', async () => {
    // Test statement viewer with XBRL data
  });
});
```

#### 3. Benchmark Comparison Integration
```typescript
describe('Benchmark Comparison Integration', () => {
  it('should display industry benchmark data for XBRL-derived ratios', async () => {
    // Test benchmark comparison with XBRL ratios
  });
});
```

### Running Frontend Tests

```bash
# Run XBRL financial integration tests
npm run test:xbrl-financial:integration

# Run all integration tests
npm run test:integration

# Run with CI configuration
npx playwright test --config=ci/configs/xbrl-financial-integration.config.ts
```

## CI Integration

### Docker Compose Configuration

The XBRL financial integration tests are integrated into the main CI pipeline:

```yaml
# XBRL Financial Integration Tests
test-xbrl-financial:
  build:
    context: ../../
    dockerfile: ci/docker/Dockerfile.test-runner
  environment:
    - BACKEND_URL=http://backend:9876
    - FRONTEND_URL=http://frontend:18473
    - DATABASE_URL=postgresql://postgres:password@postgres:5432/econ_graph_test
    - REACT_APP_ENABLE_XBRL_FEATURES=true
    - REACT_APP_ENABLE_FINANCIAL_ANALYSIS=true
  command: ["npm", "run", "test:xbrl-financial:integration"]
  volumes:
    - ./test-results/xbrl-financial:/app/test-results
  depends_on:
    - frontend
    - backend
```

### Running in CI

```bash
# Run all test groups including XBRL financial
./ci/scripts/run-tests.sh --group all

# Run only XBRL financial integration tests
./ci/scripts/run-tests.sh --group xbrl-financial

# Run without cleanup (for debugging)
./ci/scripts/run-tests.sh --group xbrl-financial --no-cleanup
```

## Test Data Management

### Mock Data Strategy

Both backend and frontend tests use comprehensive mock data:

#### Backend Mock Data
- **Companies**: Real company data (Apple Inc., Chevron, etc.)
- **XBRL Instances**: Sample 10-K filings with realistic financial data
- **Taxonomy Schemas**: US-GAAP and company-specific schemas
- **Financial Facts**: Comprehensive balance sheet and income statement data

#### Frontend Mock Data
- **API Responses**: Mocked financial statements, ratios, and benchmark data
- **Component Props**: Realistic data for all financial UI components
- **Error Scenarios**: Mocked API errors for error handling tests

### Test Isolation

Each test is completely isolated:
- **Database**: Fresh testcontainers instance for each test
- **API Calls**: Mocked responses with predictable data
- **UI State**: Clean component state for each test
- **Browser**: Isolated browser context for frontend tests

## Performance Considerations

### Backend Tests
- **Parallel Execution**: Tests run in parallel using `#[serial]` only where necessary
- **Testcontainers**: Efficient container reuse and cleanup
- **Database Optimization**: Optimized queries and minimal test data

### Frontend Tests
- **Component Mocking**: Efficient mocking of heavy dependencies
- **Async Testing**: Proper async/await patterns for API calls
- **Performance Testing**: Tests for large dataset handling

## Error Handling and Debugging

### Backend Debugging
```bash
# Run with verbose output
RUST_LOG=debug cargo test --test xbrl_financial_integration_tests -- --nocapture

# Run specific test
cargo test --test xbrl_financial_integration_tests test_xbrl_taxonomy_schema_storage -- --nocapture
```

### Frontend Debugging
```bash
# Run with debugging output
npm run test:xbrl-financial:integration -- --verbose

# Run specific test file
npm run test:xbrl-financial:integration -- --testNamePattern="Financial Dashboard Integration"
```

### CI Debugging
```bash
# Run with no cleanup to inspect containers
./ci/scripts/run-tests.sh --group xbrl-financial --no-cleanup

# Access running container
docker exec -it <container_name> /bin/bash
```

## Best Practices

### Test Design
1. **Single Responsibility**: Each test focuses on one specific integration point
2. **Realistic Data**: Use realistic financial data that mirrors production scenarios
3. **Error Scenarios**: Test both success and failure paths
4. **Performance**: Include performance benchmarks for critical paths

### CI Integration
1. **Parallel Execution**: Tests run in parallel where possible
2. **Resource Management**: Efficient use of testcontainers and cleanup
3. **Reporting**: Comprehensive test reports with detailed failure information
4. **Artifacts**: Test results and screenshots saved for debugging

### Maintenance
1. **Regular Updates**: Keep mock data current with real financial reporting
2. **Schema Validation**: Ensure tests validate against current XBRL taxonomies
3. **Performance Monitoring**: Track test execution times and optimize as needed
4. **Documentation**: Keep test documentation current with implementation changes

## Conclusion

The XBRL financial CI integration tests provide comprehensive coverage of the entire XBRL processing pipeline, from SEC EDGAR data ingestion through financial analysis and visualization. The use of testcontainers ensures reliable, isolated testing while the CI integration provides automated validation of the complete system.

This structure supports:
- **Reliable Testing**: Consistent results across different environments
- **Comprehensive Coverage**: End-to-end validation of XBRL features
- **Efficient Execution**: Parallel testing with proper resource management
- **Easy Debugging**: Clear test structure and debugging capabilities
- **CI Integration**: Seamless integration with existing CI/CD pipeline
