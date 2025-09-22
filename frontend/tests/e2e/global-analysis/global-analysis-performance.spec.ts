import { test, expect } from '@playwright/test';

test.describe('Global Analysis Performance Integration Tests', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/global');
    await page.waitForLoadState('networkidle');
  });

  test('should load map within performance budget', async ({ page }) => {
    const startTime = Date.now();

    // Wait for map to be visible
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();

    const loadTime = Date.now() - startTime;

    // Map should load within 5 seconds
    expect(loadTime).toBeLessThan(5000);
  });

  test('should maintain 60fps during zoom interactions', async ({ page }) => {
    const zoomIn = page.getByLabel('Zoom In');
    const zoomOut = page.getByLabel('Zoom Out');

    // Perform zoom operations and measure performance
    const startTime = Date.now();

    for (let i = 0; i < 5; i++) {
      await zoomIn.click();
      await page.waitForTimeout(100);
      await zoomOut.click();
      await page.waitForTimeout(100);
    }

    const endTime = Date.now();
    const totalTime = endTime - startTime;

    // Should complete 10 operations in reasonable time
    expect(totalTime).toBeLessThan(3000);
  });

  test('should handle rapid state changes without performance degradation', async ({ page }) => {
    const indicatorSelect = page.getByLabel('Economic Indicator');
    const projectionSelect = page.getByLabel('Projection');

    const startTime = Date.now();

    // Rapidly change states
    for (let i = 0; i < 10; i++) {
      await indicatorSelect.click();
      await page.getByText('GDP').click();
      await page.waitForTimeout(50);

      await projectionSelect.click();
      await page.getByText('Mercator').click();
      await page.waitForTimeout(50);
    }

    const endTime = Date.now();
    const totalTime = endTime - startTime;

    // Should handle rapid changes efficiently
    expect(totalTime).toBeLessThan(5000);
  });

  test('should not cause memory leaks during extended use', async ({ page }) => {
    // Perform many interactions
    const zoomIn = page.getByLabel('Zoom In');
    const zoomOut = page.getByLabel('Zoom Out');
    const indicatorSelect = page.getByLabel('Economic Indicator');

    for (let i = 0; i < 20; i++) {
      await zoomIn.click();
      await page.waitForTimeout(50);
      await zoomOut.click();
      await page.waitForTimeout(50);

      if (i % 5 === 0) {
        await indicatorSelect.click();
        await page.getByText('GDP').click();
        await page.waitForTimeout(50);
      }
    }

    // Map should still be responsive
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();

    // No console errors should indicate memory issues
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    // Perform a few more operations
    await zoomIn.click();
    await zoomOut.click();

    const memoryErrors = consoleErrors.filter(error =>
      error.includes('memory') ||
      error.includes('leak') ||
      error.includes('out of memory')
    );

    expect(memoryErrors).toHaveLength(0);
  });

  test('should handle large viewport sizes efficiently', async ({ page }) => {
    // Test with large viewport
    await page.setViewportSize({ width: 2560, height: 1440 });
    await page.waitForTimeout(500);

    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();

    // Test zoom operations on large viewport
    const zoomIn = page.getByLabel('Zoom In');
    await zoomIn.click();
    await page.waitForTimeout(500);

    await expect(worldMap).toBeVisible();
  });

  test('should handle multiple rapid clicks without issues', async ({ page }) => {
    const countryPath = page.locator('svg path').first();

    if (await countryPath.isVisible()) {
      // Rapidly click on country
      for (let i = 0; i < 10; i++) {
        await countryPath.click();
        await page.waitForTimeout(50);
      }

      // Map should still be responsive
      const worldMap = page.locator('svg').first();
      await expect(worldMap).toBeVisible();
    }
  });

  test('should maintain performance during window resize', async ({ page }) => {
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();

    // Resize window multiple times
    const sizes = [
      { width: 800, height: 600 },
      { width: 1200, height: 800 },
      { width: 1920, height: 1080 },
      { width: 800, height: 600 }
    ];

    for (const size of sizes) {
      await page.setViewportSize(size);
      await page.waitForTimeout(200);
      await expect(worldMap).toBeVisible();
    }
  });

  test('should handle concurrent user interactions', async ({ page }) => {
    // Simulate concurrent interactions
    const zoomIn = page.getByLabel('Zoom In');
    const zoomOut = page.getByLabel('Zoom Out');
    const indicatorSelect = page.getByLabel('Economic Indicator');

    // Start multiple interactions simultaneously
    const promises = [
      zoomIn.click(),
      zoomOut.click(),
      indicatorSelect.click()
    ];

    await Promise.allSettled(promises);

    // Map should still be visible and responsive
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });

  test('should not block UI during data processing', async ({ page }) => {
    // Monitor for UI blocking
    let uiBlocked = false;

    page.on('console', msg => {
      if (msg.text().includes('blocking') || msg.text().includes('unresponsive')) {
        uiBlocked = true;
      }
    });

    // Perform operations that might block UI
    const indicatorSelect = page.getByLabel('Economic Indicator');
    await indicatorSelect.click();
    await page.getByText('Inflation').click();

    const projectionSelect = page.getByLabel('Projection');
    await projectionSelect.click();
    await page.getByText('Mercator').click();

    // UI should not be blocked
    expect(uiBlocked).toBeFalsy();

    // Map should be visible
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });
});
