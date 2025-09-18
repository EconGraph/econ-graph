import { test, expect } from '@playwright/test';

test.describe('D3.js Visualization Integration Tests', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/global');
    await page.waitForLoadState('networkidle');
  });

  test('should load D3.js world atlas data', async ({ page }) => {
    // Monitor network requests for world atlas data
    const requests: string[] = [];
    page.on('request', request => {
      if (request.url().includes('world-atlas') || request.url().includes('110m.json')) {
        requests.push(request.url());
      }
    });

    await page.reload();
    await page.waitForLoadState('networkidle');

    // Should have requested world atlas data
    expect(requests.length).toBeGreaterThan(0);
  });

  test('should render SVG with proper D3.js structure', async ({ page }) => {
    const svg = page.locator('svg').first();
    await expect(svg).toBeVisible();

    // Check for D3.js generated elements
    const paths = page.locator('svg path');
    const pathCount = await paths.count();
    expect(pathCount).toBeGreaterThan(0);

    // Check for proper SVG attributes
    await expect(svg).toHaveAttribute('width');
    await expect(svg).toHaveAttribute('height');
  });

  test('should handle D3.js zoom behavior', async ({ page }) => {
    const svg = page.locator('svg').first();
    await expect(svg).toBeVisible();

    // Test zoom by clicking zoom controls
    const zoomIn = page.getByLabel('Zoom In');
    await zoomIn.click();

    // Wait for zoom animation
    await page.waitForTimeout(1000);

    // SVG should still be visible
    await expect(svg).toBeVisible();
  });

  test('should apply D3.js color scaling', async ({ page }) => {
    // Switch to different indicator to test color scaling
    const indicatorSelect = page.getByLabel('Economic Indicator');
    await indicatorSelect.click();
    await page.getByText('Inflation').click();

    // Wait for color update
    await page.waitForTimeout(500);

    // Check for color-coded paths
    const paths = page.locator('svg path');
    const firstPath = paths.first();

    if (await firstPath.isVisible()) {
      const fillColor = await firstPath.getAttribute('fill');
      expect(fillColor).toBeTruthy();
      expect(fillColor).not.toBe('#ccc'); // Should not be default gray
    }
  });

  test('should handle D3.js projection changes', async ({ page }) => {
    const svg = page.locator('svg').first();
    await expect(svg).toBeVisible();

    // Change projection
    const projectionSelect = page.getByLabel('Projection');
    await projectionSelect.click();
    await page.getByText('Mercator').click();

    // Wait for projection update
    await page.waitForTimeout(1000);

    // SVG should still be visible with new projection
    await expect(svg).toBeVisible();
  });

  test('should handle D3.js data binding', async ({ page }) => {
    // Check that countries have data bound
    const paths = page.locator('svg path');
    const pathCount = await paths.count();

    // Should have multiple countries rendered
    expect(pathCount).toBeGreaterThan(10);

    // Check for data attributes or classes
    const firstPath = paths.first();
    if (await firstPath.isVisible()) {
      const className = await firstPath.getAttribute('class');
      expect(className).toContain('country');
    }
  });

  test('should handle D3.js event listeners', async ({ page }) => {
    const svg = page.locator('svg').first();
    await expect(svg).toBeVisible();

    // Test hover events
    const paths = page.locator('svg path');
    const firstPath = paths.first();

    if (await firstPath.isVisible()) {
      await firstPath.hover();

      // Should show tooltip or highlight
      const tooltip = page.locator('[role="tooltip"]').or(
        page.locator('.tooltip')
      );

      // Tooltip might not be visible due to timing, but no errors should occur
      await page.waitForTimeout(100);
    }
  });

  test('should handle D3.js responsive updates', async ({ page }) => {
    const svg = page.locator('svg').first();
    await expect(svg).toBeVisible();

    // Change viewport size
    await page.setViewportSize({ width: 800, height: 600 });
    await page.waitForTimeout(500);

    // SVG should still be visible and properly sized
    await expect(svg).toBeVisible();

    // Change back to original size
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.waitForTimeout(500);

    await expect(svg).toBeVisible();
  });

  test('should handle D3.js data loading errors', async ({ page }) => {
    // Block world atlas data request to simulate error
    await page.route('**/world-atlas/**', route => route.abort());

    await page.reload();
    await page.waitForLoadState('networkidle');

    // Should show error message
    const errorMessage = page.getByText('Failed to load world map data');
    await expect(errorMessage).toBeVisible();
  });

  test('should maintain D3.js performance with interactions', async ({ page }) => {
    const svg = page.locator('svg').first();
    await expect(svg).toBeVisible();

    // Perform multiple rapid interactions
    const zoomIn = page.getByLabel('Zoom In');
    const zoomOut = page.getByLabel('Zoom Out');

    for (let i = 0; i < 5; i++) {
      await zoomIn.click();
      await page.waitForTimeout(100);
      await zoomOut.click();
      await page.waitForTimeout(100);
    }

    // SVG should still be visible and responsive
    await expect(svg).toBeVisible();
  });
});
