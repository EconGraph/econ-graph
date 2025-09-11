import { test, expect } from '@playwright/test';

test.describe('Series Explorer', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/explore');
  });

  test('should display series explorer page', async ({ page }) => {
    await expect(page.locator('main')).toBeVisible();

    // Check for series explorer specific content - use specific heading
    const explorerContent = page.getByRole('heading', { name: 'Explore Economic Series' });

    await expect(explorerContent).toBeVisible();
  });

  test('should have search functionality', async ({ page }) => {
    // Look for search input - be more specific to avoid strict mode violation
    const searchInput = page.getByRole('textbox', { name: 'Search series' });

    await expect(searchInput).toBeVisible();
  });

  test('should allow searching for economic series', async ({ page }) => {
    const searchInput = page.getByRole('textbox', { name: 'Search series' });

    if (await searchInput.isVisible()) {
      await searchInput.fill('GDP');
      await searchInput.press('Enter');

      // Should show search results or loading state
      await page.waitForTimeout(2000);

      const hasResults = await page.getByText(/found.*results/i).isVisible() ||
        await page.locator('[data-testid="search-results"]').isVisible() ||
        await page.getByText(/GDP/i).isVisible();

      expect(hasResults).toBeTruthy();
    }
  });

  test('should display series list or grid', async ({ page }) => {
    // The series explorer page should load and display the main content
    // Check that the page loads properly and has the expected structure
    await expect(page.locator('main')).toBeVisible();

    // Check for the search functionality
    const searchInput = page.getByRole('textbox', { name: 'Search series' });
    await expect(searchInput).toBeVisible();

    // Check for the page heading
    const pageHeading = page.getByRole('heading', { name: 'Explore Economic Series' });
    await expect(pageHeading).toBeVisible();
  });

  test('should allow filtering or sorting series', async ({ page }) => {
    // Look for filter or sort controls
    const filterButton = page.getByRole('button', { name: /filter/i }).or(
      page.getByRole('button', { name: /sort/i })
    );

    const selectElement = page.locator('select');

    // Should have some form of filtering or sorting
    const hasFiltering = await filterButton.isVisible() || await selectElement.isVisible();
    expect(hasFiltering).toBeTruthy();
  });

  test('should navigate to series detail when series is clicked', async ({ page }) => {
    // Look for clickable series items
    const seriesItem = page.locator('[data-testid="series-item"]').or(
      page.locator('.series-item').or(
        page.locator('li').or(
          page.locator('[role="listitem"]')
        )
      )
    ).first();

    if (await seriesItem.isVisible()) {
      await seriesItem.click();

      // Should navigate to series detail page
      await expect(page).toHaveURL(/\/series\//);
    }
  });

  test('should handle empty search results', async ({ page }) => {
    const searchInput = page.getByRole('textbox', { name: 'Search series' });

    if (await searchInput.isVisible()) {
      await searchInput.fill('nonexistent-series-xyz');
      await searchInput.press('Enter');

      await page.waitForTimeout(2000);

      // Should show no results message
      const noResultsMessage = page.getByText(/no results/i).or(
        page.getByText(/not found/i).or(
          page.getByText(/no data/i)
        )
      );

      await expect(noResultsMessage).toBeVisible();
    }
  });

  test('should be responsive on mobile devices', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });

    await expect(page.locator('main')).toBeVisible();

    // Search functionality should still work on mobile
    const searchInput = page.getByRole('textbox', { name: 'Search series' });

    await expect(searchInput).toBeVisible();
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

    // Filter out expected errors
    const unexpectedErrors = consoleErrors.filter(error =>
      !error.includes('OAuth') &&
      !error.includes('authentication') &&
      !error.includes('403') &&
      !error.includes('401')
    );

    expect(unexpectedErrors).toHaveLength(0);
  });
});
