# GitHub Issue: Missing Crawler Logs Backend Implementation

## 🚨 Critical Issue: Frontend/Backend Schema Mismatch

**Priority**: High  
**Type**: Bug / Missing Feature  
**Component**: Backend GraphQL API, Frontend Admin Interface  

## Problem Description

The admin frontend implements a complete crawler logs monitoring interface, but the backend has **zero support** for crawler logs. This is a major architectural mismatch that breaks the entire crawler logs functionality.

## Current State

### ✅ Frontend Implementation (Complete)
- `CrawlerLogs.tsx` component with full UI
- `useCrawlerLogs.ts` hook with GraphQL integration
- GraphQL queries defined in `src/services/graphql/queries.ts`:
  ```graphql
  query GetCrawlerLogs($limit: Int, $offset: Int, $level: String, $source: String, $search: String, $start_date: String, $end_date: String) {
    crawlerLogs(limit: $limit, offset: $offset, level: $level, source: $source, search: $search, start_date: $start_date, end_date: $end_date) {
      id
      timestamp
      level
      source
      message
      details
      duration_ms
      status
      created_at
    }
  }
  ```
- TypeScript types defined:
  ```typescript
  export interface LogEntry {
    id: string;
    timestamp: string;
    level: string;
    source: string;
    message: string;
    details?: string;
    duration_ms?: number;
    status: string;
    created_at: string;
  }
  ```
- Mock data and tests (currently disabled due to this issue)

### ❌ Backend Implementation (Missing Everything)
- **No GraphQL type**: No `LogEntryType` or `CrawlerLogType` in backend
- **No GraphQL resolver**: No `crawlerLogs` query resolver
- **No database schema**: No crawler logs table or model
- **No service layer**: No crawler logging service

## Impact

1. **Broken Admin Interface**: Crawler logs page is completely non-functional
2. **Test Failures**: All crawler logs tests fail with `Cannot read properties of undefined (reading 'filter')`
3. **Development Blocker**: Cannot test or develop crawler monitoring features
4. **Production Risk**: Admin interface shows broken functionality

## Investigation Results

### Backend GraphQL Schema Analysis
```rust
// Found in backend/crates/econ-graph-graphql/src/graphql/types.rs:
// ✅ EXISTS: CrawlerStatusType
pub struct CrawlerStatusType {
    pub is_running: bool,
    pub active_workers: i32,
    pub last_crawl: Option<DateTime<Utc>>,
    pub next_scheduled_crawl: Option<DateTime<Utc>>,
}

// ✅ EXISTS: QueueStatisticsType  
pub struct QueueStatisticsType {
    pub total_items: i32,
    pub pending_items: i32,
    pub processing_items: i32,
    pub completed_items: i32,
    pub failed_items: i32,
    pub retrying_items: i32,
    pub oldest_pending: Option<DateTime<Utc>>,
    pub average_processing_time: Option<f64>,
}

// ❌ MISSING: LogEntryType or CrawlerLogType
// ❌ MISSING: crawlerLogs query resolver
```

### Database Schema Analysis
- **Missing**: Crawler logs table
- **Missing**: Crawler log entries model
- **Missing**: Log level, source, message fields

## Required Implementation

### 1. Database Schema
```sql
CREATE TABLE crawler_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    level VARCHAR(20) NOT NULL, -- 'debug', 'info', 'warn', 'error', 'fatal'
    source VARCHAR(100) NOT NULL, -- 'FRED', 'BLS', 'Census', etc.
    message TEXT NOT NULL,
    details TEXT,
    duration_ms INTEGER,
    status VARCHAR(20) NOT NULL, -- 'success', 'failed', 'pending'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_crawler_logs_timestamp ON crawler_logs(timestamp DESC);
CREATE INDEX idx_crawler_logs_level ON crawler_logs(level);
CREATE INDEX idx_crawler_logs_source ON crawler_logs(source);
CREATE INDEX idx_crawler_logs_status ON crawler_logs(status);
```

### 2. Rust Models
```rust
// backend/crates/econ-graph-core/src/models/crawler_log.rs
#[derive(Queryable, Insertable, Clone, Debug)]
#[diesel(table_name = crawler_logs)]
pub struct CrawlerLog {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub source: String,
    pub message: String,
    pub details: Option<String>,
    pub duration_ms: Option<i32>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}
```

### 3. GraphQL Type
```rust
// backend/crates/econ-graph-graphql/src/graphql/types.rs
#[derive(Clone, SimpleObject)]
#[graphql(name = "CrawlerLog")]
pub struct CrawlerLogType {
    pub id: ID,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub source: String,
    pub message: String,
    pub details: Option<String>,
    pub duration_ms: Option<i32>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}
```

### 4. GraphQL Resolver
```rust
// backend/crates/econ-graph-graphql/src/graphql/query.rs
async fn crawler_logs(
    &self,
    ctx: &Context<'_>,
    limit: Option<i32>,
    offset: Option<i32>,
    level: Option<String>,
    source: Option<String>,
    search: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
) -> Result<Vec<CrawlerLogType>> {
    // Implementation needed
}
```

### 5. Service Layer
```rust
// backend/crates/econ-graph-core/src/services/crawler_log_service.rs
pub struct CrawlerLogService;

impl CrawlerLogService {
    pub async fn get_logs(
        pool: &DatabasePool,
        filters: CrawlerLogFilters,
    ) -> AppResult<Vec<CrawlerLog>> {
        // Implementation needed
    }
    
    pub async fn create_log(
        pool: &DatabasePool,
        log_entry: NewCrawlerLog,
    ) -> AppResult<CrawlerLog> {
        // Implementation needed
    }
}
```

## Questions to Resolve

1. **Logging Strategy**: How should crawler operations log their activities?
   - Real-time logging during crawl operations?
   - Batch logging after operations complete?
   - Integration with existing logging frameworks?

2. **Log Retention**: How long should logs be kept?
   - Database cleanup policies?
   - Log rotation strategies?

3. **Performance**: How to handle high-volume logging?
   - Async logging to prevent blocking crawler operations?
   - Log aggregation for performance metrics?

4. **Integration Points**: Where should logging be added?
   - Crawler worker processes?
   - Data source connectors?
   - Queue processing?

## Acceptance Criteria

- [ ] Database schema for crawler logs created
- [ ] Rust models and migrations implemented
- [ ] GraphQL type and resolver implemented
- [ ] Service layer for log management implemented
- [ ] Crawler operations actually log their activities
- [ ] Frontend crawler logs page works end-to-end
- [ ] Tests pass and are re-enabled
- [ ] Documentation updated

## Related Files

### Frontend (Ready)
- `admin-frontend/src/pages/CrawlerLogs.tsx`
- `admin-frontend/src/hooks/useCrawlerLogs.ts`
- `admin-frontend/src/services/graphql/queries.ts`
- `admin-frontend/src/pages/__tests__/CrawlerLogs.test.tsx` (disabled)

### Backend (Need Implementation)
- `backend/crates/econ-graph-core/src/models/crawler_log.rs` (create)
- `backend/crates/econ-graph-core/src/services/crawler_log_service.rs` (create)
- `backend/crates/econ-graph-graphql/src/graphql/types.rs` (add CrawlerLogType)
- `backend/crates/econ-graph-graphql/src/graphql/query.rs` (add crawler_logs resolver)
- Database migrations (create)

## Temporary Workaround

Currently, the `CrawlerLogs.test.tsx` has been disabled with:
```typescript
// DISABLED: Backend GraphQL schema mismatch - crawlerLogs query doesn't exist
describe.skip("CrawlerLogs", () => {
```

This should be re-enabled once the backend implementation is complete.

---

**Created**: [Date]  
**Assigned**: [Backend Team]  
**Labels**: `bug`, `backend`, `graphql`, `database`, `crawler`, `admin-interface`
