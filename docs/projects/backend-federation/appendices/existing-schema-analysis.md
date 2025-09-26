# Existing Schema and Crawler Analysis

## Overview

This document analyzes the existing financial data schemas, crawler patterns, and GraphQL APIs to inform the design of the new financial data service for the federated backend architecture.

## Current Data Models

### 1. Economic Series Model

**Core Fields:**
- `id`: UUID primary key
- `source_id`: Reference to data source (FRED, BLS, etc.)
- `external_id`: External API identifier (e.g., FRED series ID)
- `title`: Human-readable series name
- `description`: Detailed description
- `units`: Measurement units (e.g., "Billions of Dollars", "Percent")
- `frequency`: Data frequency (Daily, Weekly, Monthly, Quarterly, Annual)
- `seasonal_adjustment`: Seasonal adjustment status
- `start_date`/`end_date`: Series date range
- `is_active`: Whether series is currently being updated
- `last_crawled_at`: Last successful data collection
- `crawl_status`/`crawl_error_message`: Crawler status tracking

**Key Characteristics:**
- Metadata-focused with rich descriptive information
- Built for time series data with frequency tracking
- Crawler integration with status monitoring
- Support for multiple data sources (FRED, BLS, Census, BEA, World Bank, IMF, etc.)

### 2. Data Points Model

**Core Fields:**
- `id`: UUID primary key
- `series_id`: Reference to economic series
- `date`: Observation date
- `value`: Numeric value (BigDecimal for precision)
- `revision_date`: When this value was published/revised
- `is_original_release`: Whether this is the first estimate or a revision

**Key Characteristics:**
- Time series data with revision tracking
- High precision numeric values using BigDecimal
- Support for data revisions (common in economic data)
- Optimized for time-based queries

### 3. Financial Statements Model (SEC EDGAR)

**Core Fields:**
- `id`: UUID primary key
- `company_id`: Reference to company
- `filing_type`: SEC form type (10-K, 10-Q, 8-K, etc.)
- `accession_number`: SEC filing identifier
- `filing_date`: When filed with SEC
- `period_end_date`: End of reporting period
- `fiscal_year`/`fiscal_quarter`: Reporting period
- `xbrl_processing_status`: XBRL processing state
- `xbrl_file_oid`: PostgreSQL Large Object reference
- `is_amended`/`is_restated`: Filing status flags

**Key Characteristics:**
- SEC EDGAR filing metadata
- XBRL processing integration
- Amendment and restatement tracking
- Large file storage using PostgreSQL Large Objects

### 4. Financial Line Items Model

**Core Fields:**
- `id`: UUID primary key
- `statement_id`: Reference to financial statement
- `taxonomy_concept`: XBRL taxonomy concept name
- `standard_label`/`custom_label`: Human-readable labels
- `value`: Financial amount (BigDecimal)
- `unit`: Currency or measurement unit
- `context_ref`: XBRL context reference
- `statement_type`/`statement_section`: Classification
- `precision`/`decimals`: Numeric precision
- `is_credit`/`is_debit`: Accounting classification

**Key Characteristics:**
- XBRL taxonomy integration
- Detailed financial statement line items
- Support for different statement types (Income, Balance Sheet, Cash Flow)
- Accounting classification support

## Current Crawler Architecture

### 1. Data Sources Supported

**Economic Data Sources:**
- FRED (Federal Reserve Economic Data)
- BLS (Bureau of Labor Statistics)
- Census Bureau
- BEA (Bureau of Economic Analysis)
- World Bank
- IMF (International Monetary Fund)
- ECB (European Central Bank)
- OECD
- Bank of England
- WTO
- Bank of Japan
- Reserve Bank of Australia
- Bank of Canada
- Swiss National Bank
- UN Statistics Division
- ILO (International Labour Organization)
- FHFA (Federal Housing Finance Agency)

**SEC EDGAR Sources:**
- XBRL filing downloads
- Company submissions crawling
- Taxonomy component downloads
- Financial statement parsing

### 2. Crawler Patterns

**Discovery Phase:**
- Series discovery by data source
- Metadata collection and validation
- Series registration in database

**Data Collection Phase:**
- Time series data download
- Bulk data processing
- Revision tracking
- Error handling and retry logic

**Processing Phase:**
- Data validation and cleaning
- Database insertion with batch operations
- Status tracking and error reporting

## Current GraphQL API

### 1. Economic Data Queries

```graphql
type EconomicSeries {
  id: ID!
  sourceId: ID!
  externalId: String!
  title: String!
  description: String
  units: String
  frequency: String!
  seasonalAdjustment: String
  lastUpdated: DateTime
  startDate: Date
  endDate: Date
  isActive: Boolean!
  createdAt: DateTime!
  updatedAt: DateTime!
  
  # Relationships
  source: DataSource
  recentDataPoints(limit: Int = 100): [DataPoint!]!
  dataPointCount: Int!
  dataPoints(filter: DataFilter, transformation: DataTransformation): [DataPoint!]!
}

type DataPoint {
  id: ID!
  seriesId: ID!
  date: Date!
  value: Decimal
  revisionDate: Date!
  isOriginalRelease: Boolean!
  createdAt: DateTime!
  updatedAt: DateTime!
}
```

### 2. Financial Statement Queries

```graphql
type FinancialStatement {
  id: ID!
  company: Company!
  filingType: String!
  periodEndDate: Date!
  fiscalYear: Int!
  fiscalQuarter: Int
  filingDate: Date!
  lineItems(filter: LineItemFilter): [FinancialLineItem!]!
  ratios: FinancialRatios
}

type FinancialLineItem {
  id: ID!
  statementId: ID!
  taxonomyConcept: String!
  standardLabel: String
  customLabel: String
  value: Decimal
  unit: String
  statementType: String!
  statementSection: String!
  precision: Int
  decimals: Int
  isCredit: Boolean
  isDebit: Boolean
}
```

### 3. Data Transformations

```graphql
enum DataTransformation {
  NONE
  YEAR_OVER_YEAR
  QUARTER_OVER_QUARTER
  MONTH_OVER_MONTH
  PERCENT_CHANGE
  LOG_DIFFERENCE
}
```

## Key Insights for Financial Data Service Design

### 1. Data Volume Characteristics

**Economic Time Series:**
- High frequency data (daily, monthly, quarterly)
- Long historical periods (decades of data)
- Frequent updates and revisions
- Large number of series (thousands)

**Financial Statements:**
- Lower frequency (quarterly, annual)
- Rich metadata and relationships
- Large file sizes (XBRL documents)
- Complex hierarchical structure

### 2. Access Patterns

**Read Patterns:**
- Time range queries (most common)
- Series metadata lookups
- Cross-series comparisons
- Historical revision analysis

**Write Patterns:**
- Bulk data ingestion from crawlers
- Batch updates for revisions
- New series discovery
- Error handling and retry logic

### 3. Performance Considerations

**Query Optimization:**
- Time-based indexing critical
- Series ID + date range queries
- Pagination for large result sets
- Data transformation calculations

**Storage Optimization:**
- Time series data compression
- Efficient revision storage
- Metadata vs. data separation
- Archive strategies for old data

## Recommendations for Financial Data Service

### 1. API Design Principles

**Maintain Existing Patterns:**
- Keep familiar GraphQL schema structure
- Preserve data transformation capabilities
- Support existing query patterns
- Maintain revision tracking

**Optimize for Time Series:**
- Efficient date range queries
- Bulk data operations
- Time-based pagination
- Series metadata caching

### 2. Database Architecture

**Time Series Optimization:**
- Consider specialized time series databases
- Parquet + Iceberg for large datasets
- Efficient compression for historical data
- Partitioning by time periods

**Metadata Management:**
- Fast series discovery
- Source integration tracking
- Crawler status monitoring
- Error handling and reporting

### 3. Crawler Integration

**Write API Design:**
- Bulk data ingestion endpoints
- Batch processing support
- Error handling and retry logic
- Status tracking and monitoring

**Data Validation:**
- Schema validation for incoming data
- Data quality checks
- Duplicate detection
- Revision conflict resolution

## Next Steps

1. **Design Financial Data Service GraphQL Schema** - Based on existing patterns
2. **Design Crawler Write API** - For bulk data ingestion
3. **Plan Database Migration Strategy** - From PostgreSQL to time series optimized storage
4. **Define Federation Schema** - How financial data integrates with user data
5. **Create Implementation Roadmap** - Phased approach to building the service
