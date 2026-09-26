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

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test services::search
cargo test services::queue_service
cargo test integration
```

### Test Infrastructure

- **Database Integration**: Real database operations with test data setup and cleanup
- **Queue Testing**: Asynchronous job processing validation and error handling

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.
