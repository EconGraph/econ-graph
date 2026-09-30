# EconGraph Services

The business logic and service layer for the EconGraph system: search, series queries, global analysis, crawl-queue statistics and collaboration. Data collection (source adapters, the queue worker and the `crawler` CLI) lives in `econ-graph-crawler`.

## Features

- **Search & Analysis**: Global economic analysis and intelligent search functionality
- **Queue Management**: `crawl_queue` statistics and admin helpers
- **Collaboration**: User collaboration and data sharing features for team workflows

## Testing

The crate includes comprehensive tests to ensure business logic correctness, and database behaviour.

### Test Types

#### **Unit Tests**
- **Purpose**: Test individual services and business logic in isolation
- **Examples**: search algorithms, queue operations
- **Benefits**: Fast execution, no external dependencies, catch logic errors early

#### **Integration Tests**
- **Purpose**: Test services against a real Postgres database
- **Examples**: search functionality, queue statistics
- **Benefits**: Catch integration issues, test real-world scenarios

### Running Tests

Run from `backend/` (use `cd backend` from the repository root); `-p` selects this
crate rather than every default workspace member. First create the disposable
`econ_graph_test` database using the [backend test setup](../../README.md#build-and-test).
Database-backed tests drop and recreate its public schema. Keep Docker available;
the subshell below leaves the application's exported `DATABASE_URL` unchanged.

```bash
(
  export DATABASE_URL=postgresql://postgres:password@localhost:5432/econ_graph_test
  # Run this crate's tests
  cargo test -p econ-graph-services

  # Filter by existing service module names
  cargo test -p econ-graph-services services::search_service
  cargo test -p econ-graph-services services::queue_service
)
```

### Test Infrastructure

- **Database Integration**: Real database operations with test data setup and cleanup
- **Queue Testing**: Asynchronous job processing validation and error handling

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.


