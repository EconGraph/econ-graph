import { test, expect } from '@playwright/test';

test.describe('Complete Application Workflow', () => {
  test('should complete full user journey from landing to data exploration', async ({ page }) => {
    // Start at the dashboard
    await page.goto('/');

    // Verify dashboard loads
    await expect(page.locator('main')).toBeVisible();
    await expect(page.getByText(/econograph/i)).toBeVisible();

    // Navigate to series explorer
    await page.getByRole('button', { name: /menu/i }).click();
    await page.getByRole('link', { name: /explore/i }).click();
    await expect(page).toHaveURL('/explore');

    // Verify series explorer loads
    await expect(page.locator('main')).toBeVisible();

    // Try to search for data
    const searchInput = page.getByRole('textbox', { name: /search/i }).or(
      page.getByPlaceholder(/search/i)
    );

    if (await searchInput.isVisible()) {
      await searchInput.fill('GDP');
      await searchInput.press('Enter');
      await page.waitForTimeout(2000);
    }

    // Navigate to data sources
    await page.getByRole('button', { name: /menu/i }).click();
    await page.getByRole('link', { name: /sources/i }).click();
    await expect(page).toHaveURL('/sources');

    // Verify data sources page loads
    await expect(page.locator('main')).toBeVisible();

    // Navigate to global analysis
    await page.getByRole('button', { name: /menu/i }).click();
    await page.getByRole('link', { name: /global/i }).click();
    await expect(page).toHaveURL('/global');

    // Verify global analysis loads
    await expect(page.locator('main')).toBeVisible();

    // Navigate to professional analysis
    await page.getByRole('button', { name: /menu/i }).click();
    await page.getByRole('link', { name: /analysis/i }).click();
    await expect(page).toHaveURL('/analysis');

    // Verify professional analysis loads
    await expect(page.locator('main')).toBeVisible();

    // Navigate to about page
    await page.getByRole('button', { name: /menu/i }).click();
    await page.getByRole('link', { name: /about/i }).click();
    await expect(page).toHaveURL('/about');

    // Verify about page loads
    await expect(page.locator('main')).toBeVisible();

    // Return to dashboard
    await page.getByRole('button', { name: /menu/i }).click();
    await page.getByRole('link', { name: /dashboard/i }).click();
    await expect(page).toHaveURL('/');
  });

  test('should handle authentication flow', async ({ page }) => {
    await page.goto('/');

    // Look for login button
    const loginButton = page.getByRole('button', { name: /login|sign in/i }).or(
      page.getByRole('link', { name: /login|sign in/i })
    );

    if (await loginButton.isVisible()) {
      await loginButton.click();

      // Verify login dialog/form appears
      const loginDialog = page.locator('[role="dialog"]').or(
        page.locator('form').or(
          page.locator('[data-testid="login-dialog"]')
        )
      );

      await expect(loginDialog).toBeVisible();

      // Try to fill login form
      const emailInput = page.getByLabel(/email/i);
      const passwordInput = page.getByLabel(/password/i);

      if (await emailInput.isVisible() && await passwordInput.isVisible()) {
        await emailInput.fill('test@example.com');
        await passwordInput.fill('testpassword');

        // Try to submit (this will likely fail, but should not crash)
        const submitButton = page.getByRole('button', { name: /login|sign in|submit/i });
        if (await submitButton.isVisible()) {
          await submitButton.click();

          // Should either show error or redirect
          await page.waitForTimeout(2000);
        }
      }
    }
  });

  test('should maintain state across page navigation', async ({ page }) => {
    await page.goto('/');

    // Open sidebar
    await page.getByRole('button', { name: /menu/i }).click();
    const sidebar = page.locator('[data-testid="sidebar"]').or(page.locator('nav'));
    await expect(sidebar).toBeVisible();

    // Navigate to different pages
    await page.getByRole('link', { name: /explore/i }).click();
    await expect(page).toHaveURL('/explore');

    // Sidebar should still be open
    await expect(sidebar).toBeVisible();

    // Navigate to another page
    await page.getByRole('link', { name: /sources/i }).click();
    await expect(page).toHaveURL('/sources');

    // Sidebar should still be open
    await expect(sidebar).toBeVisible();
  });

  test('should handle responsive design across all pages', async ({ page }) => {
    const pages = ['/', '/explore', '/sources', '/global', '/analysis', '/about'];

    for (const pagePath of pages) {
      await page.goto(pagePath);

      // Test desktop view
      await page.setViewportSize({ width: 1200, height: 800 });
      await expect(page.locator('main')).toBeVisible();

      // Test tablet view
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(page.locator('main')).toBeVisible();

      // Test mobile view
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(page.locator('main')).toBeVisible();

      // Menu button should be visible on mobile
      await expect(page.getByRole('button', { name: /menu/i })).toBeVisible();
    }
  });

  test('should not have JavaScript console errors during navigation', async ({ page }) => {
    const consoleErrors: string[] = [];

    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    const pages = ['/', '/explore', '/sources', '/global', '/analysis', '/about'];

    for (const pagePath of pages) {
      await page.goto(pagePath);
      await page.waitForLoadState('networkidle');

      // Navigate between pages
      await page.getByRole('button', { name: /menu/i }).click();
      await page.waitForTimeout(500);
    }

    // Filter out expected errors
    const unexpectedErrors = consoleErrors.filter(error =>
      !error.includes('OAuth') &&
      !error.includes('authentication') &&
      !error.includes('403') &&
      !error.includes('401') &&
      !error.includes('NetworkError') &&
      !error.includes('Failed to fetch')
    );

    expect(unexpectedErrors).toHaveLength(0);
  });

  test('should handle network errors gracefully', async ({ page }) => {
    // Intercept all network requests to simulate network issues
    await page.route('**/*', route => {
      if (route.request().url().includes('graphql')) {
        route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'Network error' })
        });
      } else {
        route.continue();
      }
    });

    await page.goto('/');
    await page.waitForLoadState('networkidle');

    // Application should still be functional
    await expect(page.locator('main')).toBeVisible();

    // Navigation should still work
    await page.getByRole('button', { name: /menu/i }).click();
    await page.getByRole('link', { name: /explore/i }).click();
    await expect(page).toHaveURL('/explore');

    // Should show error messages or fallback content
    const hasErrorHandling = await page.locator('[role="alert"]').isVisible() ||
      await page.getByText(/error/i).isVisible() ||
      await page.getByText(/unable to load/i).isVisible() ||
      await page.getByText(/no data/i).isVisible();

    expect(hasErrorHandling).toBeTruthy();
  });
});
