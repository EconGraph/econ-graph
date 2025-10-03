# SEC Crawler Admin Implementation

## Overview

This document outlines the implementation of SEC crawler admin functionality for the EconGraph platform. The implementation provides comprehensive admin UI and API exposure for managing SEC EDGAR data crawling, including company search with fulltext capabilities and financial data visualization.

## Features Implemented

### 1. Enhanced GraphQL API

#### New Types Added
- `CompanyType` - GraphQL representation of company data
- `FinancialStatementType` - GraphQL representation of financial statements
- `CompanySearchInput` - Input for company search with fulltext capabilities
- `SecCrawlInput` - Input for triggering SEC crawls
- `SecRssImportInput` - Input for importing SEC EDGAR RSS feeds
- `SecCrawlResult` - Result of SEC crawl operations
- `SecRssImportResult` - Result of RSS import operations

#### New Queries
- `searchCompanies(input: CompanySearchInput!)` - Fulltext search for companies
- `company(id: ID!)` - Get specific company by ID
- `companyFinancialStatements(companyId: ID!, pagination: PaginationInput)` - Get financial statements for a company

#### New Mutations
- `triggerSecCrawl(input: SecCrawlInput!)` - Trigger SEC crawl for a specific company
- `importSecRss(input: SecRssImportInput!)` - Import SEC EDGAR RSS feed

### 2. Backend Services

#### CompanyService (`backend/src/services/company_service.rs`)
- Fulltext search using PostgreSQL indices
- Company CRUD operations
- Search by name, ticker, CIK, or legal name
- Industry and sector filtering
- Fuzzy search with spelling error tolerance

#### FinancialStatementService (`backend/src/services/financial_statement_service.rs`)
- Financial statement queries
- Date range filtering
- Filing type filtering
- Processing status tracking
- Pagination support

#### SecCrawlerService (`backend/src/services/sec_crawler_service.rs`)
- Integration with existing SEC crawler crate
- Company-specific crawl triggers
- RSS feed import functionality
- Crawl status monitoring
- Configuration management

### 3. Admin UI Components

#### CompanySearch (`admin-frontend/src/components/sec/CompanySearch.tsx`)
- Real-time company search with debouncing
- Fulltext search using PostgreSQL indices
- Fuzzy matching with spelling error tolerance
- Company selection and crawl triggering
- Search by name, ticker, or CIK
- Industry and sector information display

#### SecCrawlerManager (`admin-frontend/src/components/sec/SecCrawlerManager.tsx`)
- Step-by-step crawl configuration
- Company search and selection
- Crawl parameter configuration
- RSS feed import
- Crawl history and monitoring
- Status tracking and error handling

### 4. Frontend Components

#### FinancialDataViewer (`frontend/src/components/financial/FinancialDataViewer.tsx`)
- Financial statement visualization
- Company information display
- Interactive charts and tables
- Export capabilities
- Company comparison features
- Time-series analysis

### 5. React Hooks

#### useCompanySearch (`admin-frontend/src/hooks/useCompanySearch.ts`)
- GraphQL integration for company search
- Debounced search functionality
- Error handling and loading states
- Search result management

#### useSecCrawler (`admin-frontend/src/hooks/useSecCrawler.ts`)
- SEC crawler operations
- RSS import functionality
- Status monitoring
- Result tracking

#### useFinancialData (`frontend/src/hooks/useFinancialData.ts`)
- Company data loading
- Financial statement queries
- Data visualization support
- Error handling and caching

## Database Schema

### Companies Table
```sql
CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cik VARCHAR(10) NOT NULL UNIQUE,
    ticker VARCHAR(10),
    name VARCHAR(255) NOT NULL,
    legal_name VARCHAR(500),
    sic_code VARCHAR(4),
    sic_description VARCHAR(255),
    industry VARCHAR(100),
    sector VARCHAR(100),
    business_address JSONB,
    mailing_address JSONB,
    phone VARCHAR(50),
    website VARCHAR(255),
    state_of_incorporation VARCHAR(2),
    state_of_incorporation_description VARCHAR(100),
    fiscal_year_end VARCHAR(4),
    entity_type VARCHAR(50),
    entity_size VARCHAR(20),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### Financial Statements Table
```sql
CREATE TABLE financial_statements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id),
    filing_type VARCHAR(20) NOT NULL,
    form_type VARCHAR(20) NOT NULL,
    accession_number VARCHAR(50) NOT NULL UNIQUE,
    filing_date DATE NOT NULL,
    period_end_date DATE NOT NULL,
    fiscal_year INTEGER NOT NULL,
    fiscal_quarter INTEGER,
    document_type VARCHAR(50) NOT NULL,
    document_url TEXT NOT NULL,
    xbrl_file_oid INTEGER,
    xbrl_file_size_bytes BIGINT,
    xbrl_file_compressed BOOLEAN NOT NULL DEFAULT TRUE,
    xbrl_processing_status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## Fulltext Search Implementation

### PostgreSQL Indices
```sql
-- Fulltext search index for company names
CREATE INDEX idx_companies_name_fts ON companies USING gin(to_tsvector('english', name));

-- Fulltext search index for ticker symbols
CREATE INDEX idx_companies_ticker_fts ON companies USING gin(to_tsvector('english', ticker));

-- Fulltext search index for CIK
CREATE INDEX idx_companies_cik_fts ON companies USING gin(to_tsvector('english', cik));

-- Fulltext search index for legal names
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
```

### Search Query Implementation
```rust
// Company search with fulltext capabilities
let search_query = format!("%{}%", query);

let mut diesel_query = companies::table
    .filter(
        companies::name.ilike(&search_query)
            .or(companies::ticker.ilike(&search_query))
            .or(companies::cik.ilike(&search_query))
            .or(companies::legal_name.ilike(&search_query))
    )
    .order(companies::name.asc())
    .limit(limit);
```

## Usage Examples

### 1. Company Search
```typescript
// Search for companies using the admin UI
const { searchCompanies, companies, loading } = useCompanySearch();

await searchCompanies({
  query: "Apple",
  limit: 20,
  include_inactive: false
});
```

### 2. SEC Crawl Trigger
```typescript
// Trigger SEC crawl for a specific company
const { triggerCrawl } = useSecCrawler();

await triggerCrawl({
  cik: "0000320193", // Apple Inc.
  form_types: "10-K,10-Q",
  start_date: "2023-01-01",
  end_date: "2023-12-31",
  exclude_amended: false,
  exclude_restated: false,
  max_file_size: 52428800
});
```

### 3. RSS Import
```typescript
// Import SEC EDGAR RSS feed
const { importRss } = useSecCrawler();

await importRss({
  rss_url: undefined, // Use default SEC RSS
  max_filings: 100,
  form_types: "10-K,10-Q,8-K"
});
```

### 4. Financial Data Visualization
```typescript
// Display financial data for a company
const { company, financialStatements, loadCompany } = useFinancialData();

await loadCompany("company-uuid");
// Financial statements will be automatically loaded
```

## GraphQL Queries

### Search Companies
```graphql
query SearchCompanies($input: CompanySearchInput!) {
  searchCompanies(input: $input) {
    nodes {
      id
      cik
      ticker
      name
      industry
      sector
      is_active
    }
    total_count
    page_info {
      has_next_page
      has_previous_page
    }
  }
}
```

### Trigger SEC Crawl
```graphql
mutation TriggerSecCrawl($input: SecCrawlInput!) {
  triggerSecCrawl(input: $input) {
    operation_id
    cik
    filings_downloaded
    filings_processed
    errors
    start_time
    end_time
    status
  }
}
```

### Get Company Financial Statements
```graphql
query GetCompanyFinancialStatements($companyId: ID!, $pagination: PaginationInput) {
  companyFinancialStatements(companyId: $companyId, pagination: $pagination) {
    nodes {
      id
      filing_type
      filing_date
      period_end_date
      fiscal_year
      fiscal_quarter
      xbrl_processing_status
    }
    total_count
    page_info {
      has_next_page
      has_previous_page
    }
  }
}
```

## Integration with Existing SEC Crawler

The implementation integrates with the existing SEC crawler crate (`econ-graph-sec-crawler`) by:

1. **Service Layer Integration**: The `SecCrawlerService` wraps the existing `SecEdgarCrawler` functionality
2. **Configuration Management**: Crawl configurations are passed through to the existing crawler
3. **Result Processing**: Crawl results are processed and stored in the database
4. **Error Handling**: Comprehensive error handling and status tracking

## Future Enhancements

### 1. Advanced Search Features
- Industry-specific search filters
- Geographic location filtering
- Market cap filtering
- Financial metrics filtering

### 2. Enhanced Visualization
- Interactive financial charts
- Time-series analysis
- Peer comparison tools
- Financial ratio calculations

### 3. Automation Features
- Scheduled crawling
- Alert notifications
- Automated report generation
- Data quality monitoring

### 4. Performance Optimizations
- Search result caching
- Pagination improvements
- Background processing
- Database query optimization

## Testing

### Backend Tests
```bash
# Run company service tests
cargo test company_service

# Run financial statement service tests
cargo test financial_statement_service

# Run SEC crawler service tests
cargo test sec_crawler_service
```

### Frontend Tests
```bash
# Run admin frontend tests
cd admin-frontend
npm test

# Run main frontend tests
cd frontend
npm test
```

### Integration Tests
```bash
# Run GraphQL API tests
cargo test graphql_sec_crawler

# Run E2E tests
npm run test:e2e
```

## Deployment

### Environment Variables
```bash
# Database configuration
DATABASE_URL=postgres://postgres:password@localhost/econ_graph

# SEC crawler configuration
SEC_CRAWLER_USER_AGENT=EconGraph-SEC-Crawler/1.0
SEC_CRAWLER_RATE_LIMIT=10
SEC_CRAWLER_MAX_FILE_SIZE=52428800

# GraphQL endpoint
GRAPHQL_ENDPOINT=http://localhost:8080/graphql
```

### Docker Configuration
```yaml
# docker-compose.yml
services:
  backend:
    build: ./backend
    environment:
      - DATABASE_URL=postgres://postgres:password@postgres:5432/econ_graph
    depends_on:
      - postgres

  admin-frontend:
    build: ./admin-frontend
    environment:
      - REACT_APP_GRAPHQL_ENDPOINT=http://localhost:8080/graphql
    depends_on:
      - backend

  frontend:
    build: ./frontend
    environment:
      - REACT_APP_GRAPHQL_ENDPOINT=http://localhost:8080/graphql
    depends_on:
      - backend
```

## Conclusion

The SEC crawler admin implementation provides a comprehensive solution for managing SEC EDGAR data crawling with:

- **Fulltext Search**: PostgreSQL-based search with fuzzy matching
- **Admin UI**: Complete management interface for crawler operations
- **API Integration**: GraphQL API with comprehensive endpoints
- **Data Visualization**: Financial data display and analysis tools
- **Scalability**: Designed for production use with proper error handling

The implementation follows the established patterns in the EconGraph codebase and integrates seamlessly with the existing infrastructure.
