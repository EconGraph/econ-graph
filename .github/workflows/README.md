# CI/CD Workflows

This directory contains the CI/CD workflows for the EconGraph project, cleaned up and optimized for better maintainability and reduced costs.

## Active Workflow Files

### Core Tests (`ci-core.yml`) - **PRIMARY WORKFLOW**
**Purpose**: Comprehensive testing that runs on every commit
- 20+ parallel backend test jobs covering all service layers
- Frontend tests, quality checks, security audits, and E2E tests
- All essential functionality validation

**Triggers**: Push to main/develop, PRs, manual dispatch

### Security (`security.yml`)
**Purpose**: Daily security vulnerability scanning
- Rust and NPM security audits
- License compliance checking

**Triggers**: Daily at 2 AM UTC, manual dispatch

### Crawler Integration Test (`crawler-integration-test.yml`)
**Purpose**: Manual testing of data crawler functionality
- Tests specific data source crawling
- Validates migration generation

**Triggers**: Manual dispatch only

### Playwright Tests (`playwright-tests*.yml`)
**Purpose**: End-to-end testing on version releases
- Comprehensive E2E testing
- Mobile and desktop browser testing

**Triggers**: Version tags (v*)

## Disabled Workflows (Available for Manual Use)

### Integration Tests (`ci-integration.yml`)
**Purpose**: Alternative integration testing approach
**Status**: Disabled (use ci-core.yml instead)

### Security & Compliance (`ci-security.yml`)
**Purpose**: Alternative security testing approach  
**Status**: Disabled (use security.yml instead)

### Build & Deploy (`ci-build.yml`)
**Purpose**: Docker build validation
**Status**: Disabled (can be enabled manually when needed)

### Experimental (`ci-experimental.yml`)
**Purpose**: Performance testing and experimental features
**Status**: Manual dispatch only

### RAM Disk Build Cache (`ramdisk-build-cache.yml`)
**Purpose**: Build performance optimization testing
**Status**: Manual dispatch only

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

## Cleanup Summary

- **Removed 9 experimental test workflows** that were disabled and causing confusion
- **Removed backup and disabled workflow files** that were no longer needed
- **Cleaned up excessive CI infrastructure** including unused scripts and documentation
- **Consolidated to essential workflows** with clear purposes and triggers
- **Maintained full test coverage** through the primary ci-core.yml workflow

## Cost Optimization

This cleanup eliminates:
- Dead workflows that could trigger accidentally
- Redundant CI infrastructure and documentation
- Confusion about which workflows are active
- Potential costs from unused or experimental workflows

The remaining workflows provide comprehensive testing while being clearly organized and cost-effective.