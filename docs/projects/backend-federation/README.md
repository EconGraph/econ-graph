# Backend Federation Architecture

## Overview

This project refactors the monolithic backend into a federated architecture using Apollo GraphQL Federation. The core motivation is to separate user interaction/data from financial series data, which is expensive to re-populate and frequently reset by developers.

## Architecture Vision

```
┌─────────────────────────────────────────┐
│           Apollo Gateway                │
├─────────────────────────────────────────┤
│  User Service (PostgreSQL)            │
│  Financial Data Service (Iceberg)      │
└─────────────────────────────────────────┘
```

### Key Principles
- **Separate Databases**: User data (PostgreSQL) vs Financial data (Iceberg/Parquet)
- **Production Financial Data**: Shared production subgraph for financial data
- **Private User Services**: Developers get their own user database
- **Clean Separation**: Financial data service only handles clean, validated data
- **Crawler Independence**: Crawler service manages raw data and transformation

## Implementation Plan

### Phase 1: Standalone Financial Data Service
- Build financial data service with Iceberg/Parquet storage
- Implement crawler write API and GraphQL read API
- Optional experimental federation with existing backend
- **Goal**: Validate architecture without disrupting existing system

### Phase 2: Full Federation
- Implement Apollo Federation
- Migrate user data to separate service
- Production financial data subgraph
- **Goal**: Complete separation of concerns

## Documentation Structure

### **Main Documents**
- **[V1 Implementation Plan](v1-implementation-plan.md)** - Detailed plan for Phase 1
- **[Research Summary](research-summary.md)** - Key research findings and decisions

### **Appendices**
- **[Storage Architecture](appendices/storage-architecture.md)** - Storage solutions and tiering
- **[API Design](appendices/api-design.md)** - Crawler and GraphQL APIs
- **[Storage Alternatives](appendices/storage-alternatives-analysis.md)** - Detailed storage analysis
- **[Automatic Tiering](appendices/automatic-storage-tiering-solutions.md)** - Tiering solutions
- **[Iceberg Manual Control](appendices/iceberg-manual-control.md)** - Manual Iceberg control
- **[Crawler API Design](appendices/crawler-write-api-design.md)** - Detailed crawler API
- **[Schema Analysis](appendices/existing-schema-analysis.md)** - Existing schema analysis
- **[Schema Separation](appendices/schema-separation-analysis.md)** - Schema separation design

## Key Decisions Made

1. **Database Architecture**: PostgreSQL for user data, Iceberg/Parquet for financial data
2. **Federation Strategy**: Apollo Supergraph for production, Schema Stitching for local dev
3. **Crawler Integration**: Separate service with write API using binary serialization
4. **Storage Tiering**: Local storage with manual tiering control
5. **Schema Evolution**: Financial data schema is stable, user schema is volatile

## Next Steps

1. Review V1 implementation plan
2. Begin development of financial data service
3. Implement crawler write API
4. Test with existing backend via optional federation
