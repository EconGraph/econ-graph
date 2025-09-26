# E2E Testing Strategy

## Overview

This document outlines the E2E (End-to-End) testing strategy for the EconGraph project, including the separation of E2E tests from core CI and the implementation of nightly E2E test runs.

## E2E Test Philosophy

**🎯 Core E2E Testing Philosophy**: E2E tests are designed to test the **ENTIRE system end-to-end, including real network calls to external services. NOT mocking external services is their fundamental purpose.**

### What E2E Tests Do
- **Make real HTTP requests** to backend APIs
- **Connect to real databases** (test databases, not production)
- **Test complete user workflows** from frontend to backend
- **Verify actual network communication** between services
- **Test with real data flows** and transformations
- **Validate actual authentication flows** and security

### What E2E Tests Don't Do
- **Don't mock external services** - this defeats the purpose of E2E testing
- **Don't skip network calls** - real network communication is the point
- **Don't use fake data** - use real test data when possible

## Test Suite Organization

The E2E tests are organized into specialized test suites that run in parallel:

### Desktop Test Suites
- **Core Tests**: Basic functionality (navigation, authentication, dashboard)
- **Global Analysis Tests**: World map, country selection, economic indicators (162 tests)
- **Professional Analysis Tests**: Advanced charting, technical indicators (39 tests)
- **Comprehensive Tests**: Integration/workflow tests (excludes specialized suites)

### Mobile Test Suites
- **Mobile Core Tests**: Mobile versions of basic functionality
- **Mobile Global Analysis Tests**: Mobile versions of global analysis features
- **Mobile Professional Analysis Tests**: Mobile versions of professional analysis features
- **Mobile Comprehensive Tests**: Mobile versions of comprehensive tests

## CI/CD Strategy

### Core CI (Disabled E2E Tests by Default)

The main CI pipeline (`ci-core.yml`) now has E2E tests **disabled by default** to prevent CI noise and improve development velocity. E2E tests can be manually triggered when needed.

#### Manual E2E Test Execution

To run E2E tests in the core CI:

1. Go to the GitHub Actions tab
2. Select "Core CI Tests" workflow
3. Click "Run workflow"
4. Check "Run E2E tests (disabled by default)"
5. Select the specific test suite to run:
   - `all` - Run all E2E test suites
   - `core` - Basic functionality tests
   - `global-analysis` - Global analysis features
   - `professional-analysis` - Professional analysis features
   - `comprehensive` - Integration tests
   - `mobile-core` - Mobile basic functionality
   - `mobile-global-analysis` - Mobile global analysis
   - `mobile-professional-analysis` - Mobile professional analysis
   - `mobile-comprehensive` - Mobile comprehensive tests

### Nightly E2E Tests

A dedicated nightly E2E test workflow (`e2e-tests-nightly.yml`) runs all E2E tests every night at 2 AM UTC. This ensures comprehensive E2E testing without blocking development.

#### Nightly Workflow Features

- **Scheduled Execution**: Runs automatically every night at 2 AM UTC
- **Manual Triggering**: Can be triggered manually with specific test suite selection
- **Comprehensive Coverage**: Tests all E2E test suites including mobile variants
- **Optimized Containers**: Uses pre-built Docker containers for faster execution
- **Artifact Retention**: Keeps test results and reports for 7 days
- **Parallel Execution**: Runs test suites in parallel for efficiency

#### Nightly Workflow Dependencies

The nightly E2E tests include all necessary dependencies:

1. **Backend Services**: PostgreSQL, Rust backend with migrations
2. **Frontend Services**: React frontend with build
3. **E2E Test Containers**: Pre-built containers with Playwright browsers
4. **Test Configurations**: Multiple test suites (core, analysis, comprehensive, global-analysis, professional-analysis)

## Running Tests Locally

### Desktop Tests
```bash
# Run specific test suites
npm run test:e2e:core
npm run test:e2e:global-analysis
npm run test:e2e:professional-analysis
npm run test:e2e:comprehensive
```

### Mobile Tests
```bash
# Run mobile test suites
npm run test:e2e:mobile:core
npm run test:e2e:mobile:global-analysis
npm run test:e2e:mobile:professional-analysis
npm run test:e2e:mobile:comprehensive
```

## Test Environment Strategy

### Local Development
- **All external services running** (Grafana, monitoring, etc.)
- **Full E2E test execution** with real network calls
- **Complete test coverage** including external service integration

### CI Environment
- **Core services only** (backend, frontend, database)
- **External services may not be available** (expected behavior)
- **Tests may skip when external services unavailable** (acceptable)

### Staging Environment
- **Full external service integration**
- **Complete E2E validation**
- **Production-like testing**

### Production
- **Full end-to-end validation**
- **Real user workflow testing**
- **Complete system integration**

## Handling External Service Dependencies

### ✅ Good Practices
```typescript
// Test the actual network call when possible
test('grafana dashboard access', async () => {
  // This test WILL make a real HTTP request to Grafana
  const response = await page.goto('http://localhost:30001/health');
  expect(response?.status()).toBe(200);
});
```

### ⚠️ Acceptable Practices
```typescript
// Skip tests when external services unavailable
test.skip('grafana dashboard integration', async () => {
  // Only run when Grafana is actually running
  // Skip in CI if Grafana not available
});
```

### ❌ Wrong Practices
```typescript
// Don't mock external services in E2E tests
test('grafana dashboard with mock', async () => {
  // This defeats the purpose of E2E testing!
  // Use integration tests instead if you need to mock
});
```

## Benefits of This Strategy

### Development Velocity
- **Faster CI runs** - E2E tests don't block development
- **Reduced CI noise** - Failed E2E tests don't block merges
- **Focused testing** - Core functionality tested quickly

### Comprehensive Testing
- **Nightly E2E coverage** - All E2E tests run every night
- **Manual E2E execution** - E2E tests available when needed
- **Complete test coverage** - Both fast and comprehensive testing

### Resource Efficiency
- **Optimized containers** - Pre-built containers for faster execution
- **Parallel execution** - Test suites run in parallel
- **Smart scheduling** - E2E tests run during off-peak hours

## Troubleshooting

### E2E Test Failures
- Check the nightly E2E test results for comprehensive failure analysis
- Use manual E2E test execution for debugging specific issues
- Review test logs and artifacts for detailed failure information

### CI Issues
- Core CI should now run faster without E2E test dependencies
- Use manual E2E test execution when E2E testing is needed
- Check nightly E2E test results for overall system health

## Future Improvements

- **Test result analysis** - Automated analysis of E2E test trends
- **Performance monitoring** - Track E2E test execution times
- **Failure prediction** - Identify patterns in E2E test failures
- **Test optimization** - Further optimize E2E test execution

## Related Documentation

- [CI/CD Documentation](../ci/docs/README.md) - Comprehensive CI/CD documentation
- [E2E Test Failure Analysis](../ci/docs/E2E_TEST_FAILURE_ANALYSIS.md) - E2E test troubleshooting
- [RelEng Persona](../../personas/releng-engineer.md) - Release engineering practices
- [AI Developer Standards](../../personas/ai-developer-standards.md) - Development guidelines
