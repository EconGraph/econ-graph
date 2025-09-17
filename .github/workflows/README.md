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

## Manual-Only Workflows

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
- **Security Checks**: `gh workflow run security.yml`
- **Crawler Integration**: `gh workflow run crawler-integration-test.yml`
- **Experimental**: `gh workflow run ci-experimental.yml --field experiment=ramdisk`
- **RAM Disk Cache**: `gh workflow run ramdisk-build-cache.yml`

## Cleanup Summary

- **Removed broken workflows** (ci-build.yml, ci-integration.yml, ci-security.yml) that had no active triggers
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