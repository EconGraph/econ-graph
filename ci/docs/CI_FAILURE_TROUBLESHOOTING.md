# CI Failure Troubleshooting Guide

This guide provides systematic approaches to diagnosing and fixing common CI failures in the EconGraph project.

## Quick Diagnosis Commands

```bash
# Check recent CI runs
gh run list --limit 10

# Get detailed logs for a specific failure
gh run view RUN_ID --log-failed

# Check specific job logs
gh run view RUN_ID --job JOB_ID
```

## Common CI Failure Categories

### 1. Database Constraint Violations

**Symptoms:**
- `duplicate key value violates unique constraint "countries_iso_code_2_key"`
- `duplicate key value violates unique constraint "data_sources_name_key"`
- Tests fail with database constraint errors

**Root Causes:**
- Poor test isolation
- Database cleaning not working properly
- Tests running in parallel with shared data
- Non-unique test identifiers

**Diagnosis Steps:**
1. Check if `clean_database()` method returns `Result` and is handled properly
2. Look for `#[serial]` attributes on tests that modify shared data
3. Verify test data uses unique identifiers (UUIDs, timestamps)
4. Check if tests are properly cleaning up after themselves

**Solutions:**
```rust
// ✅ Fix: Proper database cleaning with Result handling
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

// ✅ Fix: Handle the Result in tests
container.clean_database().await.expect("Failed to clean database");

// ✅ Fix: Use unique test identifiers
let test_id = Uuid::new_v4().to_string()[..8].to_string();
let country = NewCountry {
    iso_code: format!("T{}", &test_id[..2]),
    iso_code_2: format!("T{}", &test_id[..1]),
    // ...
};
```

### 2. Rust Compiler Warnings

**Symptoms:**
- `unused variable: 'variable_name'`
- `unused import: 'module_name'`
- `unused Result that must be used`
- `variable does not need to be mutable`

**Diagnosis Steps:**
1. Run `cargo check` locally to see all warnings
2. Check if warnings are consistent across different test runs
3. Look for patterns in warning types

**Solutions:**
```rust
// ✅ Fix unused variables
let _unused_variable = some_function();

// ✅ Fix unused imports
// Remove unused imports or use them

// ✅ Fix unused Results
let _ = result_that_must_be_used();
// OR
result_that_must_be_used().expect("Meaningful error message");

// ✅ Fix unnecessary mutability
let variable = some_value(); // Remove 'mut' if not needed
```

**Automated Fixes:**
```bash
# Apply many warning fixes automatically
cargo fix --lib -p package_name --tests

# Check what would be fixed without applying
cargo fix --lib -p package_name --tests --dry-run
```

### 3. Container Timeout Issues

**Symptoms:**
- `Failed to start container: Client(CreateContainer(RequestTimeoutError))`
- Tests hang for 60+ seconds then fail
- Docker-related timeouts

**Root Causes:**
- Docker resource constraints
- Network connectivity issues
- Test container configuration problems
- Parallel test execution overwhelming system

**Diagnosis Steps:**
1. Check Docker resource usage: `docker stats`
2. Verify Docker daemon is running: `docker ps`
3. Check available disk space: `df -h`
4. Look for network connectivity issues

**Solutions:**
```rust
// ✅ Fix: Add proper timeout handling
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_with_timeout() {
    let result = timeout(Duration::from_secs(30), async {
        // Test code here
    }).await;
    
    match result {
        Ok(test_result) => test_result,
        Err(_) => panic!("Test timed out"),
    }
}
```

### 4. Frontend Build Warnings

**Symptoms:**
- `'function_name' is defined but never used`
- `'TypeName' is defined but never used`
- TypeScript/ESLint warnings

**Solutions:**
```typescript
// ✅ Fix unused functions
export const _unusedFunction = () => { /* ... */ };

// ✅ Fix unused types
export type _UnusedType = { /* ... */ };

// ✅ Remove unused imports
// Remove or use the imported items
```

### 5. Port Configuration Issues

**Symptoms:**
- Backend health check failures (`Failed to start container`)
- E2E tests fail with "Backend server never became ready"
- Connection timeouts to backend services
- Tests pass locally but fail in CI

**Root Causes:**
- Port configuration mismatch between environments
- Backend server not binding to expected port
- Docker container port mapping issues
- Environment variable conflicts

**Important Note:**
Non-standard ports (like 9876 for backend, 3001 for frontend) are **intentionally used for local development** to avoid conflicts with other services. This is by design and should be preserved.

**Diagnosis Steps:**
1. Check port configuration in different environments:
   ```bash
   # Local development
   cat ports.env
   
   # CI environment
   cat ci-env.config
   
   # CI workflow
   grep -r "BACKEND_PORT" .github/workflows/
   ```

2. Verify backend startup logs for port binding:
   ```bash
   # Look for these log messages in backend startup
   grep "Server is now running" backend.log
   grep "Health check available" backend.log
   ```

3. Test backend health endpoint:
   ```bash
   curl http://localhost:9876/health  # Local development
   curl http://localhost:8080/health  # CI environment
   ```

**Solutions:**
1. **Keep local development ports** (non-standard by design):
   ```bash
   # ports.env - Keep as-is for local development
   BACKEND_PORT=9876
   FRONTEND_PORT=3001
   ```

2. **Standardize CI port configuration**:
   ```bash
   # ci-env.config - Use standard ports for CI
   BACKEND_PORT=8080
   FRONTEND_PORT=3000
   ```

3. **Update CI workflow** to use consistent ports:
   ```yaml
   # .github/workflows/ci-core.yml
   env:
     BACKEND_PORT: 8080  # Use standard port for CI
     FRONTEND_PORT: 3000
   ```

4. **Add comprehensive backend startup logging** (already implemented):
   - Port binding confirmation
   - Health check URL logging
   - Environment variable logging
   - Quick access URLs for debugging

### 6. Test Execution Failures

**Symptoms:**
- Tests fail with specific error messages
- Tests pass locally but fail in CI
- Intermittent test failures

**Diagnosis Steps:**
1. Run the specific failing test locally
2. Check for environment differences
3. Look for race conditions or timing issues
4. Verify test data setup and cleanup

**Solutions:**
```rust
// ✅ Fix: Add proper test isolation
#[tokio::test]
#[serial] // Prevents parallel execution
async fn test_that_modifies_shared_state() {
    // Test implementation
}

// ✅ Fix: Use unique test data
let unique_id = Uuid::new_v4();
let test_data = create_test_data_with_id(unique_id);
```

## Systematic Debugging Process

### Step 1: Identify the Failure Type
1. Check the CI run summary for failed jobs
2. Look at the error messages in the logs
3. Categorize the failure type using the categories above

### Step 2: Get Detailed Information
```bash
# Get full logs for the failed run
gh run view RUN_ID --log-failed

# Get logs for specific job
gh run view RUN_ID --job JOB_ID

# Download logs as file
gh run download RUN_ID
```

### Step 3: Reproduce Locally
```bash
# Run the specific failing test
cargo test --lib test_name -- --nocapture

# Run all tests to see if it's isolated
cargo test --lib

# Check for warnings
cargo check
```

### Step 4: Apply the Appropriate Fix
1. Use the solutions provided for each failure type
2. Test the fix locally
3. Commit and push to trigger CI again

### Step 5: Verify the Fix
1. Check that CI passes
2. Ensure no new warnings or errors are introduced
3. Verify the fix doesn't break other functionality

## Prevention Strategies

### 1. Pre-commit Hooks
- Ensure all tests pass before committing
- Fix all compiler warnings
- Run database tests with proper isolation

### 2. Local Testing
```bash
# Run the same tests as CI
cargo test --lib
cargo check
cargo clippy

# Test database operations
cargo test --lib --features test-database
```

### 3. Code Review Checklist
- [ ] All tests pass locally
- [ ] No compiler warnings
- [ ] Database tests use proper isolation
- [ ] Test data uses unique identifiers
- [ ] Results are handled properly

## Emergency Fixes

### Quick Database Fix
```bash
# If database tests are failing, try cleaning up
docker-compose down
docker volume rm econ-graph_postgres_data
docker-compose up -d postgres
```

### Quick Warning Fix
```bash
# Apply automatic fixes
cargo fix --lib --tests --allow-dirty
```

### Quick Test Fix
```bash
# Run tests with more verbose output
cargo test --lib -- --nocapture --test-threads=1
```

## Monitoring and Alerts

### CI Health Metrics
- Test pass rate
- Build time trends
- Warning count over time
- Failure frequency by category

### Proactive Monitoring
- Set up alerts for CI failures
- Monitor warning trends
- Track test execution times
- Watch for resource usage spikes

### 6. Database Migration Order Issues

**Symptoms:**
- `❌ ERROR: Migration 'YYYY-MM-DD-*' has date YYYY-MM-DD`
- `This is after the latest migration date: YYYY-MM-DD`
- `Migration dates must be chronologically ordered`
- Quality checks job fails with migration ordering errors

**Root Causes:**
- Migration dates not following chronological order
- Backdating migrations for "logical grouping"
- Cross-day development creating date mismatches
- Migration consolidation without proper date management

**Important Note:**
Migration dates must reflect the actual chronological order of development, not logical grouping. Non-standard dates (like backdated migrations) cause database state inconsistencies.

**Diagnosis Steps:**
1. Check migration directory structure:
   ```bash
   ls -la backend/migrations/
   ```

2. Verify migration date ordering:
   ```bash
   # Check migration dates
   find backend/migrations -name "*-*" -type d | sort
   
   # Compare with commit dates
   for dir in backend/migrations/*/; do
     migration_name=$(basename "$dir")
     commit_date=$(git log --format="%ad" --date=short -1 -- "$dir")
     echo "$commit_date: $migration_name"
   done | sort
   ```

3. Check CI quality-checks job output:
   ```bash
   gh run view RUN_ID --job quality-checks
   ```

**Solutions:**
1. **Fix Migration Ordering**:
   ```bash
   # Rename migrations to proper chronological order
   mv backend/migrations/2024-01-01-000001_old_name \
      backend/migrations/2025-09-10-000001_new_name
   ```

2. **Consolidate Problematic Migrations**:
   ```bash
   # Create consolidated migration
   diesel migration generate consolidated_initial_schema
   
   # Move all problematic migrations into consolidated one
   # Update migration files accordingly
   ```

3. **Use Proper Migration Naming**:
   ```bash
   # Format: YYYY-MM-DD-000001_description
   # Example: 2025-09-23-000001_add_user_preferences
   ```

4. **Pre-commit Hook Prevention**:
   ```bash
   # Install pre-commit hooks
   pre-commit install
   
   # The hook will prevent commits with ordering issues
   ```

**Prevention:**
- Always use current date for new migrations
- Never backdate migrations for logical grouping
- Use pre-commit hooks to catch ordering issues early
- Consolidate migrations when needed rather than backdating

## Additional Resources

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Docker Troubleshooting](https://docs.docker.com/config/troubleshooting/)
- [PostgreSQL Error Codes](https://www.postgresql.org/docs/current/errcodes-appendix.html)
- [Database Migration Best Practices](https://docs.djangoproject.com/en/stable/topics/migrations/)
