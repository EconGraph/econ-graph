# CI/CD Workflows

This directory contains the CI/CD workflows for the EconGraph project, cleaned up and optimized for better maintainability and reduced costs.

## Active Workflow Files

### Core Tests (`ci-core.yml`) - **PRIMARY WORKFLOW**
**Purpose**: Comprehensive testing that runs on every commit
- 20+ parallel backend test jobs covering all service layers
- Admin Frontend tests (runs in parallel, doesn't gate other tests)
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

### Admin Frontend Tests (`admin-frontend-tests.yml`)
**Purpose**: Comprehensive testing of the admin frontend interface
- Unit tests, integration tests, and E2E tests for admin UI
- Build verification and quality checks
- Runs independently and doesn't gate other workflows

**Triggers**: Push/PR to admin-frontend, manual dispatch

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
├── admin-frontend-tests (parallel, non-gating)
├── backend-database-tests (needs: backend-smoke-tests)
├── backend-service-tests (needs: backend-smoke-tests)
├── frontend-tests
└── quality-checks
```

## Environment Variables

All workflows share these environment variables:
- `CARGO_TERM_COLOR: always` - Colored Rust output
- `DATABASE_URL: postgresql://postgres:password@localhost:5432/econ_graph_test` - Test database connection

## Database Configuration

All CI workflows now use **PostgreSQL 18** with the following benefits:
- **UUIDv7 Support**: Native UUIDv7 generation with `uuidv7()` function
- **Performance**: Better performance for UUID-based primary keys
- **Future-Proof**: Latest PostgreSQL features and optimizations
- **Consistency**: All test environments use the same PostgreSQL version

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
- **Admin Frontend Tests**: `gh workflow run admin-frontend-tests.yml`
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

## Workflow Validation

### Automated Validation Script

Use the `ci/scripts/validate-ci-workflows.sh` script to validate all GitHub Actions CI/CD workflows before committing changes:

```bash
# Run CI/CD workflow validation
./ci/scripts/validate-ci-workflows.sh
```

### Validation Checks

The script performs the following checks:

1. **YAML Syntax Validation**: Ensures all workflow files have valid YAML syntax
2. **Job Structure Validation**: Verifies all jobs have proper `steps` sections
3. **Orphaned Workflow Detection**: Identifies workflows with no active triggers
4. **Naming Consistency**: Ensures workflows have descriptive names

### Integration

- **Pre-commit**: Run validation before committing workflow changes
- **CI Pipeline**: Validation is integrated into the main CI pipeline
- **Error Reporting**: Provides clear, actionable error messages with color-coded output

### Common Issues Detected

- **Invalid Job Definitions**: Jobs without `steps` sections (causes 0s duration failures)
- **Orphaned Workflows**: Workflows from deleted branches showing as active in GitHub
- **Malformed YAML**: Syntax errors that prevent workflow parsing
- **Missing Triggers**: Workflows that can't be executed
- **Poor Naming**: Workflows without descriptive names

### Example Output

```bash
🔍 Validating GitHub Actions CI/CD workflows...
📋 Checking YAML syntax...
✅ ci-core.yml - Valid YAML syntax
❌ ci-core.yml - Job structure issues found
⚠️  experimental.yml - No active triggers (may be orphaned)
📊 Validation Summary:
❌ Found 1 validation errors
💡 Fix the errors above before committing workflow changes
```