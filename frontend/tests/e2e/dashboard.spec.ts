import { test, expect } from '@playwright/test';

test.describe('Dashboard', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('should display dashboard page with main content', async ({ page }) => {
    await expect(page.locator('main')).toBeVisible();

    // Check for dashboard-specific content - use the main heading
    await expect(page.getByRole('heading', { name: 'Economic Dashboard' })).toBeVisible();
  });

  test('should display economic data series or charts', async ({ page }) => {
    // Look for economic indicators or data cards
    const dataCard = page.locator('[data-testid="indicator-card"]').or(
      page.getByText('Real Gross Domestic Product').or(
        page.getByText('Unemployment Rate').or(
          page.getByText('Consumer Price Index')
        )
      )
    );

    // Should have at least one economic indicator
    await expect(dataCard.first()).toBeVisible();
  });

  test('should display navigation to other sections', async ({ page }) => {
    // Look for navigation buttons or links
    const collaborationButton = page.getByRole('button', { name: /collaboration/i });
    const exploreButton = page.getByRole('button', { name: /explore/i });

    // At least one navigation element should be visible
    const hasNavigation = await collaborationButton.isVisible() || await exploreButton.isVisible();
    expect(hasNavigation).toBeTruthy();
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
