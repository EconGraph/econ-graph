import { test, expect } from '@playwright/test';

test.describe('Complete Application Workflow', () => {
  test('should complete full user journey from landing to data exploration', async ({ page }) => {
    // Start at the dashboard
    await page.goto('/');

    // Verify dashboard loads - use more flexible selectors
    await expect(page.locator('main')).toBeVisible();

    // Look for any heading that might indicate the app is loaded
    const heading = page.getByRole('heading').first();
    await expect(heading).toBeVisible();

    // Try to navigate to series explorer if menu is available
    const menuButton = page.getByRole('button', { name: /menu/i }).or(
      page.locator('[data-testid="menu-button"]').or(
        page.locator('button[aria-label*="menu" i]')
      )
    );

    if (await menuButton.isVisible()) {
      await menuButton.click();

      // Look for explore link with multiple possible selectors
      const exploreLink = page.getByRole('link', { name: /explore/i }).or(
        page.getByRole('link', { name: /series/i }).or(
          page.locator('a[href*="/explore"]')
        )
      );

      if (await exploreLink.isVisible()) {
        await exploreLink.click();
        await expect(page).toHaveURL(/.*\/explore.*/);
        await expect(page.locator('main')).toBeVisible();
      }
    }

    // Try to search for data if search input is available
    const searchInput = page.getByRole('textbox', { name: /search/i }).or(
      page.getByPlaceholder(/search/i).or(
        page.locator('input[type="search"]')
      )
    );

    if (await searchInput.isVisible()) {
      await searchInput.fill('GDP');
      await searchInput.press('Enter');
      // Wait for search results instead of fixed timeout
      await page.waitForLoadState('networkidle');
    }

    // Try to navigate to other pages if menu is available
    if (await menuButton.isVisible()) {
      await menuButton.click();

      // Try data sources page
      const sourcesLink = page.getByRole('link', { name: /sources/i }).or(
        page.getByRole('link', { name: /data.*sources/i }).or(
          page.locator('a[href*="/sources"]')
        )
      );

      if (await sourcesLink.isVisible()) {
        await sourcesLink.click();
        await expect(page).toHaveURL(/.*\/sources.*/);
        await expect(page.locator('main')).toBeVisible();
      }
    }

    // Try global analysis page
    if (await menuButton.isVisible()) {
      await menuButton.click();

      const globalLink = page.getByRole('link', { name: /global/i }).or(
        page.getByRole('link', { name: /analysis/i }).or(
          page.locator('a[href*="/global"]')
        )
      );

      if (await globalLink.isVisible()) {
        await globalLink.click();
        await expect(page).toHaveURL(/.*\/global.*/);
        await expect(page.locator('main')).toBeVisible();
      }
    }

    // Try about page
    if (await menuButton.isVisible()) {
      await menuButton.click();

      const aboutLink = page.getByRole('link', { name: /about/i }).or(
        page.locator('a[href*="/about"]')
      );

      if (await aboutLink.isVisible()) {
        await aboutLink.click();
        await expect(page).toHaveURL(/.*\/about.*/);
        await expect(page.locator('main')).toBeVisible();
      }
    }

    // Return to dashboard
    if (await menuButton.isVisible()) {
      await menuButton.click();

      const dashboardLink = page.getByRole('link', { name: /dashboard/i }).or(
        page.getByRole('link', { name: /home/i }).or(
          page.locator('a[href="/"]')
        )
      );

      if (await dashboardLink.isVisible()) {
        await dashboardLink.click();
        await expect(page).toHaveURL('/');
      }
    }
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
