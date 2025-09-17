# Database Integration Testing with TestContainers

This document describes the comprehensive database integration testing setup for the EconGraph backend using `testcontainers` for real PostgreSQL database testing.

## Overview

The EconGraph backend uses `testcontainers` to provide real PostgreSQL database instances for integration testing. This approach ensures that our tests run against actual database behavior rather than mocks, providing higher confidence in data layer functionality.

## Key Benefits

### ✅ **Real Database Testing**
- Tests run against actual PostgreSQL instances
- Validates real SQL queries and database constraints  
- Catches database-specific issues that mocks might miss
- Tests actual transaction behavior and isolation levels

### ✅ **Isolation and Reproducibility**
- Each test gets a clean database state
- Tests can run in parallel without interference
- Consistent test environment across different machines
- Docker-based setup works on any platform with Docker

### ✅ **Comprehensive Coverage**
- Tests all CRUD operations with real persistence
- Validates database constraints and foreign keys
- Tests complex queries and aggregations
- Verifies concurrent access patterns

## Architecture

### Test Infrastructure Components

```
┌─────────────────────┐
│   Test Runner       │
│   (Cargo Test)      │
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│   TestContainer     │
│   - PostgreSQL      │
│   - Migrations      │
│   - Seed Data       │
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│   Database Pool     │
│   - Connection Mgmt │
│   - Transaction     │
│   - Query Execution │
└─────────────────────┘
```

### Key Files

- `src/test_utils.rs` - Core testing infrastructure
- `src/models/*/tests.rs` - Model-specific integration tests  
- `src/integration_tests.rs` - End-to-end integration tests
- `Cargo.toml` - TestContainers dependencies

## Test Infrastructure

### TestContainer Setup

```rust
pub struct TestContainer {
    container: Container<'static, Postgres>,
    pub database_url: String,
    pub pool: Pool,
}

impl TestContainer {
    pub async fn new() -> Self {
        // Creates PostgreSQL container with test configuration
        // Runs database migrations automatically
        // Sets up connection pool for tests
    }
}
```

### Database Test Macro

```rust
db_test!(test_name, |container: Arc<TestContainer>| async move {
    // Test body with automatic setup and cleanup
    let pool = container.pool();
    // ... test implementation
});
```

**Features:**
- Automatic container setup and teardown
- Database state cleaning between tests
- Connection pool management
- Panic handling and cleanup

### Helper Traits

```rust
#[async_trait]
pub trait DatabaseTestExt {
    async fn execute_count(&self, query: &str) -> i64;
    async fn table_exists(&self, table_name: &str) -> bool;
    async fn table_row_count(&self, table_name: &str) -> i64;
}
```

## Test Categories

### 1. Model CRUD Tests

**Location:** `src/models/*/tests.rs`

**Coverage:**
- Create operations with validation
- Read operations with filtering and searching
- Update operations with constraint checking
- Delete operations with cascade behavior
- Unique constraint validation
- Foreign key relationship testing

**Example:**
```rust
db_test!(test_create_data_source, |container: Arc<TestContainer>| async move {
    let pool = container.pool();
    let conn = pool.get().await.expect("Failed to get connection");
    
    let new_source = NewDataSource {
        name: "Federal Reserve Economic Data".to_string(),
        base_url: "https://api.stlouisfed.org/fred".to_string(),
        // ... other fields
    };
    
    let created_source: DataSource = conn.interact(move |conn| {
        diesel::insert_into(data_sources::table)
            .values(&new_source)
            .get_result(conn)
    }).await.expect("Failed to interact").expect("Failed to create data source");
    
    // Assertions and verification
    assert_eq!(created_source.name, "Federal Reserve Economic Data");
    // ... more assertions
});
```

### 2. Queue Processing Tests

**Location:** `src/models/crawl_queue/tests.rs`

**Coverage:**
- SKIP LOCKED concurrent processing
- Priority-based queue ordering  
- Status transitions through job lifecycle
- Retry logic with backoff
- Scheduled job processing
- Queue cleanup operations

**Key Test:**
```rust
db_test!(test_skip_locked_queue_processing, |container: Arc<TestContainer>| async move {
    // Simulates concurrent workers using SKIP LOCKED
    let worker1_item = conn.interact(|conn| {
        diesel::sql_query(
            "UPDATE crawl_queue 
             SET status = 'processing', worker_id = 'worker1'
             WHERE id = (
                 SELECT id FROM crawl_queue 
                 WHERE status = 'pending' 
                 ORDER BY priority DESC, created_at ASC 
                 FOR UPDATE SKIP LOCKED 
                 LIMIT 1
             )
             RETURNING *"
        ).get_result::<CrawlQueueItem>(conn).optional()
    }).await?;
    
    // Verify workers get different items
    assert!(worker1_item.is_some());
    // ... concurrent access verification
});
```

### 3. Time Series Data Tests  

**Location:** `src/models/data_point/tests.rs`

**Coverage:**
- Time series data storage and retrieval
- Date range filtering and queries
- Original vs. revised data tracking
- Null value handling for missing data
- Data aggregation and statistics
- Time series continuity and gap detection

### 4. Integration Tests

**Location:** `src/integration_tests.rs`

**Coverage:**
- End-to-end workflow testing
- Cross-table relationship validation
- Concurrent database access patterns
- Performance characteristics with larger datasets
- Database constraint enforcement
- Foreign key integrity

## Running Tests

### Prerequisites

```bash
# Install Docker (required for testcontainers)
# Docker must be running when tests execute

# Install Rust dependencies
cargo build
```

### Test Execution

```bash
# Run all tests (includes integration tests)
cargo test

# Run only database integration tests
cargo test --test integration

# Run specific test module
cargo test models::data_source::tests

# Run tests with output
cargo test -- --nocapture

# Run tests serially (recommended for database tests)
cargo test -- --test-threads=1
```

### Test Configuration

Tests use the `serial_test` crate to prevent conflicts:

```rust
#[tokio::test]
#[serial_test::serial] // Ensures tests run sequentially
async fn test_database_operation() {
    // Test implementation
}
```

## Test Data Management

### Seed Data

The `TestContainer::seed_test_data()` method provides consistent test data:

```rust
pub async fn seed_test_data(&self) {
    // Creates standard test data:
    // - 1 data source (Test Data Source)
    // - 1 economic series (Test Economic Series)  
    // - 12 data points (monthly data for 2024)
}
```

### Database Cleaning

```rust
pub async fn clean_database(&self) {
    // Truncates all tables in dependency order
    // Resets sequences to avoid ID conflicts
    // Ensures clean state for each test
}
```

### Custom Test Data

Tests can create additional data as needed:

```rust
db_test!(test_custom_scenario, |container: Arc<TestContainer>| async move {
    // Start with seed data
    container.seed_test_data().await;
    
    // Add custom test data
    let custom_source = NewDataSource { /* ... */ };
    // ... create custom test scenario
});
```

## Performance Considerations

### Connection Pooling

- Each test container has its own connection pool
- Pool size limited to 5 connections for tests
- Connections automatically managed and cleaned up

### Test Parallelization

```rust
// Tests marked with serial_test run sequentially
#[serial_test::serial]

// This prevents:
// - Database port conflicts
// - Resource contention  
// - Test data interference
```

### Container Reuse

- Containers are created per test for isolation
- Alternative: Shared container with database cleaning
- Trade-off between speed and isolation

## Debugging Tests

### Logging

```bash
# Enable database query logging
RUST_LOG=debug cargo test

# Enable testcontainers logging  
TESTCONTAINERS_LOG_LEVEL=DEBUG cargo test
```

### Manual Inspection

```rust
db_test!(test_debug_scenario, |container: Arc<TestContainer>| async move {
    // Print database URL for manual connection
    println!("Database URL: {}", container.database_url());
    
    // Add breakpoint or sleep to inspect database
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
});
```

### Connection Details

```rust
// Access raw connection for debugging
let conn = container.pool().get().await?;
let result = conn.interact(|conn| {
    // Raw SQL for debugging
    diesel::sql_query("SELECT * FROM data_sources")
        .load::<DataSource>(conn)
}).await?;
```

## Best Practices

### Test Structure

```rust
db_test!(test_descriptive_name, |container: Arc<TestContainer>| async move {
    // REQUIREMENT: Link to specific system requirement
    // PURPOSE: Clear description of what is being tested
    // Context: Why this test is important
    
    // 1. Setup - prepare test data
    container.seed_test_data().await;
    
    // 2. Execute - perform the operation being tested
    let result = perform_database_operation().await;
    
    // 3. Verify - assert expected outcomes
    assert_eq!(result.status, ExpectedStatus);
    assert!(result.data.is_some());
    
    // 4. Additional verification - check side effects
    let count = container.pool().table_row_count("table_name").await;
    assert_eq!(count, expected_count);
});
```

### Error Handling

```rust
// Use expect() with descriptive messages
let conn = pool.get().await.expect("Failed to get database connection");

// Handle database errors appropriately
let result = conn.interact(|conn| {
    diesel::insert_into(table)
        .values(&data)
        .get_result(conn)
}).await.expect("Failed to interact with database")
  .expect("Failed to insert data");
```

### Test Data Isolation

```rust
// Each test should be independent
db_test!(test_independent_operation, |container: Arc<TestContainer>| async move {
    // Don't rely on data from other tests
    container.clean_database().await;
    
    // Create only the data needed for this test
    create_minimal_test_data().await;
    
    // Test implementation...
});
```

## Troubleshooting

### Common Issues

**Docker Not Running**
```
Error: Could not find a valid Docker installation
Solution: Start Docker Desktop or Docker daemon
```

**Port Conflicts**  
```
Error: Port already in use
Solution: Tests run serially to avoid this, check for hanging containers
```

**Permission Issues**
```
Error: Permission denied accessing Docker
Solution: Add user to docker group or run with appropriate permissions
```

**Slow Test Execution**
```
Issue: Tests take long time to complete
Solutions:
- Use shared test container (trade-off: less isolation)
- Reduce test data size
- Optimize database queries in tests
```

### Debugging Commands

```bash
# List running containers
docker ps

# View container logs
docker logs <container_id>

# Connect to test database manually
psql postgresql://test_user:test_password@localhost:<port>/test_econ_graph

# Clean up hanging containers
docker container prune
```

## Continuous Integration

### GitHub Actions Setup

```yaml
name: Database Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      docker:
        image: docker:dind
        options: --privileged
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        
    - name: Run Database Tests
      run: cargo test --release
      env:
        RUST_LOG: info
```

### Docker-in-Docker

For CI environments, ensure Docker-in-Docker is available:

```dockerfile
# If using custom CI containers
FROM rust:latest

# Install Docker for testcontainers
RUN apt-get update && apt-get install -y docker.io

# Set up test environment
WORKDIR /app
COPY . .
RUN cargo build --release

CMD ["cargo", "test", "--release"]
```

## Future Enhancements

### Potential Improvements

1. **Shared Container Strategy**
   - Single container per test suite
   - Faster test execution
   - Database state management between tests

2. **Test Data Fixtures**
   - JSON/YAML test data files
   - Reusable test scenarios
   - Complex relationship testing

3. **Performance Benchmarking**
   - Automated performance regression testing
   - Query performance monitoring
   - Database optimization validation

4. **Multi-Database Testing**
   - Test against different PostgreSQL versions
   - Validate migration compatibility
   - Cross-platform database testing

### Migration Testing

```rust
db_test!(test_migration_rollback, |container: Arc<TestContainer>| async move {
    // Test migration rollback scenarios
    // Verify data integrity during schema changes
    // Validate backward compatibility
});
```

This comprehensive database testing setup ensures that the EconGraph backend maintains high data integrity and reliability through real database integration testing with testcontainers.
