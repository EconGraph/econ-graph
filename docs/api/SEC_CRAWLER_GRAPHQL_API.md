# SEC Crawler GraphQL API Documentation

## Overview

The SEC Crawler GraphQL API provides comprehensive endpoints for managing SEC data crawling, company search, and financial statement retrieval. This API is designed to support both admin operations and end-user financial data access.

## Base URL

```
http://localhost:8000/graphql
```

## Authentication

All endpoints require authentication via JWT token in the Authorization header:

```
Authorization: Bearer <your-jwt-token>
```

## Schema

### Types

#### CompanyType

Represents a company in the SEC database.

```graphql
type CompanyType {
  id: ID!
  cik: String!
  ticker: String
  name: String!
  legalName: String
  sicCode: String
  sicDescription: String
  industry: String
  sector: String
  businessAddress: String
  mailingAddress: String
  phone: String
  website: String
  stateOfIncorporation: String
  stateOfIncorporationDescription: String
  fiscalYearEnd: String
  entityType: String
  entitySize: String
  isActive: Boolean!
  createdAt: String!
  updatedAt: String!
}
```

#### FinancialStatementType

Represents a financial statement for a company.

```graphql
type FinancialStatementType {
  id: ID!
  companyId: ID!
  statementType: FinancialStatementType!
  period: String!
  fiscalYear: Int!
  fiscalQuarter: Int
  startDate: String!
  endDate: String!
  currency: String!
  totalRevenue: Float
  netIncome: Float
  totalAssets: Float
  totalLiabilities: Float
  shareholdersEquity: Float
  operatingCashFlow: Float
  investingCashFlow: Float
  financingCashFlow: Float
  netCashFlow: Float
  earningsPerShare: Float
  bookValuePerShare: Float
  marketCap: Float
  priceToEarnings: Float
  priceToBook: Float
  returnOnEquity: Float
  returnOnAssets: Float
  debtToEquity: Float
  currentRatio: Float
  quickRatio: Float
  grossMargin: Float
  operatingMargin: Float
  netMargin: Float
  assetTurnover: Float
  inventoryTurnover: Float
  receivablesTurnover: Float
  payablesTurnover: Float
  workingCapital: Float
  freeCashFlow: Float
  capex: Float
  depreciation: Float
  amortization: Float
  goodwill: Float
  intangibleAssets: Float
  longTermDebt: Float
  shortTermDebt: Float
  cashAndEquivalents: Float
  accountsReceivable: Float
  inventory: Float
  accountsPayable: Float
  accruedExpenses: Float
  deferredRevenue: Float
  otherCurrentAssets: Float
  otherCurrentLiabilities: Float
  otherAssets: Float
  otherLiabilities: Float
  minorityInterest: Float
  preferredStock: Float
  commonStock: Float
  additionalPaidInCapital: Float
  retainedEarnings: Float
  accumulatedOtherComprehensiveIncome: Float
  treasuryStock: Float
  totalStockholdersEquity: Float
  totalLiabilitiesAndEquity: Float
  sharesOutstanding: Float
  weightedAverageShares: Float
  dilutedSharesOutstanding: Float
  stockPrice: Float
  dividendPerShare: Float
  dividendYield: Float
  payoutRatio: Float
  createdAt: String!
  updatedAt: String!
}
```

#### CompanySearchInput

Input for company search operations.

```graphql
input CompanySearchInput {
  query: String!
  limit: Int
  includeInactive: Boolean
}
```

#### SecCrawlInput

Input for triggering SEC crawls.

```graphql
input SecCrawlInput {
  companyId: ID!
  startDate: String
  endDate: String
  formTypes: [String!]
  maxDocuments: Int
  includeAmendments: Boolean
  includeExhibits: Boolean
  rateLimit: Int
  retryAttempts: Int
  timeout: Int
}
```

#### SecRssImportInput

Input for importing SEC RSS feeds.

```graphql
input SecRssImportInput {
  feedUrl: String!
  maxItems: Int
  includeAmendments: Boolean
  includeExhibits: Boolean
  rateLimit: Int
  retryAttempts: Int
  timeout: Int
}
```

#### SecCrawlResult

Result of SEC crawl operations.

```graphql
type SecCrawlResult {
  success: Boolean!
  message: String!
  documentsProcessed: Int!
  documentsSkipped: Int!
  documentsFailed: Int!
  totalSizeBytes: Int!
  processingTimeMs: Int!
  errors: [String!]
  warnings: [String!]
}
```

#### SecRssImportResult

Result of RSS import operations.

```graphql
type SecRssImportResult {
  success: Boolean!
  message: String!
  itemsProcessed: Int!
  itemsSkipped: Int!
  itemsFailed: Int!
  newCompanies: Int!
  updatedCompanies: Int!
  processingTimeMs: Int!
  errors: [String!]
  warnings: [String!]
}
```

#### CompanyConnection

Paginated company results.

```graphql
type CompanyConnection {
  nodes: [CompanyType!]!
  totalCount: Int!
  hasNextPage: Boolean!
  hasPreviousPage: Boolean!
  startCursor: String
  endCursor: String
}
```

#### FinancialStatementConnection

Paginated financial statement results.

```graphql
type FinancialStatementConnection {
  nodes: [FinancialStatementType!]!
  totalCount: Int!
  hasNextPage: Boolean!
  hasPreviousPage: Boolean!
  startCursor: String
  endCursor: String
}
```

## Queries

### searchCompanies

Search for companies using fulltext search with fuzzy matching.

```graphql
query SearchCompanies($input: CompanySearchInput!) {
  searchCompanies(input: $input) {
    nodes {
      id
      cik
      ticker
      name
      legalName
      industry
      sector
      isActive
    }
    totalCount
    hasNextPage
    hasPreviousPage
  }
}
```

**Variables:**
```json
{
  "input": {
    "query": "Apple",
    "limit": 10,
    "includeInactive": false
  }
}
```

**Response:**
```json
{
  "data": {
    "searchCompanies": {
      "nodes": [
        {
          "id": "123e4567-e89b-12d3-a456-426614174000",
          "cik": "0000320193",
          "ticker": "AAPL",
          "name": "Apple Inc.",
          "legalName": "Apple Inc.",
          "industry": "Technology Hardware & Equipment",
          "sector": "Technology",
          "isActive": true
        }
      ],
      "totalCount": 1,
      "hasNextPage": false,
      "hasPreviousPage": false
    }
  }
}
```

### company

Get a specific company by ID.

```graphql
query GetCompany($id: ID!) {
  company(id: $id) {
    id
    cik
    ticker
    name
    legalName
    industry
    sector
    website
    isActive
  }
}
```

**Variables:**
```json
{
  "id": "123e4567-e89b-12d3-a456-426614174000"
}
```

### companyFinancialStatements

Get financial statements for a company.

```graphql
query GetCompanyFinancialStatements($companyId: ID!, $limit: Int, $offset: Int) {
  companyFinancialStatements(companyId: $companyId, limit: $limit, offset: $offset) {
    nodes {
      id
      statementType
      period
      fiscalYear
      fiscalQuarter
      totalRevenue
      netIncome
      totalAssets
      totalLiabilities
      shareholdersEquity
    }
    totalCount
    hasNextPage
    hasPreviousPage
  }
}
```

**Variables:**
```json
{
  "companyId": "123e4567-e89b-12d3-a456-426614174000",
  "limit": 10,
  "offset": 0
}
```

## Mutations

### triggerSecCrawl

Trigger a SEC crawl for a specific company.

```graphql
mutation TriggerSecCrawl($input: SecCrawlInput!) {
  triggerSecCrawl(input: $input) {
    success
    message
    documentsProcessed
    documentsSkipped
    documentsFailed
    totalSizeBytes
    processingTimeMs
    errors
    warnings
  }
}
```

**Variables:**
```json
{
  "input": {
    "companyId": "123e4567-e89b-12d3-a456-426614174000",
    "startDate": "2023-01-01",
    "endDate": "2023-12-31",
    "formTypes": ["10-K", "10-Q", "8-K"],
    "maxDocuments": 100,
    "includeAmendments": true,
    "includeExhibits": false,
    "rateLimit": 10,
    "retryAttempts": 3,
    "timeout": 300
  }
}
```

**Response:**
```json
{
  "data": {
    "triggerSecCrawl": {
      "success": true,
      "message": "SEC crawl completed successfully",
      "documentsProcessed": 15,
      "documentsSkipped": 3,
      "documentsFailed": 0,
      "totalSizeBytes": 52428800,
      "processingTimeMs": 45000,
      "errors": [],
      "warnings": ["Some documents were skipped due to size limits"]
    }
  }
}
```

### importSecRss

Import SEC RSS feed data.

```graphql
mutation ImportSecRss($input: SecRssImportInput!) {
  importSecRss(input: $input) {
    success
    message
    itemsProcessed
    itemsSkipped
    itemsFailed
    newCompanies
    updatedCompanies
    processingTimeMs
    errors
    warnings
  }
}
```

**Variables:**
```json
{
  "input": {
    "feedUrl": "https://www.sec.gov/Archives/edgar/daily-index/xbrl/companyfacts/",
    "maxItems": 1000,
    "includeAmendments": true,
    "includeExhibits": false,
    "rateLimit": 10,
    "retryAttempts": 3,
    "timeout": 300
  }
}
```

**Response:**
```json
{
  "data": {
    "importSecRss": {
      "success": true,
      "message": "RSS import completed successfully",
      "itemsProcessed": 500,
      "itemsSkipped": 50,
      "itemsFailed": 5,
      "newCompanies": 25,
      "updatedCompanies": 475,
      "processingTimeMs": 120000,
      "errors": [],
      "warnings": ["Some items were skipped due to parsing errors"]
    }
  }
}
```

## Error Handling

The API provides comprehensive error handling with detailed error messages:

### Common Error Types

1. **Authentication Errors**
   ```json
   {
     "errors": [
       {
         "message": "Authentication required",
         "extensions": {
           "code": "UNAUTHENTICATED"
         }
       }
     ]
   }
   ```

2. **Validation Errors**
   ```json
   {
     "errors": [
       {
         "message": "Invalid input: companyId is required",
         "extensions": {
           "code": "VALIDATION_ERROR",
           "field": "companyId"
         }
       }
     ]
   }
   ```

3. **Rate Limiting Errors**
   ```json
   {
     "errors": [
       {
         "message": "Rate limit exceeded. Please try again later.",
         "extensions": {
           "code": "RATE_LIMITED",
           "retryAfter": 60
         }
       }
     ]
   }
   ```

4. **Database Errors**
   ```json
   {
     "errors": [
       {
         "message": "Database connection failed",
         "extensions": {
           "code": "DATABASE_ERROR"
         }
       }
     ]
   }
   ```

## Rate Limiting

The API implements rate limiting to prevent abuse:

- **Search Queries**: 100 requests per minute per user
- **Crawl Operations**: 10 requests per hour per user
- **RSS Import**: 5 requests per hour per user

Rate limit headers are included in responses:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1640995200
```

## Performance Considerations

### Query Optimization

1. **Use Specific Fields**: Only request the fields you need
2. **Implement Pagination**: Use limit/offset for large result sets
3. **Cache Results**: Cache frequently accessed data
4. **Batch Operations**: Use batch mutations when possible

### Best Practices

1. **Error Handling**: Always handle errors gracefully
2. **Loading States**: Show loading indicators for long operations
3. **Retry Logic**: Implement exponential backoff for failed requests
4. **Monitoring**: Monitor API usage and performance

## Examples

### Complete Workflow Example

1. **Search for a Company**
   ```graphql
   query SearchApple {
     searchCompanies(input: { query: "Apple", limit: 10, includeInactive: false }) {
       nodes {
         id
         name
         ticker
       }
     }
   }
   ```

2. **Get Company Details**
   ```graphql
   query GetAppleDetails($id: ID!) {
     company(id: $id) {
       id
       name
       ticker
       industry
       sector
     }
   }
   ```

3. **Trigger SEC Crawl**
   ```graphql
   mutation CrawlApple($input: SecCrawlInput!) {
     triggerSecCrawl(input: $input) {
       success
       message
       documentsProcessed
     }
   }
   ```

4. **Get Financial Statements**
   ```graphql
   query GetAppleFinancials($companyId: ID!) {
     companyFinancialStatements(companyId: $companyId, limit: 10) {
       nodes {
         statementType
         fiscalYear
         totalRevenue
         netIncome
       }
     }
   }
   ```

## Testing

### Test Queries

Use these test queries to verify API functionality:

```graphql
# Test company search
query TestSearch {
  searchCompanies(input: { query: "Microsoft", limit: 5 }) {
    nodes {
      id
      name
      ticker
    }
    totalCount
  }
}

# Test company retrieval
query TestCompany($id: ID!) {
  company(id: $id) {
    id
    name
    ticker
    isActive
  }
}
```

### Test Mutations

```graphql
# Test SEC crawl
mutation TestCrawl($input: SecCrawlInput!) {
  triggerSecCrawl(input: $input) {
    success
    message
  }
}

# Test RSS import
mutation TestRss($input: SecRssImportInput!) {
  importSecRss(input: $input) {
    success
    message
  }
}
```

## Support

For API support and questions:

- **Documentation**: This document and inline code documentation
- **Issues**: GitHub issues for bug reports and feature requests
- **Discussions**: GitHub discussions for general questions
- **Email**: Contact the development team for urgent issues

## Changelog

### Version 1.0.0
- Initial release of SEC Crawler GraphQL API
- Company search with fulltext indices
- SEC crawl operations
- RSS feed import
- Financial statement retrieval
- Comprehensive error handling
- Rate limiting and performance optimization
