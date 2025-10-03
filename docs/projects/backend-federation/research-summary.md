# Research Summary

## Overview

This document summarizes the research findings and key decisions made during the backend federation architecture planning phase.

## Key Research Areas

### **Apollo Federation Architecture**
- **Apollo Supergraph**: Production federation with shared financial data
- **Schema Stitching**: Local development with production financial data
- **Cross-Environment Access**: Secure access to production data from dev environments

### **Database Architecture**
- **User Data**: PostgreSQL with Diesel ORM (existing)
- **Financial Data**: Apache Iceberg + Parquet (new)
- **Separation**: Different databases for different data types

### **Storage Solutions**
- **Primary**: Apache Iceberg + Parquet for time series data
- **Tiering**: Manual control with local storage (NVMe → SSD → HDD)
- **Alternatives**: S3 Intelligent Tiering for cloud scale

### **Crawler Integration**
- **Binary Serialization**: Protocol Buffers for bulk operations
- **Dual Architecture**: Separate Iceberg instances for crawler and financial data
- **Raw Data Preservation**: Crawler service maintains raw data archive

### **Query Languages**
- **GraphQL**: Primary API for user queries
- **OData**: Future consideration for analytical queries
- **SQL**: For complex analytics with Iceberg

## Key Decisions Made

1. **Federation Strategy**: Apollo Supergraph for production, Schema Stitching for local dev
2. **Database Separation**: PostgreSQL for user data, Iceberg for financial data
3. **Storage Architecture**: Local storage with manual tiering control
4. **Crawler Design**: Separate service with dual Iceberg architecture
5. **Schema Evolution**: Financial data schema is stable, user schema is volatile

## Technology Stack

### **Core Technologies**
- **Apollo Federation**: GraphQL federation
- **Apache Iceberg**: Table format for financial data
- **Apache Parquet**: Columnar storage format
- **Protocol Buffers**: Binary serialization for bulk operations
- **PostgreSQL**: User data storage
- **Rust**: Backend implementation language

### **Storage Solutions**
- **Local Storage**: MinIO or custom Parquet manager
- **Tiering**: Manual control with frequency-based analysis
- **Cloud**: S3 Intelligent Tiering for production scale

## Implementation Phases

### **Phase 1**: Standalone Financial Service
- Build financial data service with Iceberg/Parquet
- Implement crawler write API and GraphQL read API
- Optional experimental federation with existing backend

### **Phase 2**: Full Federation
- Implement Apollo Federation
- Migrate user data to separate service
- Production financial data subgraph

## Benefits

1. **Performance**: Optimized storage for each data type
2. **Scalability**: Services can evolve independently
3. **Cost Control**: Manual tiering based on usage patterns
4. **Data Integrity**: Raw data preserved, clean data validated
5. **Developer Experience**: Private user services with shared financial data

## Next Steps

1. Review V1 implementation plan
2. Begin development of financial data service
3. Implement crawler write API
4. Test with existing backend via optional federation
