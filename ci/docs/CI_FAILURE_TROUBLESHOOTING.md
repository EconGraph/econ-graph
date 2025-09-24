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

### 5. Test Execution Failures

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

## 5. Port Configuration Issues

**Symptoms:**
- `Backend server never became ready` errors
- `curl: (7) Failed to connect to localhost port` errors
- E2E tests failing with connection timeouts
- Backend health checks failing consistently

**Root Causes:**
- Port configuration mismatch between development and CI environments
- Hardcoded port values in CI workflows
- Missing environment variables for port configuration
- Inconsistent port usage across different test suites

**Diagnosis Steps:**
1. Check if backend is starting on the expected port:
   ```bash
   # In CI logs, look for:
   # "Server starting on http://0.0.0.0:8080" (should be 8080, not 9876)
   ```
2. Verify environment variables are set correctly:
   ```bash
   # Backend should log: "BACKEND_PORT: 8080"
   ```
3. Check Docker port mappings:
   ```bash
   # Should be: -p 8080:8080 (not -p 9876:9876)
   ```
4. Verify health check URLs:
   ```bash
   # Should be: http://localhost:8080/health (not localhost:9876)
   ```

**Solutions:**
1. **Standardize CI Ports**: Use port 8080 for backend in CI environments
2. **Preserve Development Ports**: Keep non-standard ports (9876) for local development to avoid conflicts
3. **Add Environment Variables**: Ensure `BACKEND_PORT=8080` is set in Docker containers
4. **Update Health Checks**: Use correct port in health check loops
5. **Add Debugging**: Backend logs should show which port it's binding to

**Port Strategy:**
- **Local Development**: `BACKEND_PORT=9876` (non-standard to avoid conflicts)
- **CI Environment**: `BACKEND_PORT=8080` (standard port for consistency)
- **Documentation**: Clearly explain the port strategy and reasoning

**Prevention:**
- Always use environment variables for port configuration
- Test port changes in CI before merging
- Document port strategy clearly
- Add port binding confirmation logs to backend startup

## 6. Database Migration Order Issues

**Symptoms:**
- Migration date ordering validation failures in CI
- `ERROR: Migration 'migration_name' has date X after latest migration date Y`
- Database schema inconsistencies between environments
- Migration conflicts during deployment

**Root Causes:**
- Migration dates not following chronological order
- Backdating migrations for "logical grouping"
- Cross-day development creating date mismatches
- Multiple developers working on migrations simultaneously

**Diagnosis Steps:**
1. Check migration date ordering:
   ```bash
   # Look for migrations with dates after the latest migration
   ls -la backend/migrations/
   ```
2. Compare commit dates vs migration dates:
   ```bash
   git log --oneline --name-only backend/migrations/
   ```
3. Check CI quality checks output for migration validation errors

**Solutions:**
1. **Fix Migration Dates**: Ensure migration dates reflect actual chronological order
2. **Consolidate Migrations**: Combine problematic migrations into single consolidated migration
3. **Use Proper Naming**: Follow `YYYY-MM-DD-000001_description` format
4. **Pre-commit Prevention**: Install pre-commit hooks to catch ordering issues early

**Prevention:**
- Always use current date for new migrations
- Never backdate migrations for "logical grouping"
- Use pre-commit hooks to validate migration dates
- Coordinate with team on migration timing

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

## 7. Comprehensive Debugging Philosophy

### **The High Cost of CI Iteration Cycles**

**Critical Insight**: CI iteration cycles in this project can take **45-60 minutes** per cycle. This means:
- **Each debugging attempt costs nearly an hour**
- **Multiple hypotheses require multiple cycles**
- **Speculation without data is extremely expensive**
- **Future troubleshooters will face the same time cost**

### **Debugging Strategy: Comprehensive Over Minimal**

**Principle**: When debugging CI failures, **add extensive debugging for ALL possible hypotheses** rather than iterating with minimal information.

**Why This Approach:**
1. **Time Cost**: One comprehensive debugging cycle vs. 3-4 minimal cycles saves 2-3 hours
2. **Complete Visibility**: See exactly what's happening, not just symptoms
3. **Future-Proofing**: Next person debugging gets complete information immediately
4. **Hypothesis Coverage**: Test all failure modes in one cycle instead of guessing

### **What Comprehensive Debugging Looks Like**

**Example: Backend Startup Failures**
Instead of just checking if backend started, comprehensive debugging includes:

```bash
# Database Connection Hypotheses
- PostgreSQL host/port connectivity testing
- Authentication verification (username/password)
- Database existence and permissions
- Network connectivity between containers

# Environment Variable Hypotheses  
- DATABASE_URL parsing and validation
- BACKEND_PORT configuration verification
- Missing required environment variables
- Container environment variable inspection

# Container/Image Hypotheses
- Backend binary existence and permissions
- Container startup and exit code analysis
- Missing dependencies detection
- Filesystem and binary validation

# Port/Network Hypotheses
- Port availability and binding verification
- Docker networking configuration
- Health endpoint accessibility
- Network interface debugging

# Backend Application Hypotheses
- Real-time log tailing during startup
- Process monitoring and debugging
- Database migration failure detection
- GraphQL schema creation issues
- Authentication service initialization
- Metrics service startup problems
```

### **Debugging Implementation Strategy**

**1. Add Debugging for ALL Hypotheses**
```bash
# Don't just check "did it start?"
# Check EVERYTHING that could go wrong:

# Pre-startup environment validation
echo "📋 Pre-startup environment check:"
echo "  - Current directory: $(pwd)"
echo "  - Docker version: $(docker --version)"
echo "  - Available images:"
docker images | grep econ-graph || echo "    No econ-graph images found"
echo "  - Port 8080 status:"
netstat -tlnp | grep :8080 || echo "    Port 8080 is free"
echo "  - PostgreSQL connectivity test:"
nc -zv host.docker.internal 5432 && echo "    ✅ PostgreSQL reachable" || echo "    ❌ PostgreSQL not reachable"

# Real-time monitoring during startup
docker logs -f backend-server &
LOG_TAIL_PID=$!

# Comprehensive health checking
for i in {1..30}; do
  echo "🔍 Attempt $i/30 - Comprehensive health check:"
  
  # Check if container is still running
  if ! docker ps --filter name=backend-server --format "table {{.Names}}" | grep -q backend-server; then
    echo "  ❌ Container stopped running!"
    echo "  📋 Container exit code: $(docker inspect backend-server --format='{{.State.ExitCode}}' 2>/dev/null || echo 'unknown')"
    break
  fi
  
  # Check if port is listening
  if netstat -tlnp | grep -q :8080; then
    echo "  ✅ Port 8080 is listening"
  else
    echo "  ❌ Port 8080 not listening"
  fi
  
  # Check health endpoint
  if curl -f http://localhost:8080/health 2>/dev/null; then
    echo "  ✅ Health endpoint responding"
    echo "✅ Backend is ready!"
    break
  else
    echo "  ❌ Health endpoint not responding"
  fi
  
  # Show container status
  echo "  📋 Container status:"
  docker ps --filter name=backend-server --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
  
  sleep 2
done

# Comprehensive failure analysis
if [ "$BACKEND_READY" = false ]; then
  echo "❌ BACKEND STARTUP FAILED - COMPREHENSIVE DIAGNOSIS:"
  
  # Container status and inspection
  echo "📋 Container Status:"
  docker ps -a --filter name=backend-server --format "table {{.Names}}\t{{.Status}}\t{{.ExitCode}}\t{{.Ports}}"
  
  # Container logs
  echo "📋 Container Logs (last 50 lines):"
  docker logs --tail 50 backend-server 2>&1 || echo "No logs available"
  
  # Network debugging
  echo "📋 Network Debugging:"
  echo "  - Port 8080 status:"
  netstat -tlnp | grep :8080 || echo "    Port 8080 not listening"
  echo "  - Docker network info:"
  docker network ls
  echo "  - Container network:"
  docker inspect backend-server --format='{{json .NetworkSettings}}' | jq . 2>/dev/null || echo "Network inspection failed"
  
  # Process debugging
  echo "📋 Process Debugging:"
  echo "  - Processes in container:"
  docker exec backend-server ps aux 2>/dev/null || echo "Cannot exec into container"
  echo "  - Container filesystem:"
  docker exec backend-server ls -la /app/backend/ 2>/dev/null || echo "Cannot list backend directory"
  
  # Environment debugging
  echo "📋 Environment Debugging:"
  echo "  - Environment variables in container:"
  docker exec backend-server env | grep -E "(DATABASE_URL|BACKEND_PORT|USER|PG)" 2>/dev/null || echo "Cannot get environment variables"
  
  # Database connectivity from container
  echo "📋 Database Connectivity from Container:"
  echo "  - Testing database connection from inside container:"
  docker exec backend-server nc -zv host.docker.internal 5432 2>&1 || echo "Cannot test database connectivity"
  echo "  - Testing with psql from container:"
  docker exec backend-server psql "postgresql://postgres:password@host.docker.internal:5432/econ_graph_test" -c "SELECT 1;" 2>&1 || echo "Cannot test database with psql"
  
  # Backend binary debugging
  echo "📋 Backend Binary Debugging:"
  echo "  - Binary exists:"
  docker exec backend-server ls -la /app/backend/econ-graph-backend 2>/dev/null || echo "Binary not found"
  echo "  - Binary is executable:"
  docker exec backend-server test -x /app/backend/econ-graph-backend && echo "    ✅ Executable" || echo "    ❌ Not executable"
  echo "  - Binary version:"
  docker exec backend-server /app/backend/econ-graph-backend --version 2>&1 || echo "Cannot get version"
  
  # System resources
  echo "📋 System Resources:"
  echo "  - Memory usage:"
  free -h
  echo "  - Disk usage:"
  df -h
  echo "  - Docker system info:"
  docker system df
fi
```

**2. Leave Debugging in Place**
- **Don't remove debugging after fixing the issue**
- **Future troubleshooters benefit from comprehensive information**
- **Debugging overhead is minimal compared to iteration cost**
- **Comprehensive debugging prevents future speculation**

**3. Document the Debugging Strategy**
- **Explain why comprehensive debugging is used**
- **Show examples of what debugging covers**
- **Make it clear this is intentional, not accidental**

### **Benefits of This Approach**

**Immediate Benefits:**
- **Complete visibility** into failure causes
- **No speculation** about what went wrong
- **Faster resolution** (one cycle vs. multiple)
- **Confidence** in the fix

**Long-term Benefits:**
- **Future troubleshooters** get complete information
- **Reduced debugging time** for similar issues
- **Better understanding** of system behavior
- **Prevention** of similar issues

**Cost-Benefit Analysis:**
- **Cost**: 5-10 minutes to add comprehensive debugging
- **Benefit**: Saves 2-3 hours of iteration cycles
- **ROI**: 20-30x return on debugging investment

### **When to Use Comprehensive Debugging**

**Always use comprehensive debugging for:**
- **CI failures** (high iteration cost)
- **Production issues** (high impact)
- **Complex system interactions** (multiple failure points)
- **Intermittent failures** (hard to reproduce)

**Examples of comprehensive debugging:**
- **Backend startup failures** (database, networking, binary, environment)
- **Database connection issues** (authentication, network, permissions, schema)
- **Container orchestration problems** (networking, resource limits, dependencies)
- **Test environment setup** (service dependencies, configuration, data)

### **My Personal Philosophy on This**

I think of comprehensive debugging as **"insurance against speculation"**. When you're dealing with complex systems like CI/CD pipelines, there are dozens of things that could go wrong, and the cost of guessing wrong is extremely high.

The traditional approach of "add minimal debugging, see what happens, iterate" works fine for local development where cycles are seconds or minutes. But in CI environments where each cycle costs an hour, this approach becomes prohibitively expensive.

Instead, I prefer to **"debug everything that could possibly be wrong, all at once"**. This gives you complete visibility into the system state, eliminates speculation, and provides a comprehensive picture that future troubleshooters can use.

It's like the difference between:
- **Traditional**: "Let me check if the backend started" → wait 60 minutes → "Hmm, it didn't start, let me check the logs" → wait 60 minutes → "Still not sure, let me check the database connection" → wait 60 minutes
- **Comprehensive**: "Let me check the backend startup, database connectivity, environment variables, container status, network configuration, binary permissions, system resources, and real-time logs all at once" → wait 60 minutes → "Here's exactly what's wrong and how to fix it"

The comprehensive approach might seem like overkill, but when iteration cycles are this expensive, it's actually the most efficient approach. You get complete information in one cycle instead of partial information across multiple cycles.

## Additional Resources

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Docker Troubleshooting](https://docs.docker.com/config/troubleshooting/)
- [PostgreSQL Error Codes](https://www.postgresql.org/docs/current/errcodes-appendix.html)
