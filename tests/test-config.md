# SEC Crawler Test Configuration

## Overview

This document provides comprehensive test configuration for the SEC Crawler system, including unit tests, integration tests, and end-to-end tests.

## Test Structure

```
tests/
├── unit/                    # Unit tests for individual components
│   ├── backend/            # Backend unit tests
│   │   ├── services/       # Service layer tests
│   │   ├── graphql/        # GraphQL API tests
│   │   └── models/         # Model tests
│   └── frontend/           # Frontend unit tests
│       ├── components/     # Component tests
│       ├── hooks/          # Hook tests
│       └── utils/          # Utility tests
├── integration/            # Integration tests
│   ├── api/               # API integration tests
│   ├── database/          # Database integration tests
│   └── workflow/          # End-to-end workflow tests
└── e2e/                   # End-to-end tests
    ├── admin-ui/          # Admin UI tests
    ├── main-ui/           # Main UI tests
    └── api/               # API tests
```

## Test Categories

### 1. Unit Tests

#### Backend Unit Tests

**Services Tests**
- `CompanyService` - Company search and management
- `FinancialStatementService` - Financial data operations
- `SecCrawlerService` - SEC crawling operations

**GraphQL Tests**
- Query resolvers
- Mutation resolvers
- Type definitions
- Error handling

**Model Tests**
- Data validation
- Serialization/deserialization
- Business logic

#### Frontend Unit Tests

**Component Tests**
- `CompanySearch` - Company search functionality
- `SecCrawlerManager` - Crawl management
- `FinancialDataViewer` - Financial data display

**Hook Tests**
- `useCompanySearch` - Company search logic
- `useSecCrawler` - Crawl operations
- `useFinancialData` - Financial data management

**Utility Tests**
- Data transformation
- Validation functions
- Helper functions

### 2. Integration Tests

#### API Integration Tests

**GraphQL API Tests**
- Query execution
- Mutation execution
- Error handling
- Performance testing

**Database Integration Tests**
- Connection testing
- Query performance
- Data integrity
- Transaction handling

#### Workflow Integration Tests

**End-to-End Workflow Tests**
- Company search → Crawl → Financial data
- Error handling and recovery
- Performance under load
- Concurrent operations

### 3. End-to-End Tests

#### Admin UI Tests

**Company Search Tests**
- Search functionality
- Result display
- Filtering and sorting
- Error handling

**Crawler Management Tests**
- Configuration
- Progress monitoring
- Result display
- Error handling

#### Main UI Tests

**Financial Data Tests**
- Data display
- Chart rendering
- Export functionality
- Performance

#### API Tests

**GraphQL API Tests**
- Query execution
- Mutation execution
- Error handling
- Rate limiting

## Test Configuration

### Backend Tests

#### Rust Tests

```toml
# Cargo.toml
[dev-dependencies]
tokio-test = "0.4"
mockito = "1.0"
tempfile = "3.0"
```

#### Test Database Setup

```rust
// tests/setup.rs
use econ_graph_core::database::create_pool;
use std::env;

pub async fn setup_test_database() -> DatabasePool {
    let database_url = env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/econ_graph_test".to_string());
    
    let pool = create_pool(&database_url).await.unwrap();
    
    // Run migrations
    run_migrations(&pool).await;
    
    pool
}

pub async fn cleanup_test_database(pool: &DatabasePool) {
    // Clean up test data
    sqlx::query("DELETE FROM financial_statements").execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM companies").execute(&pool).await.unwrap();
}
```

#### Test Data Setup

```rust
// tests/fixtures.rs
use econ_graph_core::models::Company;
use uuid::Uuid;
use chrono::Utc;

pub fn create_test_company() -> Company {
    Company {
        id: Uuid::new_v4(),
        cik: "0000320193".to_string(),
        ticker: Some("AAPL".to_string()),
        name: "Apple Inc.".to_string(),
        legal_name: Some("Apple Inc.".to_string()),
        industry: Some("Technology Hardware & Equipment".to_string()),
        sector: Some("Technology".to_string()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        // ... other fields
    }
}

pub fn create_test_financial_statement(company_id: Uuid) -> FinancialStatement {
    FinancialStatement {
        id: Uuid::new_v4(),
        company_id,
        statement_type: "10-K".to_string(),
        period: "2023".to_string(),
        fiscal_year: 2023,
        fiscal_quarter: None,
        start_date: "2023-01-01".parse().unwrap(),
        end_date: "2023-12-31".parse().unwrap(),
        currency: "USD".to_string(),
        total_revenue: Some(394328000000.0),
        net_income: Some(99803000000.0),
        total_assets: Some(352755000000.0),
        total_liabilities: Some(258549000000.0),
        shareholders_equity: Some(94206000000.0),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        // ... other fields
    }
}
```

### Frontend Tests

#### React Testing Library Setup

```typescript
// tests/setup.ts
import '@testing-library/jest-dom';
import { configure } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ThemeProvider, createTheme } from '@mui/material/styles';

// Configure testing library
configure({ testIdAttribute: 'data-testid' });

// Create test theme
export const testTheme = createTheme();

// Create test query client
export const createTestQueryClient = () => new QueryClient({
  defaultOptions: {
    queries: { retry: false },
    mutations: { retry: false },
  },
});

// Test wrapper
export const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const queryClient = createTestQueryClient();
  
  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider theme={testTheme}>
        {children}
      </ThemeProvider>
    </QueryClientProvider>
  );
};
```

#### Mock Setup

```typescript
// tests/mocks.ts
import { jest } from '@jest/globals';

// Mock GraphQL client
export const mockGraphQLClient = {
  request: jest.fn(),
  setHeader: jest.fn(),
  setHeaders: jest.fn(),
};

// Mock hooks
export const mockUseCompanySearch = {
  query: '',
  setQuery: jest.fn(),
  results: [],
  loading: false,
  error: null,
  searchCompanies: jest.fn(),
};

export const mockUseSecCrawler = {
  crawlCompany: jest.fn(),
  importRssFeed: jest.fn(),
  isCrawling: false,
  progress: 0,
  status: 'idle',
  error: null,
};

export const mockUseFinancialData = {
  data: [],
  loading: false,
  error: null,
  refetch: jest.fn(),
};
```

### End-to-End Tests

#### Playwright Configuration

```typescript
// playwright.config.ts
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: 'http://localhost:3000',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },
    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'] },
    },
  ],
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:3000',
    reuseExistingServer: !process.env.CI,
  },
});
```

#### Test Environment Setup

```bash
# tests/setup.sh
#!/bin/bash

# Start test database
docker run -d \
  --name econ_graph_test_db \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=econ_graph_test \
  -p 5433:5432 \
  postgres:15

# Wait for database to be ready
until docker exec econ_graph_test_db pg_isready -U postgres; do
  echo "Waiting for database..."
  sleep 1
done

# Run migrations
DATABASE_URL="postgres://postgres:password@localhost:5433/econ_graph_test" \
  cargo run --bin migrate

# Start backend server
DATABASE_URL="postgres://postgres:password@localhost:5433/econ_graph_test" \
  cargo run --bin backend &
BACKEND_PID=$!

# Start frontend server
npm run dev &
FRONTEND_PID=$!

# Wait for servers to be ready
until curl -f http://localhost:8000/health; do
  echo "Waiting for backend..."
  sleep 1
done

until curl -f http://localhost:3000; do
  echo "Waiting for frontend..."
  sleep 1
done

echo "Test environment ready!"
```

## Test Execution

### Running Tests

#### Backend Tests

```bash
# Run all backend tests
cargo test

# Run specific test categories
cargo test --test company_service
cargo test --test graphql_api
cargo test --test integration

# Run tests with coverage
cargo test --coverage
```

#### Frontend Tests

```bash
# Run all frontend tests
npm test

# Run specific test files
npm test -- CompanySearch.test.tsx
npm test -- SecCrawlerManager.test.tsx

# Run tests with coverage
npm test -- --coverage
```

#### End-to-End Tests

```bash
# Run all E2E tests
npx playwright test

# Run specific test files
npx playwright test sec_crawler_workflow.test.ts

# Run tests in headed mode
npx playwright test --headed

# Run tests in debug mode
npx playwright test --debug
```

### Test Data Management

#### Test Data Setup

```sql
-- tests/setup.sql
-- Create test companies
INSERT INTO companies (id, cik, ticker, name, legal_name, industry, sector, is_active, created_at, updated_at)
VALUES 
  ('123e4567-e89b-12d3-a456-426614174000', '0000320193', 'AAPL', 'Apple Inc.', 'Apple Inc.', 'Technology Hardware & Equipment', 'Technology', true, NOW(), NOW()),
  ('123e4567-e89b-12d3-a456-426614174001', '0000789019', 'MSFT', 'Microsoft Corporation', 'Microsoft Corporation', 'Software', 'Technology', true, NOW(), NOW()),
  ('123e4567-e89b-12d3-a456-426614174002', '0001018724', 'GOOGL', 'Alphabet Inc.', 'Alphabet Inc.', 'Internet Content & Information', 'Technology', true, NOW(), NOW());

-- Create test financial statements
INSERT INTO financial_statements (id, company_id, statement_type, period, fiscal_year, fiscal_quarter, start_date, end_date, currency, total_revenue, net_income, total_assets, total_liabilities, shareholders_equity, created_at, updated_at)
VALUES 
  ('123e4567-e89b-12d3-a456-426614174010', '123e4567-e89b-12d3-a456-426614174000', '10-K', '2023', 2023, NULL, '2023-01-01', '2023-12-31', 'USD', 394328000000, 99803000000, 352755000000, 258549000000, 94206000000, NOW(), NOW()),
  ('123e4567-e89b-12d3-a456-426614174011', '123e4567-e89b-12d3-a456-426614174001', '10-K', '2023', 2023, NULL, '2023-01-01', '2023-12-31', 'USD', 211915000000, 83383000000, 447704000000, 302433000000, 145271000000, NOW(), NOW());
```

#### Test Data Cleanup

```sql
-- tests/cleanup.sql
-- Clean up test data
DELETE FROM financial_statements WHERE company_id IN (
  SELECT id FROM companies WHERE name LIKE 'Test%'
);
DELETE FROM companies WHERE name LIKE 'Test%';
```

## Performance Testing

### Load Testing

#### Backend Load Tests

```rust
// tests/load/backend_load_test.rs
use tokio::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn test_concurrent_searches() {
    let pool = setup_test_database().await;
    let service = CompanyService::new(Arc::new(pool));
    
    let start = Instant::now();
    let success_count = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];
    
    // Spawn 100 concurrent searches
    for i in 0..100 {
        let service_clone = service.clone();
        let success_count_clone = success_count.clone();
        
        let handle = tokio::spawn(async move {
            let result = service_clone.search_companies("Apple", Some(10), Some(false)).await;
            if result.is_ok() {
                success_count_clone.fetch_add(1, Ordering::Relaxed);
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all searches to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    let duration = start.elapsed();
    let success_rate = success_count.load(Ordering::Relaxed) as f64 / 100.0;
    
    println!("Completed 100 concurrent searches in {:?}", duration);
    println!("Success rate: {:.2}%", success_rate * 100.0);
    
    assert!(duration < Duration::from_secs(10));
    assert!(success_rate > 0.95);
}
```

#### Frontend Load Tests

```typescript
// tests/load/frontend_load_test.ts
import { test, expect } from '@playwright/test';

test('should handle multiple concurrent searches', async ({ page }) => {
  await page.goto('http://localhost:3000/admin');
  
  const searchTerms = ['Apple', 'Microsoft', 'Google', 'Amazon', 'Tesla'];
  const promises = searchTerms.map(async (term) => {
    await page.fill('[data-testid="company-search-input"]', term);
    await page.click('[data-testid="search-button"]');
    await page.waitForSelector('[data-testid="search-results"]');
  });
  
  await Promise.all(promises);
  
  // Verify all searches completed successfully
  const results = await page.locator('[data-testid="company-result"]');
  expect(await results.count()).toBeGreaterThan(0);
});
```

### Stress Testing

#### Database Stress Tests

```rust
// tests/stress/database_stress_test.rs
#[tokio::test]
async fn test_database_connection_pool() {
    let pool = create_pool("postgres://...").await.unwrap();
    let max_connections = 20;
    
    let mut handles = vec![];
    
    // Spawn more connections than the pool can handle
    for i in 0..max_connections * 2 {
        let pool_clone = pool.clone();
        
        let handle = tokio::spawn(async move {
            let conn = pool_clone.get().await;
            if conn.is_ok() {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all connections to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify pool is still healthy
    assert!(pool.get().await.is_ok());
}
```

## Test Reporting

### Coverage Reports

#### Backend Coverage

```bash
# Generate coverage report
cargo test --coverage
cargo run --bin coverage-report
```

#### Frontend Coverage

```bash
# Generate coverage report
npm test -- --coverage
npm run coverage:report
```

### Test Results

#### JUnit Reports

```xml
<!-- tests/results/junit.xml -->
<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="SEC Crawler Tests" tests="150" failures="0" errors="0" time="45.2">
    <testcase name="CompanySearch Component" classname="components.CompanySearch" time="0.5"/>
    <testcase name="SecCrawlerManager Component" classname="components.SecCrawlerManager" time="0.8"/>
    <!-- ... more test cases ... -->
  </testsuite>
</testsuites>
```

#### HTML Reports

```html
<!-- tests/results/index.html -->
<!DOCTYPE html>
<html>
<head>
    <title>SEC Crawler Test Results</title>
    <style>
        .pass { color: green; }
        .fail { color: red; }
        .skip { color: orange; }
    </style>
</head>
<body>
    <h1>SEC Crawler Test Results</h1>
    <div class="summary">
        <p>Total Tests: 150</p>
        <p>Passed: 148</p>
        <p>Failed: 2</p>
        <p>Skipped: 0</p>
    </div>
    <!-- ... detailed results ... -->
</body>
</html>
```

## Continuous Integration

### GitHub Actions

```yaml
# .github/workflows/test.yml
name: Tests

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  backend-tests:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: password
          POSTGRES_DB: econ_graph_test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    
    steps:
    - uses: actions/checkout@v3
    - uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true
    
    - name: Run backend tests
      run: |
        export DATABASE_URL="postgres://postgres:password@localhost:5432/econ_graph_test"
        cargo test --verbose
    
    - name: Generate coverage
      run: |
        cargo test --coverage
        cargo run --bin coverage-report
    
    - name: Upload coverage
      uses: codecov/codecov-action@v3
      with:
        file: ./coverage/lcov.info

  frontend-tests:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    - uses: actions/setup-node@v3
      with:
        node-version: '18'
        cache: 'npm'
    
    - name: Install dependencies
      run: npm ci
    
    - name: Run frontend tests
      run: npm test -- --coverage --watchAll=false
    
    - name: Upload coverage
      uses: codecov/codecov-action@v3
      with:
        file: ./coverage/lcov.info

  e2e-tests:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    - uses: actions/setup-node@v3
      with:
        node-version: '18'
        cache: 'npm'
    
    - name: Install dependencies
      run: npm ci
    
    - name: Install Playwright
      run: npx playwright install --with-deps
    
    - name: Run E2E tests
      run: npx playwright test
    
    - name: Upload test results
      uses: actions/upload-artifact@v3
      if: always()
      with:
        name: playwright-report
        path: playwright-report/
```

## Conclusion

This comprehensive test configuration ensures that the SEC Crawler system is thoroughly tested at all levels, from individual components to end-to-end workflows. The test suite provides confidence in the system's reliability, performance, and maintainability.
