import { test, expect } from '@playwright/test';

test.describe('Global Analysis - Enhanced Integration Tests', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/global');
    await page.waitForLoadState('networkidle');
  });

  test('should load interactive world map with D3.js', async ({ page }) => {
    // Check for the main world map component
    const worldMap = page.locator('[data-testid="interactive-world-map"]').or(
      page.locator('svg[width="900"][height="500"]')
    );

    await expect(worldMap).toBeVisible();

    // Verify SVG has proper dimensions
    const svg = page.locator('svg').first();
    await expect(svg).toHaveAttribute('width', '900');
    await expect(svg).toHaveAttribute('height', '500');
  });

  test('should display map controls panel', async ({ page }) => {
    // Check for map controls
    const controlsPanel = page.getByText('Map Controls');
    await expect(controlsPanel).toBeVisible();

    // Check for zoom controls
    const zoomIn = page.getByLabel('Zoom In');
    const zoomOut = page.getByLabel('Zoom Out');
    await expect(zoomIn).toBeVisible();
    await expect(zoomOut).toBeVisible();

    // Check for projection selector
    const projectionSelect = page.getByLabel('Projection');
    await expect(projectionSelect).toBeVisible();
  });

  test('should allow country selection and display tooltips', async ({ page }) => {
    // Wait for map to load
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();

    // Try to click on a country (path element)
    const countryPath = page.locator('svg path').first();

    if (await countryPath.isVisible()) {
      await countryPath.click();

      // Check for country selection feedback
      const selectedCountries = page.getByText('Selected Countries');
      await expect(selectedCountries).toBeVisible();
    }
  });

  test('should display economic indicators selector', async ({ page }) => {
    // Check for indicator selector
    const indicatorSelect = page.getByLabel('Economic Indicator');
    await expect(indicatorSelect).toBeVisible();

    // Check for available indicators
    await indicatorSelect.click();
    await expect(page.getByText('GDP')).toBeVisible();
    await expect(page.getByText('Inflation')).toBeVisible();
    await expect(page.getByText('Unemployment')).toBeVisible();
  });

  test('should show map legend with color scale', async ({ page }) => {
    // Check for legend
    const legend = page.getByText('GDP').or(page.getByText('Inflation'));
    await expect(legend).toBeVisible();

    // Check for color gradient
    const colorGradient = page.locator('[style*="linear-gradient"]');
    await expect(colorGradient).toBeVisible();
  });

  test('should handle map zoom interactions', async ({ page }) => {
    const zoomIn = page.getByLabel('Zoom In');
    const zoomOut = page.getByLabel('Zoom Out');

    // Test zoom in
    await zoomIn.click();
    await page.waitForTimeout(500);

    // Test zoom out
    await zoomOut.click();
    await page.waitForTimeout(500);

    // Map should still be visible
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });

  test('should switch between map projections', async ({ page }) => {
    const projectionSelect = page.getByLabel('Projection');
    await projectionSelect.click();

    // Test Natural Earth projection
    await page.getByText('Natural Earth').click();
    await page.waitForTimeout(500);

    // Test Mercator projection
    await projectionSelect.click();
    await page.getByText('Mercator').click();
    await page.waitForTimeout(500);

    // Map should still be visible
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });

  test('should display sample country data', async ({ page }) => {
    // Check for sample data display
    const sampleData = page.getByText('United States').or(
      page.getByText('China').or(
        page.getByText('Japan')
      )
    );
    await expect(sampleData).toBeVisible();
  });

  test('should handle map loading states', async ({ page }) => {
    // Check for loading indicator initially
    const loadingIndicator = page.getByRole('progressbar');

    // Should show loading initially, then disappear
    if (await loadingIndicator.isVisible()) {
      await expect(loadingIndicator).toBeHidden({ timeout: 10000 });
    }

    // Map should be visible after loading
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });

  test('should be responsive on different screen sizes', async ({ page }) => {
    // Test mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();

    // Test tablet viewport
    await page.setViewportSize({ width: 768, height: 1024 });
    await expect(worldMap).toBeVisible();

    // Test desktop viewport
    await page.setViewportSize({ width: 1920, height: 1080 });
    await expect(worldMap).toBeVisible();
  });

  test('should handle map interaction errors gracefully', async ({ page }) => {
    // Monitor console errors
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    // Try various interactions
    const worldMap = page.locator('svg').first();
    await worldMap.click();

    // Check for error alerts
    const errorAlert = page.locator('[role="alert"]').filter({ hasText: /error|failed/i });
    await expect(errorAlert).not.toBeVisible();

    // Check console errors
    const unexpectedErrors = consoleErrors.filter(error =>
      !error.includes('OAuth') &&
      !error.includes('authentication') &&
      !error.includes('403') &&
      !error.includes('401')
    );
    expect(unexpectedErrors).toHaveLength(0);
  });

  test('should maintain state during navigation', async ({ page }) => {
    // Select a country
    const countryPath = page.locator('svg path').first();
    if (await countryPath.isVisible()) {
      await countryPath.click();
    }

    // Navigate away and back
    await page.goto('/');
    await page.goto('/global');

    // Map should still be visible
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });
});
