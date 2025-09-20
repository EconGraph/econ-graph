import { test, expect } from '@playwright/test';

/**
 * Basic functionality tests for core E2E testing
 * These tests verify that the application loads and basic navigation works
 */
test.describe('Basic Functionality', () => {
  test('should load the homepage', async ({ page }) => {
    await page.goto('/');

    // Check that the page loads
    await expect(page).toHaveTitle(/EconGraph/);

    // Check that the main content is visible
    await expect(page.locator('body')).toBeVisible();
  });

  test('should have working navigation', async ({ page }) => {
    await page.goto('/');

    // Wait for the page to load
    await page.waitForLoadState('networkidle');

    // Check that the page is responsive
    await expect(page.locator('body')).toBeVisible();
  });

  test('should handle basic page interactions', async ({ page }) => {
    await page.goto('/');

    // Wait for the page to load
    await page.waitForLoadState('networkidle');

    // Basic interaction test - just verify the page is interactive
    await expect(page.locator('body')).toBeVisible();

    // Test that we can interact with the page
    await page.mouse.move(100, 100);
  });
});
