# CI/CD Documentation

This directory contains documentation for the Continuous Integration and Continuous Deployment (CI/CD) infrastructure of the EconGraph project.

## Documentation Files

### CI Pipeline Documentation
- **[CI Failure Analysis and Fixes](CI_FAILURE_ANALYSIS_AND_FIXES.md)** - Analysis of CI failures and their resolutions
- **[CI Failure Troubleshooting](CI_FAILURE_TROUBLESHOOTING.md)** - Guide for troubleshooting common CI issues
- **[E2E Test Failure Analysis](E2E_TEST_FAILURE_ANALYSIS.md)** - Analysis of end-to-end test failures and solutions
- **[Workflow Status Report](workflow-status-report.md)** - Current status of GitHub Actions workflows

### CI Infrastructure
- **[../README.md](../README.md)** - Main CI infrastructure documentation
- **[../configs/](../configs/)** - Playwright test configurations
- **[../docker/](../docker/)** - Docker configurations for CI
- **[../scripts/](../scripts/)** - CI utility scripts

## Test Suite Organization

The CI pipeline includes multiple specialized test suites that run in parallel:

### E2E Test Suites
- **Core Tests**: Basic functionality (navigation, authentication, dashboard)
- **Global Analysis Tests**: World map, country selection, economic indicators (162 tests)
- **Professional Analysis Tests**: Advanced charting, technical indicators (39 tests)
- **Mobile Tests**: Mobile versions of all test suites
- **Comprehensive Tests**: Integration/workflow tests (excludes specialized suites)

### CI Pipeline Jobs
- `backend-build-cache`: Backend compilation and caching
- `backend-tests`: Backend unit tests with PostgreSQL
- `frontend-tests`: Frontend unit tests and coverage
- `quality-checks`: Code formatting, linting, and migration order validation
- `security-audit`: Security vulnerability scanning
- `e2e-core-tests`: Basic functionality tests
- `e2e-global-analysis-tests`: Global analysis features
- `e2e-professional-analysis-tests`: Professional analysis features
- `e2e-comprehensive-tests`: Integration tests
- `mobile-e2e-*`: Mobile versions of all test suites

## Running Tests Locally

```bash
# Run specific test suites
npm run test:e2e:core
npm run test:e2e:global-analysis
npm run test:e2e:professional-analysis
npm run test:e2e:comprehensive

# Run mobile test suites
npm run test:e2e:mobile:core
npm run test:e2e:mobile:global-analysis
npm run test:e2e:mobile:professional-analysis
npm run test:e2e:mobile:comprehensive
```

## CI Configuration

The CI pipeline is configured in `.github/workflows/ci-core.yml` and includes:
- Backend tests with PostgreSQL service containers
- Frontend tests with coverage reporting
- Quality checks including code formatting, linting, and database migration order validation
- E2E tests with optimized Docker containers
- Parallel execution of specialized test suites
- Security audits and license compliance checks

## Troubleshooting

For common CI issues, see:
- [CI Failure Troubleshooting](CI_FAILURE_TROUBLESHOOTING.md)
- [E2E Test Failure Analysis](E2E_TEST_FAILURE_ANALYSIS.md)
- [CI Failure Analysis and Fixes](CI_FAILURE_ANALYSIS_AND_FIXES.md)
