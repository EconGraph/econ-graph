import { test, expect } from '@playwright/test';

test.describe('Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('should display header with title and menu button', async ({ page }) => {
    await expect(page.locator('header')).toBeVisible();
    await expect(page.locator('header').getByText('EconGraph')).toBeVisible();

    // Menu button is only visible on mobile screens
    await page.setViewportSize({ width: 375, height: 667 });
    await expect(page.getByRole('button', { name: /open drawer/i })).toBeVisible();
  });

  test('should toggle sidebar when menu button is clicked', async ({ page }) => {
    // Set mobile viewport to show menu button
    await page.setViewportSize({ width: 375, height: 667 });

    const sidebar = page.locator('[role="navigation"][aria-label="Main navigation"]');

    // Sidebar should be closed initially
    await expect(sidebar).not.toBeVisible();

    // Click menu button to open sidebar
    await page.getByRole('button', { name: /open drawer/i }).click();
    await expect(sidebar).toBeVisible();

    // Click on backdrop to close sidebar
    await page.locator('.MuiBackdrop-root').click();
    await expect(sidebar).not.toBeVisible();
  });

  test('should navigate to all main pages', async ({ page }) => {
    // Set mobile viewport to show menu button
    await page.setViewportSize({ width: 375, height: 667 });

    // Test navigation to each page
    const navigationTests = [
      { link: 'Dashboard', path: '/' },
      { link: 'Explore Series', path: '/explore' },
      { link: 'Data Sources', path: '/sources' },
      { link: 'About', path: '/about' },
      { link: 'Global Analysis', path: '/global' },
    ];

    for (const { link, path } of navigationTests) {
      // Open sidebar for each navigation
      await page.getByRole('button', { name: /open drawer/i }).click();

      // Click on the navigation item
      await page.getByRole('button', { name: link }).click();
      await expect(page).toHaveURL(path);

      // Verify page content is loaded
      await expect(page.locator('main')).toBeVisible();
    }
  });

  test('should be responsive on mobile devices', async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });

    // Header should still be visible
    await expect(page.locator('header')).toBeVisible();

    // Menu button should be visible
    await expect(page.getByRole('button', { name: /open drawer/i })).toBeVisible();

    // Sidebar should be hidden by default on mobile
    const sidebar = page.locator('[role="navigation"][aria-label="Main navigation"]');
    await expect(sidebar).not.toBeVisible();
  });

  test('should close sidebar when clicking outside on mobile', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });

    // Open sidebar
    await page.getByRole('button', { name: /open drawer/i }).click();
    const sidebar = page.locator('[role="navigation"][aria-label="Main navigation"]');
    await expect(sidebar).toBeVisible();

    // Click on the backdrop to close sidebar
    await page.locator('.MuiBackdrop-root').click();

    // Sidebar should close
    await expect(sidebar).not.toBeVisible();
  });
});
