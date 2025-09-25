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

**🎯 Core E2E Testing Philosophy**: E2E tests are designed to test the **ENTIRE system end-to-end, including real network calls to external services. NOT mocking external services is their fundamental purpose.**

#### **Test Suite Descriptions**
- **Core Tests**: Basic functionality (navigation, authentication, dashboard)
- **Global Analysis Tests**: World map, country selection, economic indicators (162 tests)
- **Professional Analysis Tests**: Advanced charting, technical indicators (39 tests)
- **Mobile Tests**: Mobile versions of all test suites
- **Comprehensive Tests**: Integration/workflow tests (excludes specialized suites)

#### **What E2E Tests Do**
- **Make real HTTP requests** to backend APIs
- **Connect to real databases** (test databases, not production)
- **Test complete user workflows** from frontend to backend
- **Verify actual network communication** between services
- **Test with real data flows** and transformations
- **Validate actual authentication flows** and security

#### **⚠️ Practical Limitations in CI Environments**
- **External Services**: Grafana, monitoring dashboards may not be running
- **Third-party APIs**: FRED, BLS, World Bank APIs may have rate limits or require authentication
- **Network Dependencies**: Tests may fail if external services are down
- **Timeouts**: Network calls may take longer than unit test timeouts

#### **🔧 Handling External Service Dependencies**
```typescript
// ✅ GOOD: Test the actual network call when possible
test('grafana dashboard access', async () => {
  // This test WILL make a real HTTP request to Grafana
  const response = await page.goto('http://localhost:30001/health');
  expect(response?.status()).toBe(200);
});

// ⚠️ ACCEPTABLE: Skip tests when external services unavailable
test.skip('grafana dashboard integration', async () => {
  // Only run when Grafana is actually running
  // Skip in CI if Grafana not available
});

// ❌ WRONG: Don't mock external services in E2E tests
test('grafana dashboard with mock', async () => {
  // This defeats the purpose of E2E testing!
  // Use integration tests instead if you need to mock
});
```

#### **🏗️ E2E Test Environment Strategy**
- **Local Development**: All external services running (Grafana, monitoring, etc.)
- **CI Environment**: Core services only (backend, frontend, database)
- **Staging Environment**: Full external service integration
- **Production**: Full end-to-end validation

### CI Pipeline Jobs
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
- E2E tests with optimized Docker containers
- Parallel execution of specialized test suites
- Security audits and license compliance checks

## Troubleshooting

For common CI issues, see:
- [CI Failure Troubleshooting](CI_FAILURE_TROUBLESHOOTING.md)
- [E2E Test Failure Analysis](E2E_TEST_FAILURE_ANALYSIS.md)
- [CI Failure Analysis and Fixes](CI_FAILURE_ANALYSIS_AND_FIXES.md)
