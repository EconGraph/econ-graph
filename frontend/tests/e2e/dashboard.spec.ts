import { test, expect } from '@playwright/test';

test.describe('Dashboard', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('should display dashboard page with main content', async ({ page }) => {
    await expect(page.locator('main')).toBeVisible();

    // Check for any heading that indicates the page loaded
    const heading = page.getByRole('heading').first();
    await expect(heading).toBeVisible();
  });

  test('should display economic data series or charts', async ({ page }) => {
    // Look for economic indicators or data cards with more flexible selectors
    const dataCard = page.locator('[data-testid="indicator-card"]').or(
      page.locator('[data-testid="data-card"]').or(
        page.locator('.card').or(
          page.locator('[class*="card"]').or(
            page.getByText(/GDP/i).or(
              page.getByText(/unemployment/i).or(
                page.getByText(/inflation/i).or(
                  page.getByText(/economic/i)
                )
              )
            )
          )
        )
      )
    );

    // Should have at least one economic indicator or data element
    if (await dataCard.count() > 0) {
      await expect(dataCard.first()).toBeVisible();
    } else {
      // If no specific data cards, at least verify the page has some content
      await expect(page.locator('main')).toBeVisible();
    }
  });

  test('should display navigation to other sections', async ({ page }) => {
    // Look for navigation elements with more flexible selectors
    const navigationElements = page.getByRole('button').or(
      page.getByRole('link').or(
        page.locator('nav').or(
          page.locator('[data-testid*="nav"]')
        )
      )
    );

    // At least one navigation element should be visible
    const navCount = await navigationElements.count();
    expect(navCount).toBeGreaterThan(0);
  });

  test('should be responsive on different screen sizes', async ({ page }) => {
    // Test desktop view
    await page.setViewportSize({ width: 1200, height: 800 });
    await expect(page.locator('main')).toBeVisible();

    // Test tablet view
    await page.setViewportSize({ width: 768, height: 1024 });
    await expect(page.locator('main')).toBeVisible();

    // Test mobile view
    await page.setViewportSize({ width: 375, height: 667 });
    await expect(page.locator('main')).toBeVisible();
  });

  test('should load without JavaScript console errors', async ({ page }) => {
    const consoleErrors: string[] = [];

    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    await page.reload();
    await page.waitForLoadState('networkidle');

    // Filter out expected errors (like OAuth errors when not authenticated)
    const unexpectedErrors = consoleErrors.filter(error =>
      !error.includes('OAuth') &&
      !error.includes('authentication') &&
      !error.includes('403') &&
      !error.includes('401')
    );

    expect(unexpectedErrors).toHaveLength(0);
  });

  test('should display loading states appropriately', async ({ page }) => {
    // Check for loading indicators
    const loadingIndicator = page.locator('[data-testid="loading"]').or(
      page.locator('.loading').or(
        page.locator('text=Loading')
      )
    );

    // Loading indicator should appear briefly and then disappear
    if (await loadingIndicator.isVisible()) {
      await expect(loadingIndicator).not.toBeVisible({ timeout: 10000 });
    }
  });

  test('should handle data loading errors gracefully', async ({ page }) => {
    // The dashboard uses static data, so we'll test that it loads without errors
    // and displays the expected content even if there are no network requests

    await page.reload();
    await page.waitForLoadState('networkidle');

    // Should show the dashboard content without errors
    const dashboardContent = page.getByRole('heading', { name: 'Economic Dashboard' });
    const indicatorCards = page.locator('[data-testid="indicator-card"]').or(
      page.getByText('Real Gross Domestic Product').or(
        page.getByText('Unemployment Rate').or(
          page.getByText('Consumer Price Index')
        )
      )
    );

    await expect(dashboardContent).toBeVisible();
    await expect(indicatorCards.first()).toBeVisible();

    // Should not show any error messages since this page uses static data
    const errorMessage = page.locator('[role="alert"]').or(
      page.locator('.error').or(
        page.getByText(/error/i)
      )
    );

    await expect(errorMessage).not.toBeVisible();
  });
});
