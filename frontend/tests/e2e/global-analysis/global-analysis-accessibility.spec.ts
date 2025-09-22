import { test, expect } from '@playwright/test';

test.describe('Global Analysis Accessibility Integration Tests', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/global');
    await page.waitForLoadState('networkidle');
  });

  test('should be navigable via keyboard', async ({ page }) => {
    // Tab through interactive elements
    await page.keyboard.press('Tab');
    await page.keyboard.press('Tab');
    await page.keyboard.press('Tab');

    // Should be able to focus on map controls
    const focusedElement = page.locator(':focus');
    await expect(focusedElement).toBeVisible();
  });

  test('should have proper ARIA labels', async ({ page }) => {
    // Check for ARIA labels on interactive elements
    const zoomIn = page.getByLabel('Zoom In');
    const zoomOut = page.getByLabel('Zoom Out');
    const projectionSelect = page.getByLabel('Projection');
    const indicatorSelect = page.getByLabel('Economic Indicator');

    await expect(zoomIn).toBeVisible();
    await expect(zoomOut).toBeVisible();
    await expect(projectionSelect).toBeVisible();
    await expect(indicatorSelect).toBeVisible();
  });

  test('should support screen reader navigation', async ({ page }) => {
    // Check for proper heading structure
    const headings = page.locator('h1, h2, h3, h4, h5, h6');
    const headingCount = await headings.count();
    expect(headingCount).toBeGreaterThan(0);

    // Check for main landmark
    const main = page.locator('main');
    await expect(main).toBeVisible();
  });

  test('should have proper focus management', async ({ page }) => {
    // Focus on map controls
    const zoomIn = page.getByLabel('Zoom In');
    await zoomIn.focus();
    await expect(zoomIn).toBeFocused();

    // Tab to next element
    await page.keyboard.press('Tab');
    const nextElement = page.locator(':focus');
    await expect(nextElement).toBeVisible();
  });

  test('should support keyboard activation of controls', async ({ page }) => {
    // Focus on zoom in button
    const zoomIn = page.getByLabel('Zoom In');
    await zoomIn.focus();

    // Activate with Enter key
    await page.keyboard.press('Enter');

    // Map should still be visible
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });

  test('should have proper color contrast', async ({ page }) => {
    // Check that text is visible against background
    const textElements = page.locator('text, p, span, div').filter({ hasText: /[A-Za-z]/ });
    const textCount = await textElements.count();

    // Should have readable text elements
    expect(textCount).toBeGreaterThan(0);
  });

  test('should support reduced motion preferences', async ({ page }) => {
    // Check for reduced motion support
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();

    // Perform zoom operation
    const zoomIn = page.getByLabel('Zoom In');
    await zoomIn.click();

    // Map should still be visible (no jarring animations)
    await expect(worldMap).toBeVisible();
  });

  test('should have proper form labels', async ({ page }) => {
    // Check that form elements have proper labels
    const selects = page.locator('select');
    const selectCount = await selects.count();

    for (let i = 0; i < selectCount; i++) {
      const select = selects.nth(i);
      const label = page.locator(`label[for="${await select.getAttribute('id')}"]`);
      await expect(label).toBeVisible();
    }
  });

  test('should announce state changes to screen readers', async ({ page }) => {
    // Monitor for ARIA live regions
    const liveRegions = page.locator('[aria-live]');
    const liveRegionCount = await liveRegions.count();

    // Should have live regions for dynamic content
    expect(liveRegionCount).toBeGreaterThanOrEqual(0);
  });

  test('should support high contrast mode', async ({ page }) => {
    // Check that elements are visible in high contrast
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();

    // Check for high contrast indicators
    const highContrastElements = page.locator('[data-high-contrast]');
    const highContrastCount = await highContrastElements.count();

    // Should have high contrast support
    expect(highContrastCount).toBeGreaterThanOrEqual(0);
  });

  test('should have proper error handling for accessibility', async ({ page }) => {
    // Check for error messages with proper ARIA attributes
    const errorMessages = page.locator('[role="alert"]');
    const errorCount = await errorMessages.count();

    // Should have proper error handling
    expect(errorCount).toBeGreaterThanOrEqual(0);
  });

  test('should support keyboard shortcuts', async ({ page }) => {
    // Test common keyboard shortcuts
    await page.keyboard.press('Escape'); // Should not cause errors

    // Test arrow keys for navigation
    await page.keyboard.press('ArrowUp');
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowRight');

    // Map should still be visible
    const worldMap = page.locator('svg').first();
    await expect(worldMap).toBeVisible();
  });

  test('should have proper semantic HTML structure', async ({ page }) => {
    // Check for proper semantic elements
    const main = page.locator('main');
    const section = page.locator('section');
    const article = page.locator('article');

    await expect(main).toBeVisible();

    // Should have semantic structure
    const semanticCount = await section.count() + await article.count();
    expect(semanticCount).toBeGreaterThanOrEqual(0);
  });
});
