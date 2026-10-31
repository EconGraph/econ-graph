# Rust Development Best Practices

This guide covers Rust-specific best practices for the EconGraph backend. Snippets are independent examples; placeholders such as `some_function()` stand for surrounding application code. Examples using project APIs include imports or links to their definitions. Standalone teaching scaffolds are labeled separately.

## Warning Management

### Unused Variable Warnings

**Problem:** `unused variable: 'variable_name'`

**Solutions:**

```rust
// ✅ Prefix with underscore for intentionally unused variables
let _unused_variable = some_function();

// ✅ Use the variable if it's actually needed
let result = some_function();
println!("Result: {:?}", result);

// ✅ Use underscore prefix for function parameters
fn process_data(_unused_param: String, active_param: i32) {
    println!("Processing: {}", active_param);
}
```

### Unused Import Warnings

**Problem:** `unused import: 'module_name'`

**Solutions:**

```rust
// ✅ Remove unused imports
// use std::collections::HashMap; // Remove this line

// ✅ Use the import if it's actually needed
use std::collections::HashMap;
let map: HashMap<String, i32> = HashMap::new();

// ✅ Use conditional imports for test-only code
#[cfg(test)]
use std::collections::HashMap;
```

### Unused Result Warnings

**Problem:** `unused Result that must be used`

Choose the behavior the caller needs. Return recoverable failures with `?`; log and continue only when the operation can safely continue after failure. `let _ =` deliberately discards both success and failure, so reserve it for results whose loss is acceptable. `expect()` still panics: in production it requires a documented invariant, while tests may use it to stop immediately when setup fails.

**Solutions:**

```rust
// ✅ Report success or failure when continuing is acceptable
match result {
    Ok(value) => println!("Success: {:?}", value),
    Err(e) => eprintln!("Error: {}", e),
}

// ✅ Report a nonfatal failure and continue
if let Err(e) = result {
    eprintln!("Error: {}", e);
}

// ✅ Use let _ = for intentionally ignored results
let _ = result_that_must_be_used();

// ✅ An invariant: this literal is always a valid integer
let value: u32 = "42".parse().expect("hard-coded literal must be a valid u32");
```

### Unnecessary Mutability Warnings

**Problem:** `variable does not need to be mutable`

**Solutions:**

```rust
// ✅ Remove 'mut' if variable is never modified
let variable = some_value(); // Remove 'mut'

// ✅ Keep 'mut' only if you actually modify the variable
let mut counter = 0;
counter += 1;
```

## Error Handling Best Practices

### Database Operations

Connection acquisition and query failures are recoverable application errors. Return them to the caller rather than panicking. This example uses the project's [pool alias](../../backend/crates/econ-graph-core/src/database.rs) and [error types](../../backend/crates/econ-graph-core/src/error.rs):

```rust
use econ_graph_core::{
    database::DatabasePool,
    error::{AppError, AppResult},
};

async fn database_operation(pool: &DatabasePool) -> AppResult<()> {
    let mut conn = pool.get().await
        .map_err(|e| AppError::database_error(format!("Failed to get connection: {}", e)))?;

    diesel_async::RunQueryDsl::execute(
        diesel::sql_query("SELECT 1"),
        &mut conn,
    )
    .await
    .map_err(|e| AppError::database_error(format!("Query failed: {}", e)))?;

    Ok(())
}
```

Using `pool.get().await.expect("Failed to get connection")` here would panic instead of returning the failure to the caller.

### Test Utilities

A test should fail immediately if required setup fails; otherwise its assertions run against invalid state. The following alternatives assume the disposable-database setup and shared locking described in [Test Isolation](#test-isolation).

```rust
// ✅ CORRECT: Handle Results in test setup
#[tokio::test]
async fn test_something() {
    let container = TestContainer::new().await;
    container.clean_database().await.expect("Failed to clean database");
    
    // Test implementation
}

// ❌ WRONG: Ignoring Results
#[tokio::test]
async fn test_something() {
    let container = TestContainer::new().await;
    container.clean_database().await; // This ignores potential errors
}
```

## Test Development Patterns

### Test Isolation

Follow the target module's existing migration, fixture, and locking convention. Tests sharing a database must coordinate destructive cleanup and fixture writes through the same lock. `#[serial]` coordinates only participating tests using the same serialization arrangement; it does not protect against unrelated locks or separate Cargo processes.

`TestContainer::new()` gives each test a migrated scratch database of its own on the server `DATABASE_URL` names (or the test fallback), dropped with it; the role needs `CREATEDB`. Only core's own unit tests without `DATABASE_URL` start a PostgreSQL container instead. See [TestContainer lifecycle](../testing/DATABASE_TESTING.md#testcontainer-lifecycle) for the full setup.

`clean_database()` drops the test database's entire `public` schema with `CASCADE` and migrates that same database again; it never touches another test's database. Point `DATABASE_URL` at a disposable server; see the [cleaning and migration targets](../testing/DATABASE_TESTING.md#cleaning-and-migration-targets).

### Test Data Management

Use a full UUID for an unconstrained fixture name or identifier. Do not truncate it to fit a country code or another constrained field; choose valid values according to that fixture's schema and isolation strategy.

```rust
use uuid::Uuid;

let test_id = Uuid::new_v4();
let unique_name = format!("test_data_{}", test_id);
```

Keep the `TestContainer` alive while its pool is in use. Dropping it drops its scratch database (or stops the container it started). Tests on the shared `get_test_db()` database leave their rows behind, so they use unique fixture rows rather than destructive cleanup.

## Async Programming Best Practices

### Async Error Handling

Async functions follow the same error policy as synchronous functions. The project's `AppError::HttpClient` conversion accepts `reqwest::Error`, so `?` can propagate both request and response-decoding failures:

```rust
use econ_graph_core::error::AppResult;

async fn fetch_data(url: &str) -> AppResult<serde_json::Value> {
    let response = reqwest::get(url).await?.error_for_status()?;
    let data = response.json().await?;
    Ok(data)
}
```

Using `expect()` for these fallible network operations would panic instead of allowing the caller to handle the failure.

### Database Connection Management

Acquire a pooled connection only for the operations that need it, as in [Database Operations](#database-operations). The connection guard returns the connection to the pool when it leaves scope; it does not need a custom `Drop` implementation. Release it before unrelated slow work so other tasks can acquire a connection.

## Performance Considerations

### Avoiding Unnecessary Allocations

```rust
// ✅ CORRECT: Use references when possible
fn process_data(data: &[String]) -> Vec<String> {
    data.iter()
        .filter(|s| s.len() > 5)
        .map(|s| s.to_uppercase())
        .collect()
}

// ❌ WRONG: Unnecessary cloning
fn process_data(data: Vec<String>) -> Vec<String> {
    data.iter()
        .filter(|s| s.len() > 5)
        .map(|s| s.clone().to_uppercase()) // Unnecessary clone
        .collect()
}
```

### Efficient String Handling

```rust
// ✅ CORRECT: Use String when you need ownership
fn create_message(prefix: &str, suffix: &str) -> String {
    format!("{} - {}", prefix, suffix)
}

// ✅ CORRECT: Use &str for read-only access
fn print_message(message: &str) {
    println!("{}", message);
}

// ✅ Borrow unchanged text; allocate only for the uppercase result
use std::borrow::Cow;

fn process_text(text: &str) -> Cow<'_, str> {
    if text.contains("error") {
        Cow::Owned(text.to_uppercase())
    } else {
        Cow::Borrowed(text)
    }
}
```

## Memory Management

### Ownership and Resource Lifetimes

Owned fields are dropped automatically when their owner is dropped. RAII guards, such as a pooled connection or a mutex guard, release their resource on scope exit. Add a custom `Drop` implementation only when a type has an extra cleanup obligation that its fields do not already perform.

An `Arc` keeps its value alive until the last strong owner is dropped. This standalone example shows the lifetime through a `Weak` reference without adding a destructor:

```rust
use std::sync::Arc;

let data = Arc::new(vec![1, 2, 3]);
let observer = Arc::downgrade(&data);
{
    let shared = Arc::clone(&data);
    drop(data);
    assert_eq!(shared.len(), 3); // A strong owner still holds the vector.
    assert!(observer.upgrade().is_some());
}
assert!(observer.upgrade().is_none()); // The last strong owner left scope.
```

`Arc` does not prevent leaks caused by strong reference cycles. Use `Weak` for back-references that must not keep their target alive.

## Code Organization

### Module Structure

This schematic module layout illustrates re-exports and visibility; the service and model names are placeholders.

```rust
// ✅ CORRECT: Clear module organization
mod database;
mod services;
mod models;

pub use database::DatabasePool;
pub use services::{ServiceA, ServiceB};
pub use models::{ModelA, ModelB};

// ✅ CORRECT: Proper visibility
pub mod public_module;
mod private_module;

pub fn public_function() {}
fn private_function() {}
```

### Error Types

Use the project's existing [`AppError` and `AppResult`](../../backend/crates/econ-graph-core/src/error.rs) for application operations. Typed `From` conversions let callers propagate errors with `?`; use contextual messages when the underlying error alone does not identify the failed operation.

This standalone teaching scaffold shows how `thiserror` derives a conversion. `ExampleError` is not a replacement for the project's error enum:

```rust
#[derive(Debug, thiserror::Error)]
enum ExampleError {
    #[error("Could not read input: {0}")]
    Io(#[from] std::io::Error),
}

fn read_input(path: &std::path::Path) -> Result<String, ExampleError> {
    Ok(std::fs::read_to_string(path)?)
}
```

## Testing Best Practices

### Unit Tests

```rust
// ✅ CORRECT: Focused unit tests
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculation() {
        let input = 42;
        let expected = 84;
        let result = calculate(input);
        assert_eq!(result, expected);
    }
    
    #[test]
    fn test_error_handling() {
        let result = risky_operation();
        assert!(result.is_err());
    }
}
```

### Integration Tests

This pattern assumes the [disposable database and shared-lock prerequisites](#test-isolation). The `database_operation` helper is defined above. Use the target module's lock convention; the serial attribute shown here is only suitable when every test sharing this database participates in the same lock.

```rust
use econ_graph_core::test_utils::TestContainer;
use serial_test::serial;

// ✅ CORRECT: Integration test with proper setup
#[tokio::test]
#[serial]
async fn test_database_integration() {
    let container = TestContainer::new().await;
    container.clean_database().await.expect("Failed to clean database");
    
    let pool = container.pool();
    
    // Test database operations
    let result = database_operation(pool).await;
    assert!(result.is_ok());
}
```

## Common Anti-Patterns to Avoid

### 1. Panicking on Recoverable Failures

```rust
// ❌ WRONG: Panic on a recoverable application failure
let value = risky_operation().unwrap();

// ✅ Return a recoverable failure to the caller
let value = risky_operation()?;
```

### 2. Ignoring Results

```rust
// ❌ WRONG: Ignoring Results
risky_operation();

// ✅ Return a failure when the operation is required
risky_operation()?;

// ✅ Discard only an explicitly optional notification failure
let _ = send_optional_notification();
```

### 3. Unnecessary Cloning

```rust
// ❌ WRONG: Unnecessary cloning
let cloned = expensive_data.clone();
process_data(cloned);

// ✅ CORRECT: Use references
process_data(&expensive_data);
```

### 4. Poor Error Messages

```rust
// ❌ WRONG: Generic error messages
.map_err(|e| AppError::DatabaseError("Error".to_string()))?;

// ✅ CORRECT: Descriptive error messages
.map_err(|e| AppError::DatabaseError(format!("Failed to execute query: {}", e)))?;
```

## Automated Tools

Run these commands from the backend Cargo workspace. From the repository root:

```bash
cd backend
```

Check diagnostics before applying automatic edits, and review the resulting diff.

### Cargo Fix

```bash
# Apply automatic fixes
cargo fix --lib --tests

# Report compiler diagnostics without applying fixes (not a patch preview)
cargo check --lib --tests

# Fix specific package
cargo fix --lib -p econ-graph-core --tests
```

### Clippy

```bash
# Run clippy for additional suggestions
cargo clippy

# Run clippy with all warnings
cargo clippy -- -W clippy::all

# Fix clippy suggestions automatically
cargo clippy --fix
```

### Formatting

```bash
# Format code
cargo fmt

# Check formatting without changing files
cargo fmt -- --check
```

## Performance Profiling

### Basic Profiling

```rust
// ✅ CORRECT: Use timing for performance measurement
use std::time::Instant;

fn measure_performance() {
    let start = Instant::now();
    
    // Expensive operation
    expensive_operation();
    
    let duration = start.elapsed();
    println!("Operation took: {:?}", duration);
}
```

## Documentation

### Function Documentation

This teaching template illustrates arguments, return bounds, error conditions, and example usage for a hypothetical correlation function. Its implementation and example imports are omitted.

```rust
/// Calculates the correlation coefficient between two data series.
///
/// # Arguments
///
/// * `series_a` - First data series
/// * `series_b` - Second data series
///
/// # Returns
///
/// Returns the correlation coefficient as a float between -1.0 and 1.0.
/// Returns an error if the series have different lengths or are empty.
///
/// # Examples
///
/// ```
/// let series_a = vec![1.0, 2.0, 3.0];
/// let series_b = vec![2.0, 4.0, 6.0];
/// let correlation = calculate_correlation(&series_a, &series_b)?;
/// ```
pub fn calculate_correlation(series_a: &[f64], series_b: &[f64]) -> Result<f64, AppError> {
    // Implementation
}
```

### Module Documentation

Module documentation should describe the module's purpose and show how its public APIs fit together. This example uses the current [database APIs](../../backend/crates/econ-graph-core/src/database.rs); `database_url` is supplied by the caller, and creating a pool requires a reachable database:

```rust
//! Database connection pooling and connectivity checks.
//!
//! Create a pool, then verify that it can acquire a connection and run a query.
//! Pool creation does not apply migrations.
//!
//! # Usage
//!
//! ```no_run
//! use econ_graph_core::{
//!     database::{create_pool, test_connection},
//!     error::AppResult,
//! };
//!
//! async fn check_database(database_url: &str) -> AppResult<()> {
//!     let pool = create_pool(database_url).await?;
//!     test_connection(&pool).await?;
//!     Ok(())
//! }
//! ```
```

## Additional Resources

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Async Book](https://rust-lang.github.io/async-book/)
- [Error Handling in Rust](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
