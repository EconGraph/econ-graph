# Financial Data Service Architecture

## Overview

The Financial Data Service is designed as a standalone microservice that provides GraphQL APIs for managing economic time series data. It's built with Rust and designed to integrate with the main EconGraph backend through Apollo Federation.

## Architecture Diagram

```
┌────────────────────────────────────────┐
│          Financial Data Service        │
├────────────────────────────────────────┤
│  GraphQL Read API (Apollo Server)     │
│  Clean Data Write API (REST/GraphQL)  │
│  Apache Parquet + Iceberg + Arrow    │
│  Clean Financial Time Series Data    │
└────────────────────────────────────────┘
```

## Components

### 1. GraphQL API Layer
- **Framework**: async-graphql with axum
- **Endpoints**: Query and Mutation operations
- **Features**: Type-safe schema, input validation, error handling

### 2. Data Models
- **EconomicSeries**: Clean financial series metadata
- **DataPoint**: Validated time series data points
- **DataSource**: Source information for series
- **Input Types**: Separate input types for mutations

### 3. Database Layer
- **Current**: In-memory storage with Arc<Mutex<>>
- **Future**: Apache Iceberg with MinIO storage tiering
- **Features**: Thread-safe operations, concurrent access

### 4. Crawler Integration
- **CleanDataPayload**: Bulk data ingestion
- **CleanDataResponse**: Processing confirmation
- **Features**: Raw data preservation, validation

## Data Flow

### 1. Data Ingestion
```
Crawler Service → Clean Data API → Database Layer → Storage
```

### 2. Data Querying
```
GraphQL Client → GraphQL API → Database Layer → Response
```

### 3. Data Processing
```
Raw Data → Validation → Clean Data → Storage → Query API
```

## Storage Architecture

### Current Implementation
- **Type**: In-memory HashMap with Arc<Mutex<>>
- **Features**: Thread-safe, concurrent access
- **Use Case**: Development and testing

### Future Implementation
- **Type**: Apache Iceberg with MinIO
- **Features**: 
  - Automatic storage tiering (NVMe → SSD → HDD → S3)
  - Schema evolution
  - Time travel
  - ACID transactions
- **Use Case**: Production data management

## API Design Principles

### 1. Clean Separation of Concerns
- **Financial Data Service**: Only clean, validated data
- **Crawler Service**: Raw data preservation and validation
- **No Mixed Concerns**: Crawler operational data separate from financial data

### 2. Type Safety
- **Rust Types**: Compile-time type checking
- **GraphQL Schema**: Runtime type validation
- **Input Validation**: Comprehensive validation for all inputs

### 3. Error Handling
- **GraphQL Errors**: Structured error responses
- **Validation Errors**: Clear validation messages
- **Internal Errors**: Proper error propagation

## Security Considerations

### Current
- **No Authentication**: Development only
- **No Authorization**: All operations allowed

### Future
- **JWT Authentication**: Token-based authentication
- **Role-Based Access**: Different access levels
- **API Rate Limiting**: Request throttling

## Performance Characteristics

### Memory Usage
- **In-Memory Storage**: Fast access, limited by RAM
- **Concurrent Access**: Thread-safe with Arc<Mutex<>>
- **Data Structures**: Optimized HashMap for O(1) lookups

### Query Performance
- **GraphQL**: Single request for multiple data points
- **Date Filtering**: Efficient range queries
- **Pagination**: Future enhancement for large datasets

## Integration Points

### 1. Apollo Federation
- **Supergraph**: Integration with main backend
- **Schema Stitching**: Local development overrides
- **Production**: Shared financial data subgraph

### 2. Crawler Services
- **Bulk Ingestion**: Efficient data loading
- **Data Validation**: Schema compliance
- **Error Handling**: Failed data preservation

### 3. Monitoring
- **Metrics**: Prometheus-compatible metrics
- **Logging**: Structured logging with tracing
- **Health Checks**: Service health monitoring

## Development Workflow

### 1. Local Development
```bash
# Start the service
cargo run

# Run tests
cargo test

# Run integration tests
cargo test --test integration
```

### 2. Testing
- **Unit Tests**: Individual component testing
- **Integration Tests**: End-to-end workflow testing
- **GraphQL Tests**: API contract testing

### 3. Deployment
- **Docker**: Containerized deployment
- **Kubernetes**: Orchestrated deployment
- **Monitoring**: Health checks and metrics

## Future Enhancements

### 1. Storage Tiering
- **MinIO Integration**: S3-compatible storage
- **Automatic Tiering**: Usage-based storage optimization
- **Cost Optimization**: Intelligent data placement

### 2. Advanced Analytics
- **Time Series Queries**: Specialized time series operations
- **Aggregations**: Statistical computations
- **Real-time Processing**: Stream processing capabilities

### 3. Federation
- **Apollo Supergraph**: Production federation
- **Schema Evolution**: Backward-compatible changes
- **Cross-Service Queries**: Unified data access

## Monitoring and Observability

### Metrics
- **Request Count**: API call frequency
- **Response Time**: Query performance
- **Error Rate**: Failure tracking
- **Data Volume**: Storage usage

### Logging
- **Structured Logs**: JSON-formatted logs
- **Request Tracing**: End-to-end request tracking
- **Error Context**: Detailed error information

### Health Checks
- **Service Health**: Basic availability
- **Database Health**: Storage connectivity
- **Dependency Health**: External service status

## Configuration

### Environment Variables
- **PORT**: Service port (default: 3001)
- **LOG_LEVEL**: Logging verbosity
- **DATABASE_URL**: Storage connection string

### Configuration Files
- **Cargo.toml**: Dependencies and metadata
- **Dockerfile**: Container configuration
- **docker-compose.yml**: Local development setup

## Troubleshooting

### Common Issues
1. **Port Conflicts**: Change port if 3001 is occupied
2. **Memory Issues**: Monitor memory usage for large datasets
3. **GraphQL Errors**: Check schema compatibility
4. **Database Errors**: Verify storage connectivity

### Debugging
- **Logs**: Check structured logs for errors
- **GraphQL Playground**: Test queries interactively
- **Health Endpoint**: Verify service status
- **Metrics**: Monitor performance indicators
