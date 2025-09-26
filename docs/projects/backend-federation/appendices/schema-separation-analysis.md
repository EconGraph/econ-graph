# Schema Separation Analysis: Decoupling Crawler Concerns

## Current Problem: Tightly Coupled Schemas

The existing schemas mix data concerns with crawler operational concerns, making it difficult to separate the financial data service from crawler management.

### Current Coupling Issues

#### 1. Economic Series Model - Crawler Fields Embedded
```rust
pub struct EconomicSeries {
    // Data fields
    pub id: Uuid,
    pub source_id: Uuid,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub units: Option<String>,
    pub frequency: String,
    pub seasonal_adjustment: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    
    // CRAWLER CONCERNS MIXED IN
    pub last_crawled_at: Option<DateTime<Utc>>,
    pub first_missing_date: Option<NaiveDate>,
    pub crawl_status: Option<String>,
    pub crawl_error_message: Option<String>,
}
```

#### 2. Separate Crawler Tables Still Coupled
- `crawl_attempts` table references `series_id`
- `crawl_queue` table has series-specific fields
- Crawler status tracking embedded in data models

#### 3. GraphQL API Exposes Crawler Concerns
```graphql
type EconomicSeries {
  # Data fields
  id: ID!
  title: String!
  description: String
  
  # CRAWLER CONCERNS EXPOSED
  lastCrawledAt: DateTime
  crawlStatus: String
  crawlErrorMessage: String
}
```

## Proposed Solution: Clean Separation of Concerns

### 1. Financial Data Service - Pure Data Models

**Economic Series (Data Only)**
```rust
pub struct EconomicSeries {
    // Core data fields only
    pub id: Uuid,
    pub source_id: Uuid,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub units: Option<String>,
    pub frequency: String,
    pub seasonal_adjustment: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // NO CRAWLER FIELDS
}
```

**Data Points (Data Only)**
```rust
pub struct DataPoint {
    pub id: Uuid,
    pub series_id: Uuid,
    pub date: NaiveDate,
    pub value: Option<BigDecimal>,
    pub revision_date: NaiveDate,
    pub is_original_release: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### 2. Crawler Service - Operational Concerns Only

**Crawler Job Management**
```rust
pub struct CrawlerJob {
    pub id: Uuid,
    pub series_id: Uuid,  // Reference to financial data
    pub source: String,
    pub status: CrawlStatus,
    pub priority: i32,
    pub scheduled_for: DateTime<Utc>,
    pub last_attempt: Option<DateTime<Utc>>,
    pub next_attempt: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct CrawlAttempt {
    pub id: Uuid,
    pub job_id: Uuid,  // Reference to crawler job
    pub series_id: Uuid,  // Reference to financial data
    pub attempted_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub success: bool,
    pub new_data_points: Option<i32>,
    pub error_message: Option<String>,
    pub response_time_ms: Option<i32>,
    pub data_size_bytes: Option<i32>,
    pub created_at: DateTime<Utc>,
}
```

### 3. Clean GraphQL APIs

**Financial Data Service API (Read-Only)**
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
  startDate: Date
  endDate: Date
  isActive: Boolean!
  createdAt: DateTime!
  updatedAt: DateTime!
  
  # Relationships
  source: DataSource
  dataPoints(filter: DataFilter): [DataPoint!]!
  dataPointCount: Int!
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

**Crawler Service API (Operational)**
```graphql
type CrawlerJob {
  id: ID!
  seriesId: ID!
  source: String!
  status: CrawlStatus!
  priority: Int!
  scheduledFor: DateTime!
  lastAttempt: DateTime
  nextAttempt: DateTime
  retryCount: Int!
  maxRetries: Int!
  errorMessage: String
  createdAt: DateTime!
  updatedAt: DateTime!
  
  # Relationships
  series: EconomicSeries  # Reference to financial data
  attempts: [CrawlAttempt!]!
}

type CrawlAttempt {
  id: ID!
  jobId: ID!
  seriesId: ID!
  attemptedAt: DateTime!
  completedAt: DateTime
  success: Boolean!
  newDataPoints: Int
  errorMessage: String
  responseTimeMs: Int
  dataSizeBytes: Int
  createdAt: DateTime!
}
```

## Benefits of Separation

### 1. Clear Service Boundaries
- **Financial Data Service**: Pure data storage and retrieval
- **Crawler Service**: Data collection and job management
- **User Service**: User interactions and preferences

### 2. Independent Scaling
- Financial data service can be optimized for read-heavy workloads
- Crawler service can be optimized for batch processing
- Each service can use appropriate database technologies

### 3. Simplified Federation
- Financial data service focuses on data queries
- No crawler concerns in the financial GraphQL schema
- Cleaner federation boundaries

### 4. Better Testing and Development
- Financial data service can be tested independently
- Crawler service can be developed separately
- Easier to mock dependencies

## Migration Strategy

### Phase 1: Create Clean Financial Data Service
1. **Extract Pure Data Models**
   - Remove all crawler fields from data models
   - Create clean GraphQL schema
   - Implement read-only API

2. **Separate Crawler Service**
   - Move crawler logic to separate service
   - Create crawler-specific models
   - Implement crawler management API

### Phase 2: Data Migration
1. **Migrate Existing Data**
   - Copy data to new clean schemas
   - Preserve crawler history in separate service
   - Update references

2. **Update Crawler Integration**
   - Modify crawlers to use new APIs
   - Update job scheduling
   - Implement new error handling

### Phase 3: Federation Integration
1. **Federate Financial Data Service**
   - Add to Apollo Supergraph
   - Implement federation resolvers
   - Test cross-service queries

2. **Keep Crawler Service Separate**
   - Crawler service remains independent
   - No federation needed for operational concerns
   - Direct API access for crawler management

## Implementation Plan

### 1. Financial Data Service Schema
```sql
-- Clean economic series table
CREATE TABLE economic_series (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_id UUID NOT NULL REFERENCES data_sources(id),
    external_id VARCHAR(255) NOT NULL,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    units VARCHAR(100),
    frequency VARCHAR(50) NOT NULL,
    seasonal_adjustment VARCHAR(100),
    start_date DATE,
    end_date DATE,
    is_active BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT now() NOT NULL,
    
    -- NO CRAWLER FIELDS
    UNIQUE(source_id, external_id)
);

-- Clean data points table
CREATE TABLE data_points (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id UUID NOT NULL REFERENCES economic_series(id),
    date DATE NOT NULL,
    value NUMERIC(20,6),
    revision_date DATE NOT NULL,
    is_original_release BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT now() NOT NULL,
    
    UNIQUE(series_id, date, revision_date)
);
```

### 2. Crawler Service Schema
```sql
-- Crawler job management
CREATE TABLE crawler_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id UUID NOT NULL,  -- Reference to financial data
    source VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL,
    priority INTEGER NOT NULL,
    scheduled_for TIMESTAMPTZ NOT NULL,
    last_attempt TIMESTAMPTZ,
    next_attempt TIMESTAMPTZ,
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,
    error_message TEXT,
    created_at TIMESTAMPTZ DEFAULT now() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT now() NOT NULL
);

-- Crawler attempt history
CREATE TABLE crawl_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES crawler_jobs(id),
    series_id UUID NOT NULL,  -- Reference to financial data
    attempted_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    success BOOLEAN NOT NULL,
    new_data_points INTEGER,
    error_message TEXT,
    response_time_ms INTEGER,
    data_size_bytes INTEGER,
    created_at TIMESTAMPTZ DEFAULT now() NOT NULL
);
```

## Next Steps

1. **Design Clean Financial Data Service API** - Based on existing patterns but without crawler concerns
2. **Design Crawler Write API** - For bulk data ingestion from crawler service
3. **Plan Data Migration** - From coupled schemas to separated concerns
4. **Update V1 Implementation Plan** - Focus on clean separation from the start
