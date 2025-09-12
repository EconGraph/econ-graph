import { test, expect } from '@playwright/test';

test.describe('Dev Server', () => {
  test('should serve the application on the correct port', async ({ page }) => {
    await page.goto('/');

    // Wait for the page to load
    await page.waitForLoadState('networkidle');

    // Check that we're on the correct port
    const url = page.url();
    expect(url).toContain('localhost:18473');

    // Check that the page loads successfully
    await expect(page).toHaveTitle(/EconGraph/);

    // Check that the main content is visible
    const main = page.locator('main');
    await expect(main).toBeVisible();
  });

  test('should have the correct base URL configuration', async ({ page }) => {
    // This test verifies that the baseURL is correctly configured
    await page.goto('/');

    const url = page.url();
    expect(url).toBe('http://localhost:18473/');
  });
});
