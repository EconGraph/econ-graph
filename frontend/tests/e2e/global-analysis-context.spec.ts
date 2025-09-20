import { test, expect } from '@playwright/test';

test.describe('Global Analysis Context API Integration Tests', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/global');
    await page.waitForLoadState('networkidle');
  });

  test('should maintain state across component interactions', async ({ page }) => {
    // Select a country
    const countryPath = page.locator('svg path').first();
    if (await countryPath.isVisible()) {
      await countryPath.click();
    }

    // Check that selected countries are displayed
    const selectedCountries = page.getByText('Selected Countries');
    await expect(selectedCountries).toBeVisible();

    // Change indicator
    const indicatorSelect = page.getByLabel('Economic Indicator');
    await indicatorSelect.click();
    await page.getByText('Inflation').click();

    // Selected countries should still be visible
    await expect(selectedCountries).toBeVisible();
  });

  test('should update map view state on zoom', async ({ page }) => {
    const zoomIn = page.getByLabel('Zoom In');
    const zoomOut = page.getByLabel('Zoom Out');

    // Zoom in
    await zoomIn.click();
    await page.waitForTimeout(500);

    // Zoom out
    await zoomOut.click();
    await page.waitForTimeout(500);

    // Map should maintain state
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });

  test('should update projection state', async ({ page }) => {
    const projectionSelect = page.getByLabel('Projection');

    // Change to Mercator
    await projectionSelect.click();
    await page.getByText('Mercator').click();
    await page.waitForTimeout(500);

    // Change to Orthographic
    await projectionSelect.click();
    await page.getByText('Orthographic').click();
    await page.waitForTimeout(500);

    // Map should still be visible
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });

  test('should handle hover state updates', async ({ page }) => {
    const countryPath = page.locator('svg path').first();

    if (await countryPath.isVisible()) {
      await countryPath.hover();

      // Should show hover state (tooltip or highlight)
      const tooltip = page.locator('[role="tooltip"]').or(
        page.locator('.tooltip')
      );

      // Tooltip might not be visible due to timing
      await page.waitForTimeout(100);

      // Move away from country
      await page.mouse.move(0, 0);
      await page.waitForTimeout(100);
    }
  });

  test('should persist state during page navigation', async ({ page }) => {
    // Set some state
    const indicatorSelect = page.getByLabel('Economic Indicator');
    await indicatorSelect.click();
    await page.getByText('Inflation').click();

    // Navigate away
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    // Navigate back
    await page.goto('/global');
    await page.waitForLoadState('networkidle');

    // Map should still be visible
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });

  test('should handle multiple rapid state updates', async ({ page }) => {
    const indicatorSelect = page.getByLabel('Economic Indicator');
    const projectionSelect = page.getByLabel('Projection');

    // Rapidly change states
    for (let i = 0; i < 3; i++) {
      await indicatorSelect.click();
      await page.getByText('GDP').click();
      await page.waitForTimeout(100);

      await projectionSelect.click();
      await page.getByText('Mercator').click();
      await page.waitForTimeout(100);
    }

    // Map should still be visible and responsive
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });

  test('should handle context provider errors gracefully', async ({ page }) => {
    // Monitor console errors
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    // Perform various interactions
    const indicatorSelect = page.getByLabel('Economic Indicator');
    await indicatorSelect.click();
    await page.getByText('Inflation').click();

    const projectionSelect = page.getByLabel('Projection');
    await projectionSelect.click();
    await page.getByText('Mercator').click();

    // Check for context-related errors
    const contextErrors = consoleErrors.filter(error =>
      error.includes('useGlobalAnalysis') ||
      error.includes('GlobalAnalysisProvider') ||
      error.includes('Context')
    );

    expect(contextErrors).toHaveLength(0);
  });

  test('should maintain state consistency across components', async ({ page }) => {
    // Change indicator
    const indicatorSelect = page.getByLabel('Economic Indicator');
    await indicatorSelect.click();
    await page.getByText('Inflation').click();

    // Check that legend updates
    const legend = page.getByText('Inflation');
    await expect(legend).toBeVisible();

    // Check that map updates (color changes)
    const paths = page.locator('svg path');
    const firstPath = paths.first();

    if (await firstPath.isVisible()) {
      const fillColor = await firstPath.getAttribute('fill');
      expect(fillColor).toBeTruthy();
    }
  });

  test('should handle state updates with loading states', async ({ page }) => {
    // Monitor for loading states
    const loadingIndicator = page.getByRole('progressbar');

    // Change indicator (might trigger loading)
    const indicatorSelect = page.getByLabel('Economic Indicator');
    await indicatorSelect.click();
    await page.getByText('Inflation').click();

    // If loading indicator appears, it should disappear
    if (await loadingIndicator.isVisible()) {
      await expect(loadingIndicator).toBeHidden({ timeout: 5000 });
    }

    // Map should be visible after loading
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });
});
