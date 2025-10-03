# SEC Crawler Developer Guide

## Overview

This guide provides comprehensive information for developers working with the SEC Crawler system. It covers architecture, extension points, testing, and best practices for maintaining and enhancing the system.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Core Components](#core-components)
3. [Extension Points](#extension-points)
4. [Database Schema](#database-schema)
5. [API Development](#api-development)
6. [Frontend Development](#frontend-development)
7. [Testing Strategy](#testing-strategy)
8. [Performance Optimization](#performance-optimization)
9. [Deployment](#deployment)
10. [Troubleshooting](#troubleshooting)

## Architecture Overview

### System Components

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Admin UI      │    │   Main UI       │    │   API Gateway   │
│   (React)       │    │   (React)       │    │   (GraphQL)    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────┐
                    │   Backend API   │
                    │   (Rust)        │
                    └─────────────────┘
                                 │
                    ┌─────────────────┐
                    │   Database      │
                    │   (PostgreSQL)  │
                    └─────────────────┘
                                 │
                    ┌─────────────────┐
                    │   SEC Crawler  │
                    │   (Rust)       │
                    └─────────────────┘
```

### Technology Stack

- **Backend**: Rust with Actix Web, Diesel ORM, Async GraphQL
- **Database**: PostgreSQL with fulltext search indices
- **Frontend**: React with TypeScript, Material-UI, React Query
- **Crawler**: Rust with async/await, rate limiting, retry logic
- **API**: GraphQL with comprehensive error handling

## Core Components

### Backend Services

#### CompanyService

```rust
/// Company service for managing company data and search operations
pub struct CompanyService {
    pool: Arc<DatabasePool>,
}

impl CompanyService {
    /// Search companies with fulltext search and fuzzy matching
    pub async fn search_companies(
        &self,
        query: &str,
        limit: Option<i32>,
        include_inactive: Option<bool>,
    ) -> Result<Vec<Company>>;
    
    /// Get company by ID
    pub async fn get_company_by_id(&self, id: Uuid) -> Result<Option<Company>>;
    
    /// Get companies by ticker symbol
    pub async fn get_companies_by_ticker(&self, ticker: &str) -> Result<Vec<Company>>;
}
```

#### FinancialStatementService

```rust
/// Service for managing financial statement data
pub struct FinancialStatementService {
    pool: Arc<DatabasePool>,
}

impl FinancialStatementService {
    /// Get financial statements for a company
    pub async fn get_company_financial_statements(
        &self,
        company_id: Uuid,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<FinancialStatement>>;
    
    /// Count financial statements for a company
    pub async fn count_company_financial_statements(
        &self,
        company_id: Uuid,
    ) -> Result<i64>;
}
```

#### SecCrawlerService

```rust
/// Service for managing SEC crawling operations
pub struct SecCrawlerService {
    pool: Arc<DatabasePool>,
    crawler: Arc<SecCrawler>,
}

impl SecCrawlerService {
    /// Crawl SEC data for a specific company
    pub async fn crawl_company(&self, input: SecCrawlInput) -> Result<SecCrawlResult>;
    
    /// Import SEC RSS feed data
    pub async fn import_rss_feed(&self, input: SecRssImportInput) -> Result<SecRssImportResult>;
}
```

### GraphQL API

#### Schema Definition

```rust
/// GraphQL schema for SEC crawler operations
pub struct Query;

#[Object]
impl Query {
    /// Search for companies using fulltext search
    async fn search_companies(
        &self,
        ctx: &Context<'_>,
        input: CompanySearchInput,
    ) -> Result<CompanyConnection>;
    
    /// Get a specific company by ID
    async fn company(&self, ctx: &Context<'_>, id: ID) -> Result<Option<CompanyType>>;
    
    /// Get financial statements for a company
    async fn company_financial_statements(
        &self,
        ctx: &Context<'_>,
        company_id: ID,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<FinancialStatementConnection>;
}

pub struct Mutation;

#[Object]
impl Mutation {
    /// Trigger SEC crawl for a company
    async fn trigger_sec_crawl(
        &self,
        ctx: &Context<'_>,
        input: SecCrawlInput,
    ) -> Result<SecCrawlResult>;
    
    /// Import SEC RSS feed data
    async fn import_sec_rss(
        &self,
        ctx: &Context<'_>,
        input: SecRssImportInput,
    ) -> Result<SecRssImportResult>;
}
```

### Frontend Components

#### CompanySearch Component

```typescript
interface CompanySearchProps {
  onCompanySelect: (company: Company) => void;
  onCrawlStart: (company: Company) => void;
  searchFilters?: SearchFilters;
}

export const CompanySearch: React.FC<CompanySearchProps> = ({
  onCompanySelect,
  onCrawlStart,
  searchFilters,
}) => {
  // Component implementation
};
```

#### SecCrawlerManager Component

```typescript
interface SecCrawlerManagerProps {
  company: Company;
  onCrawlComplete: (result: SecCrawlResult) => void;
  onCrawlError: (error: Error) => void;
}

export const SecCrawlerManager: React.FC<SecCrawlerManagerProps> = ({
  company,
  onCrawlComplete,
  onCrawlError,
}) => {
  // Component implementation
};
```

## Extension Points

### Adding New Search Fields

To add new search fields to the company search:

1. **Update Database Schema**
   ```sql
   ALTER TABLE companies ADD COLUMN new_field TEXT;
   CREATE INDEX idx_companies_new_field_fts ON companies 
   USING gin(to_tsvector('english', new_field));
   ```

2. **Update Rust Models**
   ```rust
   #[derive(Queryable, Insertable, AsChangeset)]
   pub struct Company {
       // ... existing fields
       pub new_field: Option<String>,
   }
   ```

3. **Update Search Query**
   ```rust
   let search_query = format!(
       "to_tsvector('english', 
           COALESCE(name, '') || ' ' || 
           COALESCE(ticker, '') || ' ' || 
           COALESCE(cik, '') || ' ' || 
           COALESCE(legal_name, '') || ' ' ||
           COALESCE(new_field, '')
       ) @@ plainto_tsquery('english', $1)"
   );
   ```

4. **Update GraphQL Schema**
   ```rust
   #[derive(SimpleObject)]
   pub struct CompanyType {
       // ... existing fields
       pub new_field: Option<String>,
   }
   ```

### Adding New Financial Metrics

To add new financial metrics:

1. **Update Database Schema**
   ```sql
   ALTER TABLE financial_statements ADD COLUMN new_metric DECIMAL(15,2);
   ```

2. **Update Rust Models**
   ```rust
   #[derive(Queryable, Insertable, AsChangeset)]
   pub struct FinancialStatement {
       // ... existing fields
       pub new_metric: Option<Decimal>,
   }
   ```

3. **Update GraphQL Schema**
   ```rust
   #[derive(SimpleObject)]
   pub struct FinancialStatementType {
       // ... existing fields
       pub new_metric: Option<f64>,
   }
   ```

4. **Update Frontend Components**
   ```typescript
   interface FinancialStatement {
     // ... existing fields
     newMetric?: number;
   }
   ```

### Adding New Crawl Sources

To add new data sources:

1. **Create New Crawler Service**
   ```rust
   pub struct NewSourceCrawler {
       pool: Arc<DatabasePool>,
       client: reqwest::Client,
   }
   
   impl NewSourceCrawler {
       pub async fn crawl_data(&self, input: NewSourceInput) -> Result<NewSourceResult> {
           // Implementation
       }
   }
   ```

2. **Update SecCrawlerService**
   ```rust
   impl SecCrawlerService {
       pub async fn crawl_new_source(&self, input: NewSourceInput) -> Result<NewSourceResult> {
           let crawler = NewSourceCrawler::new(self.pool.clone());
           crawler.crawl_data(input).await
       }
   }
   ```

3. **Add GraphQL Mutations**
   ```rust
   #[Object]
   impl Mutation {
       async fn crawl_new_source(
           &self,
           ctx: &Context<'_>,
           input: NewSourceInput,
       ) -> Result<NewSourceResult> {
           // Implementation
       }
   }
   ```

## Database Schema

### Core Tables

#### Companies Table

```sql
CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cik VARCHAR(20) NOT NULL UNIQUE,
    ticker VARCHAR(10),
    name VARCHAR(255) NOT NULL,
    legal_name VARCHAR(255),
    sic_code VARCHAR(10),
    sic_description VARCHAR(255),
    industry VARCHAR(255),
    sector VARCHAR(255),
    business_address TEXT,
    mailing_address TEXT,
    phone VARCHAR(20),
    website VARCHAR(255),
    state_of_incorporation VARCHAR(10),
    state_of_incorporation_description VARCHAR(255),
    fiscal_year_end VARCHAR(10),
    entity_type VARCHAR(50),
    entity_size VARCHAR(50),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

#### Financial Statements Table

```sql
CREATE TABLE financial_statements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id),
    statement_type VARCHAR(50) NOT NULL,
    period VARCHAR(50) NOT NULL,
    fiscal_year INTEGER NOT NULL,
    fiscal_quarter INTEGER,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    currency VARCHAR(3) DEFAULT 'USD',
    total_revenue DECIMAL(15,2),
    net_income DECIMAL(15,2),
    total_assets DECIMAL(15,2),
    total_liabilities DECIMAL(15,2),
    shareholders_equity DECIMAL(15,2),
    -- ... additional financial fields
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

### Indices for Performance

```sql
-- Fulltext search indices
CREATE INDEX idx_companies_name_fts ON companies USING gin(to_tsvector('english', name));
CREATE INDEX idx_companies_ticker_fts ON companies USING gin(to_tsvector('english', ticker));
CREATE INDEX idx_companies_cik_fts ON companies USING gin(to_tsvector('english', cik));
CREATE INDEX idx_companies_legal_name_fts ON companies USING gin(to_tsvector('english', legal_name));

-- Combined fulltext search index
CREATE INDEX idx_companies_combined_fts ON companies USING gin(
    to_tsvector('english', 
        COALESCE(name, '') || ' ' || 
        COALESCE(ticker, '') || ' ' || 
        COALESCE(cik, '') || ' ' || 
        COALESCE(legal_name, '')
    )
);

-- Performance indices
CREATE INDEX idx_companies_industry ON companies(industry);
CREATE INDEX idx_companies_sector ON companies(sector);
CREATE INDEX idx_companies_is_active ON companies(is_active);
CREATE INDEX idx_financial_statements_company_id ON financial_statements(company_id);
CREATE INDEX idx_financial_statements_fiscal_year ON financial_statements(fiscal_year);
```

## API Development

### Adding New Endpoints

1. **Define Input/Output Types**
   ```rust
   #[derive(InputObject)]
   pub struct NewEndpointInput {
       pub field1: String,
       pub field2: Option<i32>,
   }
   
   #[derive(SimpleObject)]
   pub struct NewEndpointResult {
       pub success: bool,
       pub message: String,
       pub data: Option<String>,
   }
   ```

2. **Implement Service Method**
   ```rust
   impl NewService {
       pub async fn process_new_endpoint(
           &self,
           input: NewEndpointInput,
       ) -> Result<NewEndpointResult> {
           // Implementation
       }
   }
   ```

3. **Add GraphQL Resolver**
   ```rust
   #[Object]
   impl Query {
       async fn new_endpoint(
           &self,
           ctx: &Context<'_>,
           input: NewEndpointInput,
       ) -> Result<NewEndpointResult> {
           let service = ctx.data::<NewService>()?;
           service.process_new_endpoint(input).await
       }
   }
   ```

### Error Handling

```rust
#[derive(Error, Debug)]
pub enum SecCrawlerError {
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Authentication required")]
    AuthenticationRequired,
}

impl From<SecCrawlerError> for async_graphql::Error {
    fn from(err: SecCrawlerError) -> Self {
        async_graphql::Error::new(err.to_string())
    }
}
```

### Rate Limiting

```rust
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    max_requests: usize,
}

impl RateLimiter {
    pub fn new(max_requests: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_requests)),
            max_requests,
        }
    }
    
    pub async fn acquire(&self) -> Result<SemaphorePermit<'_>> {
        self.semaphore.acquire().await.map_err(|_| {
            SecCrawlerError::RateLimitExceeded
        })
    }
}
```

## Frontend Development

### Component Architecture

```typescript
// Base component interface
interface BaseComponentProps {
  className?: string;
  children?: React.ReactNode;
}

// Search component interface
interface SearchComponentProps extends BaseComponentProps {
  onSearch: (query: string) => void;
  onResultSelect: (result: any) => void;
  loading?: boolean;
  error?: string;
}

// Data component interface
interface DataComponentProps extends BaseComponentProps {
  data: any[];
  loading?: boolean;
  error?: string;
  onRefresh?: () => void;
}
```

### State Management

```typescript
// Custom hook for company search
export const useCompanySearch = () => {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<Company[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const searchCompanies = useCallback(async (searchQuery: string) => {
    setLoading(true);
    setError(null);
    
    try {
      const response = await searchCompaniesQuery({
        variables: { input: { query: searchQuery } }
      });
      
      setResults(response.data?.searchCompanies?.nodes || []);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, []);
  
  return {
    query,
    setQuery,
    results,
    loading,
    error,
    searchCompanies,
  };
};
```

### Error Handling

```typescript
// Error boundary component
export class ErrorBoundary extends React.Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }
  
  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }
  
  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Error caught by boundary:', error, errorInfo);
  }
  
  render() {
    if (this.state.hasError) {
      return (
        <div className="error-boundary">
          <h2>Something went wrong</h2>
          <p>{this.state.error?.message}</p>
          <button onClick={() => this.setState({ hasError: false, error: null })}>
            Try again
          </button>
        </div>
      );
    }
    
    return this.props.children;
  }
}
```

## Testing Strategy

### Testing Framework

This project uses **Vitest** for frontend testing, not Jest. Vitest provides:

- Fast test execution with Vite's build system
- Jest-compatible API for easy migration
- Built-in TypeScript support
- Excellent performance and developer experience

#### Key Differences from Jest

- Use `vi` instead of `jest` for mocking
- Import from `vitest` instead of `@jest/globals`
- Use `vi.fn()` instead of `jest.fn()`
- Use `vi.mock()` instead of `jest.mock()`

### Backend Testing

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::database::create_pool;
    
    #[tokio::test]
    async fn test_company_search() {
        let pool = create_pool("postgres://...").await.unwrap();
        let service = CompanyService::new(Arc::new(pool));
        
        let result = service.search_companies("Apple", Some(10), Some(false)).await;
        assert!(result.is_ok());
        
        let companies = result.unwrap();
        assert!(!companies.is_empty());
    }
    
    #[tokio::test]
    async fn test_company_search_fuzzy() {
        let pool = create_pool("postgres://...").await.unwrap();
        let service = CompanyService::new(Arc::new(pool));
        
        // Test fuzzy search with misspelling
        let result = service.search_companies("Appel", Some(10), Some(false)).await;
        assert!(result.is_ok());
        
        let companies = result.unwrap();
        // Should still find Apple Inc.
        assert!(companies.iter().any(|c| c.name.contains("Apple")));
    }
}
```

#### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use actix_web::test;
    
    #[actix_web::test]
    async fn test_graphql_search_companies() {
        let app = test::init_service(App::new().configure(configure)).await;
        
        let req = test::TestRequest::post()
            .uri("/graphql")
            .set_json(&serde_json::json!({
                "query": r#"
                    query SearchCompanies($input: CompanySearchInput!) {
                        searchCompanies(input: $input) {
                            nodes {
                                id
                                name
                                ticker
                            }
                        }
                    }
                "#,
                "variables": {
                    "input": {
                        "query": "Apple",
                        "limit": 10
                    }
                }
            }))
            .to_request();
        
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}
```

### Frontend Testing

#### Component Tests

```typescript
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi } from 'vitest';
import { CompanySearch } from '../CompanySearch';

describe('CompanySearch', () => {
  it('renders search input', () => {
    render(<CompanySearch onCompanySelect={vi.fn()} />);
    
    const searchInput = screen.getByPlaceholderText('Search companies...');
    expect(searchInput).toBeInTheDocument();
  });
  
  it('handles search input', async () => {
    const onCompanySelect = vi.fn();
    render(<CompanySearch onCompanySelect={onCompanySelect} />);
    
    const searchInput = screen.getByPlaceholderText('Search companies...');
    fireEvent.change(searchInput, { target: { value: 'Apple' } });
    
    await waitFor(() => {
      expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
    });
  });
});
```

#### Hook Tests

```typescript
import { renderHook, act } from '@testing-library/react';
import { vi } from 'vitest';
import { useCompanySearch } from '../useCompanySearch';

describe('useCompanySearch', () => {
  it('initializes with empty state', () => {
    const { result } = renderHook(() => useCompanySearch());
    
    expect(result.current.query).toBe('');
    expect(result.current.results).toEqual([]);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });
  
  it('handles search', async () => {
    const { result } = renderHook(() => useCompanySearch());
    
    act(() => {
      result.current.setQuery('Apple');
    });
    
    await act(async () => {
      await result.current.searchCompanies('Apple');
    });
    
    expect(result.current.results).toHaveLength(1);
    expect(result.current.results[0].name).toBe('Apple Inc.');
  });
});
```

### End-to-End Tests

```typescript
import { test, expect } from '@playwright/test';

test('complete SEC crawler workflow', async ({ page }) => {
  // Navigate to admin UI
  await page.goto('http://localhost:3000/admin');
  
  // Search for company
  await page.fill('[data-testid="company-search"]', 'Apple');
  await page.click('[data-testid="search-button"]');
  
  // Wait for results
  await page.waitForSelector('[data-testid="search-results"]');
  
  // Select company
  await page.click('[data-testid="company-result-0"]');
  
  // Start crawl
  await page.click('[data-testid="start-crawl"]');
  
  // Configure crawl
  await page.selectOption('[data-testid="form-types"]', '10-K');
  await page.fill('[data-testid="max-documents"]', '10');
  
  // Start crawl
  await page.click('[data-testid="start-crawl-button"]');
  
  // Wait for completion
  await page.waitForSelector('[data-testid="crawl-complete"]');
  
  // Verify results
  const result = await page.textContent('[data-testid="crawl-result"]');
  expect(result).toContain('Documents processed');
});
```

## Performance Optimization

### Database Optimization

1. **Query Optimization**
   ```sql
   -- Use EXPLAIN ANALYZE to identify slow queries
   EXPLAIN ANALYZE SELECT * FROM companies 
   WHERE to_tsvector('english', name) @@ plainto_tsquery('english', 'Apple');
   
   -- Add missing indices
   CREATE INDEX CONCURRENTLY idx_companies_name_fts 
   ON companies USING gin(to_tsvector('english', name));
   ```

2. **Connection Pooling**
   ```rust
   use deadpool_postgres::{Config, Runtime};
   
   let config = Config {
       host: Some("localhost".to_string()),
       port: Some(5432),
       user: Some("postgres".to_string()),
       password: Some("password".to_string()),
       dbname: Some("econ_graph".to_string()),
       pool: Some(deadpool_postgres::PoolConfig {
           max_size: 20,
           timeout: Some(std::time::Duration::from_secs(30)),
       }),
       ..Default::default()
   };
   ```

3. **Caching**
   ```rust
   use std::collections::HashMap;
   use std::sync::Arc;
   use tokio::sync::RwLock;
   
   pub struct Cache {
       data: Arc<RwLock<HashMap<String, Vec<Company>>>>,
   }
   
   impl Cache {
       pub async fn get(&self, key: &str) -> Option<Vec<Company>> {
           let data = self.data.read().await;
           data.get(key).cloned()
       }
       
       pub async fn set(&self, key: String, value: Vec<Company>) {
           let mut data = self.data.write().await;
           data.insert(key, value);
       }
   }
   ```

### Frontend Optimization

1. **Code Splitting**
   ```typescript
   import { lazy, Suspense } from 'react';
   
   const CompanySearch = lazy(() => import('./CompanySearch'));
   const SecCrawlerManager = lazy(() => import('./SecCrawlerManager'));
   
   export const AdminDashboard = () => (
     <Suspense fallback={<div>Loading...</div>}>
       <CompanySearch />
       <SecCrawlerManager />
     </Suspense>
   );
   ```

2. **Memoization**
   ```typescript
   import { memo, useMemo, useCallback } from 'react';
   
   export const CompanySearch = memo(({ onCompanySelect }) => {
     const searchResults = useMemo(() => {
       return companies.filter(company => 
         company.name.toLowerCase().includes(query.toLowerCase())
       );
     }, [companies, query]);
     
     const handleCompanySelect = useCallback((company) => {
       onCompanySelect(company);
     }, [onCompanySelect]);
     
     return (
       <div>
         {searchResults.map(company => (
           <CompanyCard 
             key={company.id} 
             company={company} 
             onSelect={handleCompanySelect}
           />
         ))}
       </div>
     );
   });
   ```

3. **Virtual Scrolling**
   ```typescript
   import { FixedSizeList as List } from 'react-window';
   
   export const CompanyList = ({ companies }) => (
     <List
       height={600}
       itemCount={companies.length}
       itemSize={80}
       itemData={companies}
     >
       {({ index, style, data }) => (
         <div style={style}>
           <CompanyCard company={data[index]} />
         </div>
       )}
     </List>
   );
   ```

## Deployment

### Docker Configuration

```dockerfile
# Backend Dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /usr/local/bin/
EXPOSE 8000
CMD ["backend"]
```

```dockerfile
# Frontend Dockerfile
FROM node:18-alpine as builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/nginx.conf
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: sec-crawler-backend
spec:
  replicas: 3
  selector:
    matchLabels:
      app: sec-crawler-backend
  template:
    metadata:
      labels:
        app: sec-crawler-backend
    spec:
      containers:
      - name: backend
        image: sec-crawler-backend:latest
        ports:
        - containerPort: 8000
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: database-secret
              key: url
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
```

### Environment Configuration

```bash
# .env
DATABASE_URL=postgres://user:password@localhost:5432/econ_graph
REDIS_URL=redis://localhost:6379
SEC_API_BASE_URL=https://www.sec.gov/Archives/edgar
RATE_LIMIT_REQUESTS_PER_SECOND=10
MAX_CONCURRENT_DOWNLOADS=5
CRAWL_TIMEOUT_SECONDS=300
```

## Troubleshooting

### Common Issues

1. **Database Connection Issues**
   ```bash
   # Check database connectivity
   psql -h localhost -U postgres -d econ_graph -c "SELECT 1;"
   
   # Check connection pool
   SELECT * FROM pg_stat_activity WHERE datname = 'econ_graph';
   ```

2. **Search Performance Issues**
   ```sql
   -- Check index usage
   EXPLAIN ANALYZE SELECT * FROM companies 
   WHERE to_tsvector('english', name) @@ plainto_tsquery('english', 'Apple');
   
   -- Rebuild indices if needed
   REINDEX INDEX idx_companies_name_fts;
   ```

3. **Memory Issues**
   ```bash
   # Monitor memory usage
   docker stats
   
   # Check for memory leaks
   valgrind --tool=memcheck ./backend
   ```

### Debugging Tools

1. **Backend Debugging**
   ```rust
   use tracing::{info, warn, error};
   
   #[tracing::instrument]
   pub async fn search_companies(&self, query: &str) -> Result<Vec<Company>> {
       info!("Searching companies with query: {}", query);
       
       let start = std::time::Instant::now();
       let result = self.perform_search(query).await?;
       let duration = start.elapsed();
       
       info!("Search completed in {:?}, found {} companies", duration, result.len());
       Ok(result)
   }
   ```

2. **Frontend Debugging**
   ```typescript
   import { useDebugValue } from 'react';
   
   export const useCompanySearch = () => {
     const [query, setQuery] = useState('');
     const [results, setResults] = useState<Company[]>([]);
     
     useDebugValue({ query, resultCount: results.length });
     
     return { query, setQuery, results, setResults };
   };
   ```

3. **Database Debugging**
   ```sql
   -- Enable query logging
   ALTER SYSTEM SET log_statement = 'all';
   ALTER SYSTEM SET log_min_duration_statement = 1000;
   SELECT pg_reload_conf();
   
   -- Check slow queries
   SELECT query, mean_time, calls 
   FROM pg_stat_statements 
   ORDER BY mean_time DESC 
   LIMIT 10;
   ```

## Conclusion

This developer guide provides comprehensive information for working with the SEC Crawler system. By following the patterns and practices outlined here, developers can effectively extend, maintain, and optimize the system. For additional support, refer to the API documentation and user guides, or contact the development team.
