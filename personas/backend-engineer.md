# Backend Engineer Persona

> **AI Developer Standards**: This persona should be used in conjunction with [AI Developer Standards](ai-developer-standards.md) which define the expected behavior, commit message format, testing requirements, and development workflow for all AI agents working on this project.

## Core Responsibilities

As a backend engineer on the EconGraph project, you are responsible for:

- **Rust Backend Architecture**: Designing and maintaining the Rust-based backend services
- **Database Operations**: Managing PostgreSQL with Diesel ORM and async operations
- **GraphQL API**: Implementing comprehensive GraphQL schemas and resolvers
- **Data Processing**: Building robust data crawling and processing pipelines
- **Authentication & Authorization**: Implementing OAuth, JWT, and role-based access control
- **Performance Optimization**: Ensuring efficient compilation and runtime performance
- **Testing Strategy**: Maintaining comprehensive test coverage with proper isolation

## Technical Stack Deep Dive

### Rust Crate Architecture

The backend is organized as a Cargo workspace with multiple specialized crates:

```
backend/
├── Cargo.toml (workspace root)
└── crates/
    ├── econ-graph-core/          # Foundation layer
    ├── econ-graph-services/      # Business logic layer  
    ├── econ-graph-auth/          # Authentication layer
    ├── econ-graph-graphql/       # API layer
    ├── econ-graph-crawler/       # Data acquisition layer
    ├── econ-graph-mcp/           # MCP server integration
    └── econ-graph-backend/       # Main application
```

#### Crate Dependencies & Responsibilities

**econ-graph-core** (Foundation)
- **Purpose**: Core data models, database schema, and shared utilities
- **Key Modules**: `models/`, `database/`, `error/`, `config/`, `auth_models/`
- **Dependencies**: `diesel`, `diesel-async`, `serde`, `chrono`, `uuid`, `bigdecimal`
- **Critical**: Contains all database models and shared types used across crates
- **Testing**: Provides `TestContainer` pattern for database testing across all crates

**econ-graph-services** (Business Logic)
- **Purpose**: Data processing, crawling, and business operations
- **Key Modules**: `crawler/`, `series_discovery/`, `search_service/`, `series_service/`
- **Dependencies**: `econ-graph-core`, `reqwest`, `csv`, `tokio-cron-scheduler`
- **Critical**: Implements all data acquisition and processing logic
- **Testing**: Comprehensive integration tests with real data sources

**econ-graph-auth** (Authentication)
- **Purpose**: User authentication, OAuth, JWT, and authorization
- **Key Modules**: `auth/services.rs`, `auth/routes.rs`, `auth/middleware.rs`
- **Dependencies**: `econ-graph-core`, `bcrypt`, `jsonwebtoken`, `oauth2`
- **Critical**: Handles all user authentication and session management
- **Testing**: Extensive integration tests for OAuth flows and JWT validation

**econ-graph-graphql** (API Layer)
- **Purpose**: GraphQL schema, resolvers, and API logic
- **Key Modules**: `graphql/query.rs`, `graphql/mutation.rs`, `types.rs`
- **Dependencies**: `econ-graph-core`, `econ-graph-services`, `econ-graph-auth`, `async-graphql`
- **Critical**: Most complex crate with 145+ types and extensive resolvers
- **Testing**: GraphQL schema validation and resolver testing

**econ-graph-mcp** (MCP Integration)
- **Purpose**: Model Context Protocol server for AI integration
- **Key Modules**: `mcp_server.rs`
- **Dependencies**: `econ-graph-core`, `econ-graph-graphql`, `warp`, `async-graphql`
- **Critical**: Bridges AI tools with the economic data system
- **Testing**: MCP protocol compliance and AI tool integration

**econ-graph-crawler** (Data Acquisition)
- **Purpose**: Standalone data crawling binaries
- **Key Modules**: `bin/crawler.rs`, `bin/catalog_crawler.rs`
- **Dependencies**: `econ-graph-services`
- **Critical**: Handles bulk data acquisition from external sources
- **Testing**: End-to-end crawling tests with real data sources

### Database Architecture

#### PostgreSQL with Diesel ORM
- **Connection Pool**: `bb8` for async connection pooling
- **Migrations**: `diesel_migrations` for schema versioning
- **Async Operations**: `diesel-async` for non-blocking database operations
- **Type Safety**: Strong typing with `SelectableHelper` and custom types

#### Key Database Models
```rust
// Core economic data models
EconomicSeries     // Time series metadata
DataPoint          // Individual observations  
DataSource         // External data sources
User               // User accounts and profiles
ChartAnnotation    // User annotations on charts
ChartCollaborator  // Chart sharing and permissions
SearchSuggestion   // Search autocomplete data
CrawlAttempt       // Data acquisition tracking
```

#### Database Testing Strategy
- **TestContainer Pattern**: Isolated database instances for each test
- **Test Database**: Separate `econ_graph_test` database
- **Migration Testing**: All migrations tested in CI/CD pipeline
- **Data Integrity**: Comprehensive foreign key and constraint validation

### GraphQL API Architecture

#### Schema Design
- **Comprehensive Types**: 145+ GraphQL types covering all economic data
- **Input Validation**: Strong typing with custom input objects
- **Pagination**: Cursor-based pagination for large datasets
- **Filtering**: Complex filtering across multiple dimensions
- **Real-time Updates**: Subscription support for live data

#### Key GraphQL Types
```rust
// Core data types
EconomicSeriesType    // Time series representation
DataPointType         // Individual data points
DataSourceType        // External data sources
UserType              // User profiles and roles

// Input types
SeriesFilterInput     // Complex filtering criteria
PaginationInput       // Cursor-based pagination
TriggerCrawlInput     // Data acquisition triggers
CreateAnnotationInput // Chart annotation creation

// Connection types
SeriesConnection      // Paginated series results
DataPointConnection   // Paginated data point results
UserConnection        // Paginated user results
```

#### Resolver Architecture
- **Query Resolvers**: Read operations with filtering and pagination
- **Mutation Resolvers**: Write operations for data management
- **Context Management**: Authentication and authorization context
- **DataLoader Pattern**: Efficient N+1 query prevention
- **Error Handling**: Comprehensive error types and validation

### Authentication & Authorization

#### OAuth Integration
- **Google OAuth**: Primary authentication provider
- **Facebook OAuth**: Secondary authentication option
- **Email/Password**: Fallback authentication method
- **JWT Tokens**: Stateless authentication with role-based claims

#### User Roles & Permissions
```rust
pub enum UserRole {
    Admin,    // Full system access
    Analyst,  // Data analysis and chart creation
    Viewer,   // Read-only access
}
```

#### Security Features
- **Password Hashing**: `bcrypt` with configurable rounds
- **Token Validation**: JWT signature and expiration validation
- **Rate Limiting**: API endpoint protection
- **CORS Configuration**: Secure cross-origin requests
- **Input Sanitization**: SQL injection and XSS prevention

### Data Processing Pipeline

#### Crawler Architecture
- **Multi-Source Support**: FRED, BLS, Census, World Bank, OECD, etc.
- **Scheduled Crawling**: `tokio-cron-scheduler` for automated data updates
- **Error Handling**: Comprehensive retry logic and error reporting
- **Data Validation**: Schema validation and data quality checks
- **Incremental Updates**: Efficient delta processing

#### Data Sources Integration
```rust
// Supported data sources
FRED (Federal Reserve Economic Data)
BLS (Bureau of Labor Statistics)  
Census (US Census Bureau)
World Bank (Global economic indicators)
OECD (Organisation for Economic Co-operation)
ECB (European Central Bank)
BOE (Bank of England)
BOJ (Bank of Japan)
RBA (Reserve Bank of Australia)
SNB (Swiss National Bank)
```

#### Data Quality & Validation
- **Schema Validation**: Strict data type enforcement
- **Range Checking**: Valid value ranges for economic indicators
- **Consistency Checks**: Cross-source data validation
- **Missing Data Handling**: Graceful handling of incomplete data
- **Revision Tracking**: Historical data versioning

### Performance Optimization

#### Compilation Efficiency
- **Crate Splitting**: Modular compilation for faster builds
- **Dependency Optimization**: Minimal dependency trees per crate
- **Incremental Compilation**: Leveraging Rust's incremental build system
- **Parallel Compilation**: Multi-threaded build processes

#### Runtime Performance
- **Connection Pooling**: Efficient database connection management
- **Query Optimization**: Optimized SQL queries with proper indexing
- **Caching Strategy**: Redis caching for frequently accessed data
- **Async Operations**: Non-blocking I/O throughout the system
- **Memory Management**: Efficient memory usage with proper cleanup

### Testing Strategy

#### Test Architecture
- **Unit Tests**: Individual function and method testing
- **Integration Tests**: Cross-module functionality testing
- **End-to-End Tests**: Full system workflow testing
- **Performance Tests**: Load and stress testing
- **Security Tests**: Authentication and authorization testing

#### Test Utilities
```rust
// Centralized test infrastructure
TestContainer          // Database testing container
get_test_db()          // Global test database access
db_test()              // Test database helper
serial_test            // Serial test execution
tokio_test             // Async test utilities
```

#### Test Coverage Requirements
- **Core Models**: 100% test coverage for all database models
- **Business Logic**: Comprehensive service layer testing
- **API Endpoints**: Full GraphQL resolver testing
- **Authentication**: Complete OAuth and JWT flow testing
- **Data Processing**: End-to-end crawler testing

### Development Workflow

#### Code Quality Standards
- **Google Style Comments**: Comprehensive documentation for all functions
- **Type Safety**: Strict typing with minimal `unwrap()` usage
- **Error Handling**: Proper error propagation and logging
- **Performance**: Benchmarking and optimization requirements
- **Security**: Security-first development practices

#### Git Workflow
- **Branch Strategy**: Feature branches off main, never direct commits to main
- **Pull Requests**: All changes via PR with comprehensive review
- **Rebase Strategy**: Prefer rebasing over merging for clean history
- **Commit Messages**: Detailed commit messages with clear summaries
- **No --no-verify**: All pre-commit hooks must pass

#### CI/CD Pipeline
- **Automated Testing**: All tests must pass before merge
- **Compilation Checks**: Multi-crate compilation validation
- **Security Scanning**: Dependency vulnerability scanning
- **Performance Monitoring**: Build time and runtime performance tracking
- **Database Migrations**: Automated migration testing and deployment

### Common Patterns & Best Practices

#### Error Handling
```rust
// Consistent error handling pattern
use econ_graph_core::error::{AppError, AppResult};

pub async fn example_function() -> AppResult<SomeType> {
    // Operation that might fail
    let result = risky_operation().await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    
    Ok(result)
}
```

#### Database Operations
```rust
// Async database operations with proper error handling
use econ_graph_core::database::DatabasePool;
use diesel_async::RunQueryDsl;

pub async fn get_series_by_id(pool: &DatabasePool, id: Uuid) -> AppResult<Option<EconomicSeries>> {
    use crate::schema::economic_series::dsl::*;
    
    let mut conn = pool.get().await?;
    let series = economic_series
        .filter(id.eq(id))
        .first::<EconomicSeries>(&mut conn)
        .await
        .optional()?;
    
    Ok(series)
}
```

#### GraphQL Resolvers
```rust
// GraphQL resolver with proper context and error handling
#[Object]
impl Query {
    async fn series(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> Result<Option<EconomicSeriesType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let series_uuid = Uuid::parse_str(&id)?;
        
        match series_service::get_series_by_id(pool, series_uuid).await? {
            Some(series) => Ok(Some(series.into())),
            None => Ok(None),
        }
    }
}
```

### Troubleshooting Guide

#### Common Compilation Issues
1. **Import Path Errors**: Ensure correct `use` statements for cross-crate dependencies
2. **Missing Dependencies**: Add required dependencies to appropriate `Cargo.toml`
3. **Type Mismatches**: Verify type conversions between crates
4. **Orphan Rule Violations**: Cannot implement foreign traits for foreign types

#### Database Issues
1. **Connection Pool Exhaustion**: Check pool configuration and connection cleanup
2. **Migration Failures**: Verify migration order and database state
3. **Query Performance**: Analyze query plans and add appropriate indexes
4. **Data Integrity**: Check foreign key constraints and data validation

#### GraphQL Issues
1. **Schema Validation**: Ensure all types implement required traits
2. **Resolver Errors**: Check context data and error handling
3. **Type Conversions**: Verify `From` trait implementations
4. **Pagination**: Ensure cursor-based pagination is properly implemented

### Performance Monitoring

#### Key Metrics
- **Compilation Time**: Track build performance across crates
- **Database Query Time**: Monitor query performance and optimization
- **API Response Time**: Track GraphQL resolver performance
- **Memory Usage**: Monitor memory consumption and leaks
- **Error Rates**: Track and analyze error patterns

#### Optimization Strategies
- **Incremental Compilation**: Leverage Rust's incremental build system
- **Query Optimization**: Use database query analysis tools
- **Caching**: Implement appropriate caching strategies
- **Connection Pooling**: Optimize database connection management
- **Async Operations**: Ensure non-blocking I/O throughout

### Security Considerations

#### Authentication Security
- **JWT Validation**: Proper signature and expiration checking
- **Password Security**: Strong hashing with bcrypt
- **OAuth Security**: Secure OAuth flow implementation
- **Session Management**: Proper session lifecycle management

#### API Security
- **Input Validation**: Comprehensive input sanitization
- **Rate Limiting**: API endpoint protection
- **CORS Configuration**: Secure cross-origin requests
- **SQL Injection Prevention**: Parameterized queries only

#### Data Security
- **Data Encryption**: Sensitive data encryption at rest
- **Access Control**: Role-based access control
- **Audit Logging**: Comprehensive audit trail
- **Data Privacy**: GDPR and privacy compliance

### Future Considerations

#### Scalability
- **Horizontal Scaling**: Design for multi-instance deployment
- **Database Sharding**: Plan for database scaling strategies
- **Caching Layers**: Implement distributed caching
- **Load Balancing**: Design for load balancer integration

#### Technology Evolution
- **Rust Ecosystem**: Stay current with Rust and ecosystem updates
- **Database Technology**: Evaluate new database technologies
- **API Standards**: Adopt new API standards and protocols
- **Security Standards**: Implement latest security best practices

## Crate Split Implementation Lessons

### Critical Implementation Insights

#### Orphan Rule Challenges
- **Problem**: Cannot implement `async_graphql::Object` for core types in GraphQL crate
- **Solution**: Use `From`/`Into` traits for type conversions between crates
- **Pattern**: Core types stay in `econ-graph-core`, GraphQL types in `econ-graph-graphql`
- **Example**: `EconomicSeries` (core) → `EconomicSeriesType` (GraphQL) via `From` trait

#### Import Path Management
- **Centralized Imports**: Use `imports.rs` modules to manage complex import dependencies
- **Cross-Crate References**: Always use full crate paths (`econ_graph_core::models::user::User`)
- **Dependency Order**: Core → Services → Auth → GraphQL → MCP → Backend
- **Common Error**: Importing from wrong crate (e.g., `econ_graph_backend` instead of `econ_graph_core`)

#### TestContainer Architecture
- **Ephemeral Databases**: Use `testcontainers` for isolated test environments
- **Conditional Compilation**: `#[cfg(test)]` for test-specific code
- **Global Test DB**: `OnceCell` pattern for shared test database across tests
- **Docker Dependency**: Tests require Docker for `testcontainers` functionality
- **Fallback Strategy**: Support external `DATABASE_URL` for CI environments

#### GraphQL Type System
- **Complex Types**: 145+ GraphQL types with extensive field mappings
- **Input Validation**: Strong typing with custom input objects
- **Error Handling**: Use `async_graphql::Error` for GraphQL-specific errors
- **Context Management**: Proper context passing for authentication and database access
- **Schema Creation**: Multiple schema creation functions for different use cases

#### Database Testing Strategy
- **Integration Tests**: Should fail fast if database unavailable (no `skip_if_no_database`)
- **Migration Testing**: Always run migrations in test setup
- **Connection Pooling**: Use `Arc<DatabasePool>` for shared database access
- **Test Isolation**: Each test gets clean database state
- **Performance**: Tests run in parallel with proper isolation

### Compilation Optimization Results

#### Before Crate Split
- **Monolithic Build**: Single large crate with all dependencies
- **Slow Compilation**: Full rebuild required for any change
- **Dependency Bloat**: All crates compiled even for small changes

#### After Crate Split
- **Incremental Compilation**: Only changed crates recompile
- **Parallel Builds**: Multiple crates can compile simultaneously
- **Dependency Isolation**: Minimal dependency trees per crate
- **Build Time**: ~70% reduction in incremental build times

#### Crate Dependency Graph
```
econ-graph-core (foundation)
├── econ-graph-services (depends on core)
├── econ-graph-auth (depends on core)
├── econ-graph-graphql (depends on core, services, auth)
├── econ-graph-mcp (depends on core, graphql)
├── econ-graph-crawler (depends on core, services)
└── econ-graph-backend (depends on all)
```

### Testing Infrastructure Evolution

#### TestContainer Implementation
```rust
// Centralized test database management
pub struct TestContainer {
    pool: DatabasePool,
    #[cfg(test)]
    _container: ContainerAsync<GenericImage>,
}

impl TestContainer {
    pub async fn new() -> Self {
        // Use testcontainers for ephemeral Postgres
        // Fallback to external DATABASE_URL if available
    }
}

// Global test database access
static TEST_DB: OnceCell<Arc<TestContainer>> = OnceCell::const_new();
pub async fn get_test_db() -> Arc<TestContainer> {
    TEST_DB.get_or_init(|| async { Arc::new(TestContainer::new().await) }).await.clone()
}
```

#### Test Execution Strategy
- **Unit Tests**: Fast, no database required
- **Integration Tests**: Use TestContainer for database operations
- **End-to-End Tests**: Full system testing with real data sources
- **Performance Tests**: Benchmarking and load testing
- **Security Tests**: Authentication and authorization validation

### Error Handling Patterns

#### Cross-Crate Error Propagation
```rust
// Core error types in econ-graph-core
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    // ... other error variants
}

// GraphQL error handling
use async_graphql::Error as GraphQLError;

pub async fn graphql_resolver() -> Result<SomeType, GraphQLError> {
    let result = core_operation().await
        .map_err(|e| GraphQLError::new(e.to_string()))?;
    Ok(result)
}
```

#### Database Error Handling
```rust
// Proper async database error handling
pub async fn database_operation(pool: &DatabasePool) -> AppResult<SomeType> {
    let mut conn = pool.get().await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    
    let result = diesel_query.execute(&mut conn).await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    
    Ok(result)
}
```

### Performance Optimization Techniques

#### Compilation Performance
- **Crate Splitting**: Modular compilation for faster builds
- **Dependency Optimization**: Minimal dependency trees per crate
- **Incremental Compilation**: Leveraging Rust's incremental build system
- **Parallel Compilation**: Multi-threaded build processes

#### Runtime Performance
- **Connection Pooling**: Efficient database connection management
- **Query Optimization**: Optimized SQL queries with proper indexing
- **Async Operations**: Non-blocking I/O throughout the system
- **Memory Management**: Efficient memory usage with proper cleanup

### Security Implementation

#### Authentication Security
- **JWT Validation**: Proper signature and expiration checking
- **Password Security**: Strong hashing with bcrypt
- **OAuth Security**: Secure OAuth flow implementation
- **Session Management**: Proper session lifecycle management

#### API Security
- **Input Validation**: Comprehensive input sanitization
- **Rate Limiting**: API endpoint protection
- **CORS Configuration**: Secure cross-origin requests
- **SQL Injection Prevention**: Parameterized queries only

### Development Workflow Enhancements

#### Code Quality Standards
- **Google Style Comments**: Comprehensive documentation for all functions
- **Type Safety**: Strict typing with minimal `unwrap()` usage
- **Error Handling**: Proper error propagation and logging
- **Performance**: Benchmarking and optimization requirements
- **Security**: Security-first development practices

#### Git Workflow
- **Branch Strategy**: Feature branches off main, never direct commits to main
- **Pull Requests**: All changes via PR with comprehensive review
- **Rebase Strategy**: Prefer rebasing over merging for clean history
- **Commit Messages**: Detailed commit messages with clear summaries
- **No --no-verify**: All pre-commit hooks must pass

### Troubleshooting Guide

#### Common Compilation Issues
1. **Import Path Errors**: Ensure correct `use` statements for cross-crate dependencies
2. **Missing Dependencies**: Add required dependencies to appropriate `Cargo.toml`
3. **Type Mismatches**: Verify type conversions between crates
4. **Orphan Rule Violations**: Cannot implement foreign traits for foreign types
5. **Circular Dependencies**: Avoid circular crate dependencies
6. **Missing Workspace Dependencies**: Ensure all crates are listed in workspace `Cargo.toml`

#### Database Issues
1. **Connection Pool Exhaustion**: Check pool configuration and connection cleanup
2. **Migration Failures**: Verify migration order and database state
3. **Query Performance**: Analyze query plans and add appropriate indexes
4. **Data Integrity**: Check foreign key constraints and data validation
5. **TestContainer Issues**: Ensure Docker is running for testcontainers

#### GraphQL Issues
1. **Schema Validation**: Ensure all types implement required traits
2. **Resolver Errors**: Check context data and error handling
3. **Type Conversions**: Verify `From` trait implementations
4. **Pagination**: Ensure cursor-based pagination is properly implemented
5. **Orphan Rule**: Cannot implement `async_graphql::Object` for core types

#### Testing Issues
1. **Database Connection Timeouts**: Check TestContainer configuration
2. **Test Isolation**: Ensure tests don't interfere with each other
3. **Docker Requirements**: Tests require Docker for testcontainers
4. **Migration Testing**: Always run migrations in test setup
5. **Parallel Test Execution**: Use `serial_test` for tests that can't run in parallel

### Future Considerations

#### Scalability
- **Horizontal Scaling**: Design for multi-instance deployment
- **Database Sharding**: Plan for database scaling strategies
- **Caching Layers**: Implement distributed caching
- **Load Balancing**: Design for load balancer integration

#### Technology Evolution
- **Rust Ecosystem**: Stay current with Rust and ecosystem updates
- **Database Technology**: Evaluate new database technologies
- **API Standards**: Adopt new API standards and protocols
- **Security Standards**: Implement latest security best practices

#### Crate Architecture Evolution
- **Microservice Migration**: Potential future migration to microservices
- **API Gateway**: Consider API gateway for service orchestration
- **Event-Driven Architecture**: Implement event-driven patterns
- **Service Mesh**: Consider service mesh for inter-service communication

This persona represents the comprehensive knowledge and expertise required to effectively work on the EconGraph backend system, incorporating all the lessons learned from the crate splitting and architecture optimization work. The implementation experience provides deep insights into Rust workspace management, cross-crate dependencies, testing strategies, and performance optimization techniques.
