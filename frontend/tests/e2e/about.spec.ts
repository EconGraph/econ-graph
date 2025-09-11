import { test, expect } from '@playwright/test';

test.describe('About Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/about');
  });

  test('should display about page', async ({ page }) => {
    await expect(page.locator('main')).toBeVisible();

    // Check for about page specific content - use specific heading
    const aboutContent = page.getByRole('heading', { name: 'About EconGraph' });

    await expect(aboutContent).toBeVisible();
  });

  test('should display application description', async ({ page }) => {
    // Look for application description - use specific heading to avoid strict mode violation
    const description = page.getByRole('heading', { name: 'Modern Economic Data Visualization Platform' });

    await expect(description).toBeVisible();
  });

  test('should display features list', async ({ page }) => {
    // Look for features section
    const featuresSection = page.getByText(/features/i).or(
      page.getByText(/capabilities/i).or(
        page.locator('[data-testid="features"]')
      )
    );

    await expect(featuresSection.first()).toBeVisible();
  });

  test('should display technology stack information', async ({ page }) => {
    // Look for technology information
    const techInfo = page.getByRole('heading', { name: 'Technology Stack' });

    await expect(techInfo).toBeVisible();
  });

  test('should display contact information or links', async ({ page }) => {
    // Look for contact information
    const contactInfo = page.getByText(/contact/i).or(
      page.getByText(/github/i).or(
        page.getByText(/support/i).or(
          page.locator('a[href*="github"]').or(
            page.locator('a[href*="mailto"]')
          )
        )
      )
    );

    await expect(contactInfo.first()).toBeVisible();
  });

  test('should display version information', async ({ page }) => {
    // Look for version information
    const versionInfo = page.getByText('Version 3.7.2');

    await expect(versionInfo).toBeVisible();
  });

  test('should have navigation back to main sections', async ({ page }) => {
    // Look for navigation links
    const navLinks = page.getByRole('link', { name: 'Dashboard' }).or(
      page.getByRole('link', { name: 'Explore Series' }).or(
        page.getByRole('link', { name: 'Data Sources' })
      )
    );

    await expect(navLinks.first()).toBeVisible();
  });

  test('should be responsive on mobile devices', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });

    await expect(page.locator('main')).toBeVisible();

    // Content should still be readable on mobile - use specific heading
    const aboutContent = page.getByRole('heading', { name: 'About EconGraph' });

    await expect(aboutContent).toBeVisible();
  });

  test('should load without JavaScript console errors', async ({ page }) => {
    const consoleErrors: string[] = [];

    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    await page.reload();
    await page.waitForLoadState('networkidle');

    // Filter out expected errors
    const unexpectedErrors = consoleErrors.filter(error =>
      !error.includes('OAuth') &&
      !error.includes('authentication') &&
      !error.includes('403') &&
      !error.includes('401')
    );

    expect(unexpectedErrors).toHaveLength(0);
  });

  test('should have proper heading structure for accessibility', async ({ page }) => {
    // Check for proper heading hierarchy
    const h1 = page.locator('h1');
    const h2 = page.locator('h2');

    await expect(h1.first()).toBeVisible();

    // Should have at least one h2 if there are sections
    const h2Count = await h2.count();
    if (h2Count > 0) {
      await expect(h2.first()).toBeVisible();
    }
  });

  test('should have working external links', async ({ page }) => {
    // Check for external links (like GitHub)
    const externalLinks = page.locator('a[href*="github"]').or(
      page.locator('a[href*="http"]')
    );

    if (await externalLinks.count() > 0) {
      const firstLink = externalLinks.first();
      const href = await firstLink.getAttribute('href');

      // Should have valid href
      expect(href).toBeTruthy();
      expect(href).toMatch(/^https?:\/\//);
    }
  });
});
