# Rust Development Best Practices

This guide covers Rust-specific best practices for the EconGraph project, based on common patterns and issues encountered during development.

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

**Solutions:**
```rust
// ✅ Handle the Result properly
match result {
    Ok(value) => println!("Success: {:?}", value),
    Err(e) => eprintln!("Error: {}", e),
}

// ✅ Use if let for simple cases
if let Err(e) = result {
    eprintln!("Error: {}", e);
}

// ✅ Use let _ = for intentionally ignored results
let _ = result_that_must_be_used();

// ✅ Use expect() only when you're certain it won't fail
let value = result.expect("This should never fail in this context");
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

```rust
// ✅ CORRECT: Proper error handling with map_err
pub async fn clean_database(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut conn = self.pool.get().await
        .map_err(|e| format!("Failed to get connection: {}", e))?;
    
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

### Test Utilities

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

```rust
// ✅ CORRECT: Use serial attribute for tests that modify shared state
#[tokio::test]
#[serial]
async fn test_that_modifies_database() {
    let container = TestContainer::new().await;
    container.clean_database().await.expect("Failed to clean database");
    
    // Test implementation
}

// ✅ CORRECT: Use unique identifiers for test data
async fn setup_test_data(container: &TestContainer) -> AppResult<(Uuid, Uuid, Uuid)> {
    let test_id = Uuid::new_v4().to_string()[..8].to_string();
    
    let country_a = NewCountry {
        iso_code: format!("T{}", &test_id[..2]),
        iso_code_2: format!("T{}", &test_id[..1]),
        name: format!("TEST Country {}", test_id),
        // ... other fields
    };
    
    // Insert and return IDs
}
```

### Test Data Management

```rust
// ✅ CORRECT: Generate unique test data
let test_id = Uuid::new_v4().to_string()[..8].to_string();
let unique_name = format!("test_data_{}", test_id);

// ✅ CORRECT: Clean up after tests
#[tokio::test]
async fn test_with_cleanup() {
    let container = TestContainer::new().await;
    
    // Setup
    container.clean_database().await.expect("Failed to clean database");
    
    // Test implementation
    
    // Cleanup (automatic with TestContainer)
}
```

## Async Programming Best Practices

### Proper Async Error Handling

```rust
// ✅ CORRECT: Proper async error handling
async fn fetch_data() -> Result<Data, AppError> {
    let response = reqwest::get("https://api.example.com/data")
        .await
        .map_err(|e| AppError::network_error(format!("Request failed: {}", e)))?;
    
    let data: Data = response.json()
        .await
        .map_err(|e| AppError::parse_error(format!("JSON parse failed: {}", e)))?;
    
    Ok(data)
}

// ❌ WRONG: Using expect() in async functions
async fn fetch_data() -> Data {
    let response = reqwest::get("https://api.example.com/data")
        .await
        .expect("Request failed");
    
    response.json().await.expect("JSON parse failed")
}
```

### Database Connection Management

```rust
// ✅ CORRECT: Proper connection handling
async fn database_operation(pool: &DatabasePool) -> AppResult<()> {
    let mut conn = pool.get().await
        .map_err(|e| AppError::database_error(format!("Failed to get connection: {}", e)))?;
    
    // Use connection
    diesel_async::RunQueryDsl::execute(
        diesel::sql_query("SELECT 1"),
        &mut conn,
    )
    .await
    .map_err(|e| AppError::database_error(format!("Query failed: {}", e)))?;
    
    Ok(())
}
```

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

// ✅ CORRECT: Use Cow<str> for flexible string handling
use std::borrow::Cow;

fn process_text(text: &str) -> Cow<str> {
    if text.contains("error") {
        Cow::Owned(text.to_uppercase())
    } else {
        Cow::Borrowed(text)
    }
}
```

## Memory Management

### Avoiding Memory Leaks

```rust
// ✅ CORRECT: Proper resource cleanup
use std::sync::Arc;

struct ResourceManager {
    resource: Arc<SomeResource>,
}

impl Drop for ResourceManager {
    fn drop(&mut self) {
        // Cleanup code here
        println!("Cleaning up resource");
    }
}

// ✅ CORRECT: Use RAII patterns
struct DatabaseConnection {
    conn: Option<Connection>,
}

impl Drop for DatabaseConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            // Close connection
        }
    }
}
```

## Code Organization

### Module Structure

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

```rust
// ✅ CORRECT: Comprehensive error types
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
}

// ✅ CORRECT: Error conversion
impl From<diesel::result::Error> for AppError {
    fn from(err: diesel::result::Error) -> Self {
        AppError::DatabaseError(err.to_string())
    }
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

```rust
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

### 1. Using `unwrap()` in Production Code

```rust
// ❌ WRONG: Using unwrap() in production
let value = risky_operation().unwrap();

// ✅ CORRECT: Proper error handling
let value = risky_operation()?;
// OR
let value = risky_operation().expect("This should never fail in this context");
```

### 2. Ignoring Results

```rust
// ❌ WRONG: Ignoring Results
risky_operation();

// ✅ CORRECT: Handle the Result
let _ = risky_operation(); // If intentionally ignoring
// OR
risky_operation().expect("Operation failed");
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

### Cargo Fix

```bash
# Apply automatic fixes
cargo fix --lib --tests

# Check what would be fixed without applying
cargo fix --lib --tests --dry-run

# Fix specific package
cargo fix --lib -p package_name --tests
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

### Memory Profiling

```rust
// ✅ CORRECT: Monitor memory usage
use std::alloc::{GlobalAlloc, System, Layout};

struct MemoryTracker;

unsafe impl GlobalAlloc for MemoryTracker {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        // Track allocation
        ptr
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Track deallocation
        System.dealloc(ptr, layout);
    }
}
```

## Documentation

### Function Documentation

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

```rust
//! # Database Module
//!
//! This module provides database connectivity and operations for the EconGraph project.
//!
//! ## Features
//!
//! - Connection pooling
//! - Async operations
//! - Error handling
//! - Migration support
//!
//! ## Usage
//!
//! ```rust
//! use crate::database::DatabasePool;
//!
//! let pool = DatabasePool::new().await?;
//! let result = pool.execute_query("SELECT 1").await?;
//! ```

pub mod connection;
pub mod migrations;
pub mod queries;
```

## Additional Resources

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Async Book](https://rust-lang.github.io/async-book/)
- [Error Handling in Rust](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
