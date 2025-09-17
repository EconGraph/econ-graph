# Developer Guide: Database Setup and Migrations

## Overview

This guide covers database setup, migration management, and testing for the econ-graph3 backend. It addresses common issues developers encounter when working with the database.

## Database Setup

### Fresh Database Setup

1. **Stop existing containers and remove volumes:**
   ```bash
   # Stop all containers
   docker-compose down
   
   # Remove the postgres data volume (WARNING: This deletes all data!)
   docker volume rm econ-graph_postgres_data
   ```

2. **Start fresh database:**
   ```bash
   # Start postgres container
   docker-compose up -d postgres
   
   # Wait for database to be ready (5-10 seconds)
   sleep 5
   ```

3. **Run all migrations:**
   ```bash
   # Check migration status
   diesel migration list
   
   # Run all pending migrations
   diesel migration run
   
   # Verify schema
   diesel print-schema | grep api_key_name
   ```

### Database Configuration

- **Development Database:** `econ_graph_dev` (port 5432)
- **Test Database:** `econ_graph_test` (port 5433)
- **Production Database:** `econ_graph` (port 5432)

**Environment Variables:**
- `DATABASE_URL=postgresql://postgres:password@localhost:5432/econ_graph_dev`

## Migration Management

### Creating Migrations

```bash
# Generate new migration
diesel migration generate migration_name

# This creates:
# - migrations/TIMESTAMP_migration_name/up.sql
# - migrations/TIMESTAMP_migration_name/down.sql
```

### Running Migrations

```bash
# Run all pending migrations
diesel migration run

# Check migration status
diesel migration list

# Revert last migration
diesel migration revert

# Redo last migration (revert + run)
diesel migration redo
```

### Migration Best Practices

1. **Always test migrations both ways:**
   ```bash
   # Test up migration
   diesel migration run
   
   # Test down migration
   diesel migration revert
   
   # Test up again
   diesel migration run
   ```

2. **Schema changes require struct updates:**
   - Update `models/data_source.rs` structs
   - Update all `NewDataSource` initializers
   - Update test files with new fields
   - Run `diesel print-schema` to verify

3. **Common migration patterns:**
   ```sql
   -- Adding columns
   ALTER TABLE table_name ADD COLUMN column_name TYPE;
   COMMENT ON COLUMN table_name.column_name IS 'Description';
   
   -- Updating existing data
   UPDATE table_name SET column_name = 'default_value' WHERE column_name IS NULL;
   ```

## Testing

### Test Database Setup

Tests use TestContainers which create fresh databases automatically. However, you need to ensure:

1. **All migrations are in the migrations directory**
2. **TestContainers run migrations automatically**
3. **Test database uses correct schema**

### Troubleshooting Test Failures

**Problem:** `column "api_key_name" does not exist`
**Solution:** 
1. Ensure migration files exist in `migrations/` directory
2. Check that `diesel migration run` works on development database
3. Verify TestContainer setup runs migrations

**Problem:** Migration shows as not applied but column exists
**Solution:**
1. Check if migration was applied manually
2. Run `diesel migration list` to see status
3. Use fresh database to test migration sequence

## Common Issues and Solutions

### Issue: Port Already in Use

```bash
# Find what's using port 5432
docker ps | grep 5432

# Stop conflicting container
docker stop CONTAINER_ID

# Or use different port in docker-compose.yml
```

### Issue: Database Connection Failed

```bash
# Check if database is running
docker ps | grep postgres

# Check database logs
docker logs econ-graph3-postgres-1

# Test connection
docker exec -it econ-graph3-postgres-1 psql -U postgres -d econ_graph_dev
```

### Issue: Migration Conflicts

```bash
# Check current schema
diesel print-schema

# Check migration status
diesel migration list

# If column exists but migration not marked as applied:
# 1. Remove the migration file
# 2. Or manually mark as applied (if safe)
```

## Development Workflow

### Daily Development

1. **Start development environment:**
   ```bash
   docker-compose up -d postgres
   ```

2. **Check database status:**
   ```bash
   diesel migration list
   ```

3. **Run tests:**
   ```bash
   cargo test --lib
   ```

### Adding New Features

1. **Create migration:**
   ```bash
   diesel migration generate feature_name
   ```

2. **Write migration SQL:**
   - Edit `up.sql` and `down.sql`
   - Test both directions

3. **Update Rust code:**
   - Update structs in `models/`
   - Update all initializers
   - Update tests

4. **Test thoroughly:**
   ```bash
   # Test migration
   diesel migration run
   diesel migration revert
   diesel migration run
   
   # Test code
   cargo test --lib
   ```

### Production Deployment

1. **Backup database**
2. **Run migrations:**
   ```bash
   diesel migration run --locked-schema
   ```
3. **Verify schema:**
   ```bash
   diesel print-schema
   ```

## Schema Reference

### Key Tables

- **`data_sources`**: Configuration for external data APIs
  - `api_key_name`: Environment variable name for API key (nullable)
  - `api_key_required`: Boolean flag for API key requirement
  
- **`economic_series`**: Time series data from external sources
- **`data_points`**: Individual data points for time series
- **`series_metadata`**: Metadata for discovered series

### Recent Schema Changes

- **2025-09-13**: Added `api_key_name` column to `data_sources` table
- **2025-09-13**: Updated Census data source configuration
- **2025-09-12**: Added series metadata tracking
- **2025-09-12**: Enhanced crawler schema

## Test Database Management

### Proper Test Cleanup Implementation

When implementing `clean_database()` methods in test utilities:

```rust
// ✅ CORRECT: Return Result and handle errors properly
pub async fn clean_database(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut conn = self.pool.get().await.map_err(|e| format!("Failed to get connection: {}", e))?;
    
    diesel_async::RunQueryDsl::execute(
        diesel::sql_query("TRUNCATE TABLE countries CASCADE"),
        &mut conn,
    )
    .await
    .map_err(|e| format!("Failed to truncate countries: {}", e))?;
    
    Ok(())
}

// ❌ WRONG: Using expect() masks real errors
pub async fn clean_database(&self) {
    let mut conn = self.pool.get().await.expect("Failed to get connection");
    // ... rest of implementation
}
```

### Test Isolation Best Practices

1. **Always use unique identifiers in test data:**
   ```rust
   // ✅ CORRECT: Use UUIDs for unique test data
   let test_id = Uuid::new_v4().to_string()[..8].to_string();
   let country = NewCountry {
       iso_code: format!("T{}", &test_id[..2]),
       iso_code_2: format!("T{}", &test_id[..1]),
       // ...
   };
   ```

2. **Handle database cleaning Results properly:**
   ```rust
   // ✅ CORRECT: Handle the Result
   container.clean_database().await.expect("Failed to clean database");
   
   // ❌ WRONG: Ignoring the Result
   container.clean_database().await;
   ```

### Common Test Database Issues

**Problem:** `duplicate key value violates unique constraint "countries_iso_code_2_key"`

**Root Causes:**
- Database cleaning not working properly
- Tests running in parallel with shared data
- Non-unique test identifiers

**Solutions:**
1. Ensure `clean_database()` returns `Result` and is handled properly
2. Use `#[serial]` attribute for tests that modify shared data
3. Generate unique identifiers for test data
4. Verify database is actually cleaned between tests

**Problem:** `unused Result that must be used` warnings

**Solution:**
```rust
// ✅ CORRECT: Handle the Result
let _ = container.clean_database().await;

// ✅ BETTER: Proper error handling
container.clean_database().await.expect("Failed to clean database");
```

## Troubleshooting Checklist

When encountering database issues:

1. ✅ Check if database is running: `docker ps | grep postgres`
2. ✅ Verify connection string: `echo $DATABASE_URL`
3. ✅ Check migration status: `diesel migration list`
4. ✅ Test fresh database: Remove volume and run migrations
5. ✅ Verify schema: `diesel print-schema`
6. ✅ Run tests: `cargo test --lib`
7. ✅ Check logs: `docker logs econ-graph3-postgres-1`
8. ✅ **NEW:** Check for database constraint violations in test logs
9. ✅ **NEW:** Verify test isolation with `#[serial]` attributes
10. ✅ **NEW:** Ensure `clean_database()` returns `Result` and is handled properly

## Additional Resources

- [Diesel Migration Guide](https://diesel.rs/guides/migrations.html)
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)
- [Docker Compose Reference](https://docs.docker.com/compose/)
- [TestContainers Rust](https://github.com/testcontainers/testcontainers-rs)
