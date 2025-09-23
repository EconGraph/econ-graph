# Playwright E2E Testing Best Practices 2025

**Version**: 1.0  
**Last Updated**: September 2025  
**Project**: EconGraph Frontend Testing

## Table of Contents

1. [Overview](#overview)
2. [Selector Strategy: The Great Debate](#selector-strategy-the-great-debate)
3. [Accessibility-First Testing](#accessibility-first-testing)
4. [Test Organization & Structure](#test-organization--structure)
5. [Performance & Reliability](#performance--reliability)
6. [CI/CD Integration](#cicd-integration)
7. [Advanced Patterns](#advanced-patterns)
8. [Common Pitfalls](#common-pitfalls)
9. [EconGraph-Specific Guidelines](#econgraph-specific-guidelines)

## Overview

This document outlines the latest best practices for Playwright E2E testing as of 2025, specifically tailored for the EconGraph project. These practices ensure robust, maintainable, and user-focused testing that aligns with modern web accessibility standards.

### Key Principles

- **User-Centric**: Test what users actually see and interact with
- **Accessibility-First**: Ensure tests verify accessible user experiences
- **Maintainable**: Write tests that survive UI changes
- **Reliable**: Minimize flaky tests through proper waiting strategies
- **Performance**: Optimize for fast CI/CD pipelines

## Selector Strategy: The Great Debate

### The 2025 Consensus: Semantic Selectors First

The testing community has reached consensus on selector priority:

#### 1. **Semantic/Accessibility Selectors (Primary)**
```typescript
// ✅ BEST: Test what users actually see and interact with
await page.getByRole('button', { name: 'Login' });
await page.getByRole('heading', { name: 'Global Economic Analysis' });
await page.getByRole('textbox', { name: 'Country Search' });
await page.getByRole('link', { name: 'Dashboard' });

// ✅ GOOD: Test semantic structure
await page.getByLabel('Email address');
await page.getByPlaceholder('Enter country name...');
await page.getByText('Welcome to EconGraph');
```

#### 2. **Visual Hierarchy Selectors (Secondary)**
```typescript
// ✅ GOOD: Test document structure
await page.locator('h1').first();
await page.locator('main nav');
await page.locator('aside[aria-label="Sidebar"]');
```

#### 3. **data-testid Selectors (Tertiary - Use Sparingly)**
```typescript
// ⚠️ USE ONLY WHEN: No semantic alternative exists
await page.locator('[data-testid="chart-container"]');
await page.locator('[data-testid="complex-widget"]');

// ❌ AVOID: Generic elements with testids
<div data-testid="button">Login</div>  // Use getByRole('button') instead
```

### When to Use Each Strategy

| Scenario | Recommended Approach | Example |
|----------|---------------------|---------|
| **User Interactions** | `getByRole()` | `page.getByRole('button', { name: 'Submit' })` |
| **Form Fields** | `getByLabel()` or `getByPlaceholder()` | `page.getByLabel('Email')` |
| **Navigation** | `getByRole('link')` | `page.getByRole('link', { name: 'Dashboard' })` |
| **Content Verification** | `getByText()` or `getByRole('heading')` | `page.getByText('Welcome')` |
| **Complex Components** | `data-testid` (as last resort) | `page.locator('[data-testid="chart"]')` |

## Accessibility-First Testing

### Why Accessibility Testing Matters in E2E

E2E tests should verify that your application is usable by all users, including those using assistive technologies.

```typescript
// ✅ Test accessibility alongside functionality
test('navigation is accessible', async ({ page }) => {
  await page.goto('/');
  
  // Test that navigation is properly structured
  await expect(page.getByRole('navigation')).toBeVisible();
  
  // Test that links have proper labels
  await expect(page.getByRole('link', { name: 'Global Analysis' })).toBeVisible();
  
  // Test keyboard navigation
  await page.keyboard.press('Tab');
  await expect(page.locator(':focus')).toHaveRole('link');
});

// ✅ Test form accessibility
test('search form is accessible', async ({ page }) => {
  await page.goto('/global-analysis');
  
  // Test that form has proper labels
  const searchInput = page.getByLabel('Country Search');
  await expect(searchInput).toBeVisible();
  
  // Test that form validation is accessible
  await searchInput.fill('Invalid Country');
  await page.getByRole('button', { name: 'Search' }).click();
  
  // Test that error messages are properly announced
  await expect(page.getByRole('alert')).toBeVisible();
});
```

### ARIA Role Testing

```typescript
// ✅ Test ARIA roles and properties
test('chart component has proper ARIA attributes', async ({ page }) => {
  await page.goto('/dashboard');
  
  const chart = page.getByRole('img', { name: 'Economic Indicators Chart' });
  await expect(chart).toBeVisible();
  await expect(chart).toHaveAttribute('alt', 'Economic Indicators Chart');
  
  // Test that chart data is accessible
  await expect(page.getByRole('region', { name: 'Chart Data' })).toBeVisible();
});
```

## Test Organization & Structure

### Recommended Folder Structure

```
frontend/tests/e2e/
├── core/                    # Basic functionality tests
│   ├── navigation.spec.ts
│   ├── authentication.spec.ts
│   └── dashboard.spec.ts
├── features/                # Feature-specific tests
│   ├── global-analysis.spec.ts
│   ├── financial-data.spec.ts
│   └── charts.spec.ts
├── comprehensive/           # Integration tests
│   └── complete-workflow.spec.ts
├── mobile/                  # Mobile-specific tests
│   ├── mobile-navigation.spec.ts
│   └── mobile-charts.spec.ts
├── fixtures/                # Shared test utilities
│   ├── auth.fixture.ts
│   ├── data.fixture.ts
│   └── page-objects/
│       ├── DashboardPage.ts
│       ├── GlobalAnalysisPage.ts
│       └── AuthPage.ts
└── utils/                   # Test utilities
    ├── test-data.ts
    └── helpers.ts
```

### Page Object Model (POM) Implementation

```typescript
// fixtures/page-objects/DashboardPage.ts
export class DashboardPage {
  constructor(private page: Page) {}

  // Use semantic selectors
  async navigateToGlobalAnalysis(): Promise<void> {
    await this.page.getByRole('link', { name: 'Global Analysis' }).click();
  }

  async searchCountry(country: string): Promise<void> {
    await this.page.getByLabel('Country Search').fill(country);
    await this.page.getByRole('button', { name: 'Search' }).click();
  }

  async expectWelcomeMessage(): Promise<void> {
    await expect(this.page.getByRole('heading', { name: 'Welcome' })).toBeVisible();
  }

  // Only use data-testid for complex components
  async getChartData(): Promise<string> {
    return await this.page.locator('[data-testid="chart-data"]').textContent();
  }
}

// Register as fixture
export const test = base.extend<{ dashboardPage: DashboardPage }>({
  dashboardPage: async ({ page }, use) => {
    const dashboardPage = new DashboardPage(page);
    await use(dashboardPage);
  },
});
```

## Performance & Reliability

### Automatic Waits & Retries

```typescript
// ✅ GOOD: Use Playwright's built-in waiting
await page.getByRole('button', { name: 'Load Data' }).click();
await expect(page.getByText('Data loaded successfully')).toBeVisible();

// ✅ GOOD: Use locator.waitFor() for complex states
await page.locator('[data-testid="chart-container"]').waitFor({ state: 'visible' });

// ❌ AVOID: Arbitrary sleeps
await page.getByRole('button').click();
await page.waitForTimeout(5000); // Don't do this
```

### Test Isolation

```typescript
// ✅ GOOD: Each test runs in fresh context
test.describe('User Authentication', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/login');
  });

  test('can login with valid credentials', async ({ page, authPage }) => {
    await authPage.login('user@example.com', 'password');
    await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible();
  });

  test('shows error for invalid credentials', async ({ page, authPage }) => {
    await authPage.login('user@example.com', 'wrong-password');
    await expect(page.getByRole('alert')).toContainText('Invalid credentials');
  });
});
```

### Parallel Execution

```typescript
// playwright.config.ts
export default defineConfig({
  fullyParallel: true, // Run tests in parallel
  workers: process.env.CI ? 2 : undefined, // Limit workers in CI
  retries: process.env.CI ? 2 : 0, // Retry failed tests in CI
});
```

## CI/CD Integration

### Health Check Best Practices

```bash
# ✅ GOOD: Proper health check with delays
timeout 60 bash -c 'until curl -f http://localhost:8080/health; do sleep 2; done'

# ✅ GOOD: Loop with proper sleep
for i in {1..30}; do
  if curl -f http://localhost:8080/health 2>/dev/null; then
    echo "Backend is ready!"
    break
  fi
  sleep 2  # CRITICAL: Always include sleep in loops
done
```

### Trace and Video Recording

```typescript
// playwright.config.ts
export default defineConfig({
  use: {
    trace: 'on-first-retry', // Record traces on retries
    video: 'retain-on-failure', // Record videos on failure
  },
});
```

## Advanced Patterns

### Soft Assertions

```typescript
// ✅ GOOD: Use soft assertions to capture multiple failures
test('dashboard loads all components', async ({ page }) => {
  await page.goto('/dashboard');
  
  // Use soft assertions to continue testing even if one fails
  await expect.soft(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible();
  await expect.soft(page.getByRole('navigation')).toBeVisible();
  await expect.soft(page.locator('[data-testid="chart-container"]')).toBeVisible();
  await expect.soft(page.getByRole('button', { name: 'Refresh Data' })).toBeVisible();
});
```

### Environment-Specific Configuration

```typescript
// playwright.config.ts
export default defineConfig({
  projects: [
    {
      name: 'chromium',
      use: { 
        ...devices['Desktop Chrome'],
        baseURL: process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:3000'
      },
    },
    {
      name: 'mobile-chromium',
      use: { 
        ...devices['Pixel 5'],
        baseURL: process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:3000'
      },
    },
  ],
});
```

## Common Pitfalls

### ❌ Don't Do This

```typescript
// ❌ Using data-testid for everything
await page.locator('[data-testid="login-button"]').click();
await page.locator('[data-testid="search-input"]').fill('test');

// ❌ Using arbitrary waits
await page.waitForTimeout(5000);

// ❌ Testing implementation details
await page.locator('.btn-primary').click(); // Brittle CSS selector

// ❌ Not testing accessibility
await page.locator('button').click(); // No role or name verification
```

### ✅ Do This Instead

```typescript
// ✅ Use semantic selectors
await page.getByRole('button', { name: 'Login' }).click();
await page.getByLabel('Search').fill('test');

// ✅ Use automatic waits
await expect(page.getByText('Success')).toBeVisible();

// ✅ Test user-visible behavior
await page.getByRole('button', { name: 'Submit Form' }).click();

// ✅ Test accessibility
await expect(page.getByRole('button', { name: 'Login' })).toBeVisible();
```

## EconGraph-Specific Guidelines

### Testing Economic Data Components

```typescript
// ✅ Test chart accessibility
test('economic chart is accessible', async ({ page }) => {
  await page.goto('/global-analysis');
  await page.getByLabel('Country Search').fill('United States');
  await page.getByRole('button', { name: 'Search' }).click();
  
  // Test that chart has proper ARIA attributes
  const chart = page.getByRole('img', { name: /Economic indicators for United States/ });
  await expect(chart).toBeVisible();
  
  // Test that chart data is accessible to screen readers
  await expect(page.getByRole('region', { name: 'Chart Data Table' })).toBeVisible();
});

// ✅ Test data loading states
test('shows loading state while fetching data', async ({ page }) => {
  await page.goto('/global-analysis');
  await page.getByLabel('Country Search').fill('Germany');
  await page.getByRole('button', { name: 'Search' }).click();
  
  // Test loading state
  await expect(page.getByRole('status', { name: 'Loading data' })).toBeVisible();
  
  // Test data appears
  await expect(page.getByText('Economic indicators for Germany')).toBeVisible();
});
```

### Testing Global Analysis Features

```typescript
// ✅ Test world map interactions
test('world map is interactive and accessible', async ({ page }) => {
  await page.goto('/global-analysis');
  
  // Test that map is accessible
  const map = page.getByRole('img', { name: 'World Map' });
  await expect(map).toBeVisible();
  
  // Test country selection
  await page.getByRole('button', { name: 'Select United States' }).click();
  await expect(page.getByText('Economic indicators for United States')).toBeVisible();
});
```

### Testing Financial Data Integration

```typescript
// ✅ Test financial data display
test('financial data table is accessible', async ({ page }) => {
  await page.goto('/financial-data');
  
  // Test table structure
  const table = page.getByRole('table', { name: 'Financial Data' });
  await expect(table).toBeVisible();
  
  // Test table headers
  await expect(page.getByRole('columnheader', { name: 'Company' })).toBeVisible();
  await expect(page.getByRole('columnheader', { name: 'Revenue' })).toBeVisible();
  
  // Test data rows
  await expect(page.getByRole('row', { name: /Apple Inc/ })).toBeVisible();
});
```

## Conclusion

Following these 2025 best practices ensures that your Playwright E2E tests are:

- **User-focused**: Test what users actually see and interact with
- **Accessible**: Verify that your app works for all users
- **Maintainable**: Survive UI changes and refactoring
- **Reliable**: Minimize flaky tests through proper patterns
- **Fast**: Optimized for CI/CD pipelines

Remember: The goal of E2E testing is to verify that your application provides a great user experience, not to test implementation details.

---

**Next Steps:**
1. Review existing tests against these guidelines
2. Update selectors to use semantic approaches
3. Add accessibility testing to critical user flows
4. Implement proper health checks in CI
5. Set up trace and video recording for debugging

**Resources:**
- [Playwright Documentation](https://playwright.dev/)
- [Web Accessibility Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [EconGraph Testing Strategy](../technical/TESTING_STRATEGY.md)
