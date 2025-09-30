# EconGraph Backend

The main backend application for the EconGraph system, providing core server infrastructure, metrics collection, and comprehensive integration testing capabilities. This crate serves as the primary backend service that orchestrates all components and provides the main application entry point.

## Features

- **Server Infrastructure**: Core HTTP server with routing and middleware
- **Metrics Collection**: Application performance and health monitoring
- **Integration Testing**: Comprehensive end-to-end testing capabilities
- **Service Orchestration**: Coordination of all backend services and components
- **Health Monitoring**: System health checks and status reporting
- **Configuration Management**: Centralized configuration and environment handling

## Testing

The crate includes comprehensive tests to ensure backend functionality, server performance, and system integration work correctly.

### Test Types

#### **Unit Tests**
- **Purpose**: Test individual backend components in isolation
- **Examples**: Server configuration, metrics collection, health checks, configuration loading
- **Benefits**: Fast execution, no external dependencies, catch logic errors early

#### **Integration Tests**
- **Purpose**: Test complete backend system with all services and dependencies
- **Examples**: End-to-end API testing, database integration, service coordination, health monitoring
- **Benefits**: Catch integration issues, test real-world scenarios, validate system behavior

#### **Performance Tests**
- **Purpose**: Ensure backend performance meets requirements under load
- **Examples**: Server response times, memory usage, concurrent request handling, metrics collection overhead
- **Benefits**: Prevent performance regressions, validate scalability, ensure efficient resource usage

#### **Health Monitoring Tests**
- **Purpose**: Validate system health monitoring and status reporting
- **Examples**: Health check endpoints, metrics accuracy, status reporting, alerting
- **Benefits**: Ensure monitoring works, validate health reporting, test alerting systems

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test integration_tests
cargo test metrics
cargo test health

# Run with performance testing
PERFORMANCE_TESTING=true cargo test
```

### Test Infrastructure

- **Server Testing**: Complete HTTP server testing with real endpoints
- **Database Integration**: Real database operations with test data setup and cleanup
- **Service Coordination**: Multi-service testing with dependency management
- **Performance Monitoring**: Automated performance testing and benchmarking

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.
