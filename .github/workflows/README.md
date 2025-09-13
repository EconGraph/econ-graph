# CI/CD Workflows

This directory contains the CI/CD workflows for the EconGraph project, split into logical, manageable pieces for better maintainability and understanding.

## Workflow Files

### Core Tests (`ci-core.yml`)
**Purpose**: Basic functionality tests that should run on every commit
- `backend-smoke-tests` - Quick backend health checks
- `backend-database-tests` - Database-specific tests (depends on smoke tests)
- `backend-service-tests` - Service layer tests (depends on smoke tests)
- `frontend-tests` - Frontend unit tests and linting
- `quality-checks` - Code formatting and linting checks

**Triggers**: Push to main/develop, PRs, manual dispatch

### Integration Tests (`ci-integration.yml`)
**Purpose**: End-to-end and integration testing
- `backend-integration-tests` - Full backend integration tests (depends on core backend tests)
- `frontend-integration-tests` - Frontend integration and E2E tests (depends on frontend tests)

**Triggers**: Push to main/develop, PRs, manual dispatch

### Security & Compliance (`ci-security.yml`)
**Purpose**: Security audits and license compliance
- `security-audit` - Rust and NPM security vulnerability scanning
- `license-compliance` - License checking for all dependencies

**Triggers**: Push to main/develop, PRs, manual dispatch

### Build & Deploy (`ci-build.yml`)
**Purpose**: Building and validating deployment artifacts
- `grafana-dashboard-validation` - Validates Grafana dashboard JSON files
- `docker-build` - Builds and tests Docker images (depends on integration tests)

**Triggers**: Push to main/develop, PRs, manual dispatch

### Experimental (`ci-experimental.yml`)
**Purpose**: Experimental features and performance testing
- `experimental-ramdisk-build-cache` - Tests build performance with RAM disk caching

**Triggers**: Manual dispatch only (with experiment parameter)

## Workflow Dependencies

```
Core Tests (ci-core.yml)
├── backend-smoke-tests
├── backend-database-tests (needs: backend-smoke-tests)
├── backend-service-tests (needs: backend-smoke-tests)
├── frontend-tests
└── quality-checks

Integration Tests (ci-integration.yml)
├── backend-integration-tests (needs: backend-smoke-tests, backend-database-tests, backend-service-tests)
└── frontend-integration-tests (needs: frontend-tests)

Build & Deploy (ci-build.yml)
└── docker-build (needs: backend-integration-tests, frontend-integration-tests)
```

## Environment Variables

All workflows share these environment variables:
- `CARGO_TERM_COLOR: always` - Colored Rust output
- `DATABASE_URL: postgresql://postgres:password@localhost:5432/econ_graph_test` - Test database connection

## Benefits of This Structure

1. **Easier to Understand**: Each workflow has a clear, focused purpose
2. **Faster Feedback**: Core tests run independently and provide quick feedback
3. **Better Debugging**: Issues are isolated to specific workflow files
4. **Selective Running**: You can run specific types of tests manually
5. **Maintainable**: Smaller files are easier to modify and review

## Running Workflows

### Automatic Triggers
- **Push to main/develop**: All workflows run automatically
- **Pull Requests**: All workflows run automatically

### Manual Triggers
- **Core Tests**: `gh workflow run ci-core.yml`
- **Integration Tests**: `gh workflow run ci-integration.yml`
- **Security Checks**: `gh workflow run ci-security.yml`
- **Build & Deploy**: `gh workflow run ci-build.yml`
- **Experimental**: `gh workflow run ci-experimental.yml --field experiment=ramdisk`

## Migration Notes

- The original monolithic `ci.yml` has been backed up as `ci-original-backup.yml`
- All job dependencies and environment variables have been preserved
- The new structure maintains the same test coverage and quality gates