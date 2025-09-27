# EconGraph Core

The foundational crate for the EconGraph system, providing core data models, database schema, and shared utilities. This crate serves as the central data layer that other crates in the workspace depend on, offering type-safe database models, authentication systems, and business logic for economic and financial data processing. It handles everything from user management and OAuth integration to complex economic time series calculations and SEC filing data structures.

## Features

- **Database Models**: Economic and financial data models with type safety
- **Authentication**: User management, OAuth integration, and JWT tokens
- **Database Integration**: Connection pooling, migrations, and async operations
- **Error Handling**: Structured error types with logging and tracing

## Testing

The crate includes comprehensive tests to ensure data integrity and business logic correctness.

### Test Types

#### **Unit Tests**
- **Purpose**: Test individual functions and business logic in isolation
- **Examples**: Economic calculations (YoY/QoQ changes), enum conversions, data validation
- **Benefits**: Fast execution, no database dependencies, catch logic errors early

#### **Integration Tests** 
- **Purpose**: Test database operations with real PostgreSQL using testcontainers
- **Examples**: CRUD operations, complex queries, data transformations, concurrent operations
- **Benefits**: Catch database-specific issues, test real-world scenarios, validate data integrity

#### **Schema Validation Tests**
- **Purpose**: Ensure database schema stays in sync with code definitions
- **Process**: Compares generated schema against existing schema definitions
- **Benefits**: Prevents schema drift, catches migration issues, ensures consistency

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test models::company
cargo test database

# Run with external database
DATABASE_URL=postgres://localhost/test cargo test
```

### Test Infrastructure

- **TestContainer**: Each test gets a fresh PostgreSQL instance for complete isolation
- **Automatic Cleanup**: Database state is reset between tests to prevent interference
- **Schema Management**: Migrations are automatically applied and validated for each test
- **Flexible Configuration**: Supports both containerized and external database testing

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.
