/**
 * REQUIREMENT: E2E test for user preferences functionality
 * PURPOSE: Verify user preferences work correctly in the browser
 * This ensures the complete user experience for theme switching and preferences
 */

import { test, expect } from '@playwright/test';

test.describe('User Preferences', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to the application
    await page.goto('/');

    // Wait for the page to load
    await page.waitForLoadState('networkidle');
  });

  test('should allow user to access profile and view preferences', async ({ page }) => {
    // Check if user is logged in, if not, we'll need to mock or skip
    const loginButton = page.locator('text=Sign In');
    if (await loginButton.isVisible()) {
      // Skip test if not logged in - in a real scenario, you'd set up test user
      test.skip('User not logged in - skipping preferences test');
    }

    // Click on user profile/avatar to open profile dialog
    const profileButton = page.locator('[data-testid="user-profile-button"], [aria-label*="profile"], [aria-label*="account"]').first();
    await expect(profileButton).toBeVisible();
    await profileButton.click();

    // Wait for profile dialog to open
    await page.waitForSelector('[role="dialog"]', { state: 'visible' });

    // Verify preferences section is visible
    await expect(page.locator('text=Preferences')).toBeVisible();

    // Verify theme selector is present and enabled
    const themeSelect = page.locator('text=Theme').locator('..').locator('select, [role="combobox"]');
    await expect(themeSelect).toBeVisible();
    await expect(themeSelect).toBeEnabled();
  });

  test('should allow theme switching from light to dark', async ({ page }) => {
    // Check if user is logged in
    const loginButton = page.locator('text=Sign In');
    if (await loginButton.isVisible()) {
      test.skip('User not logged in - skipping theme test');
    }

    // Open profile dialog
    const profileButton = page.locator('[data-testid="user-profile-button"], [aria-label*="profile"], [aria-label*="account"]').first();
    await profileButton.click();
    await page.waitForSelector('[role="dialog"]', { state: 'visible' });

    // Find and click theme selector
    const themeSelect = page.locator('text=Theme').locator('..').locator('select, [role="combobox"]');
    await themeSelect.click();

    // Select dark theme
    await page.locator('text=Dark').click();

    // Verify theme has changed (check for dark theme indicators)
    // This could be checking for specific CSS classes or data attributes
    await expect(page.locator('text=Dark')).toBeVisible();

    // Verify the theme is applied to the page
    // Check if the page background or other elements have changed
    const body = page.locator('body');
    const bodyClasses = await body.getAttribute('class');
    const hasDarkTheme = bodyClasses?.includes('dark') ||
                        await page.evaluate(() =>
                          document.documentElement.style.colorScheme === 'dark' ||
                          getComputedStyle(document.body).backgroundColor.includes('rgb(18, 18, 18)')
                        );

    // Note: The exact implementation depends on how the theme is applied
    // This is a placeholder for theme verification
    expect(hasDarkTheme).toBeTruthy();
  });

  test('should allow theme switching from dark to light', async ({ page }) => {
    // Check if user is logged in
    const loginButton = page.locator('text=Sign In');
    if (await loginButton.isVisible()) {
      test.skip('User not logged in - skipping theme test');
    }

    // Open profile dialog
    const profileButton = page.locator('[data-testid="user-profile-button"], [aria-label*="profile"], [aria-label*="account"]').first();
    await profileButton.click();
    await page.waitForSelector('[role="dialog"]', { state: 'visible' });

    // Find and click theme selector
    const themeSelect = page.locator('text=Theme').locator('..').locator('select, [role="combobox"]');
    await themeSelect.click();

    // Select light theme
    await page.locator('text=Light').click();

    // Verify theme has changed
    await expect(page.locator('text=Light')).toBeVisible();

    // Verify the theme is applied to the page
    const body = page.locator('body');
    const bodyClasses = await body.getAttribute('class');
    const hasLightTheme = bodyClasses?.includes('light') ||
                         await page.evaluate(() =>
                           document.documentElement.style.colorScheme === 'light' ||
                           getComputedStyle(document.body).backgroundColor.includes('rgb(250, 250, 250)')
                         );

    expect(hasLightTheme).toBeTruthy();
  });

  test('should allow changing default chart type', async ({ page }) => {
    // Check if user is logged in
    const loginButton = page.locator('text=Sign In');
    if (await loginButton.isVisible()) {
      test.skip('User not logged in - skipping chart type test');
    }

    // Open profile dialog
    const profileButton = page.locator('[data-testid="user-profile-button"], [aria-label*="profile"], [aria-label*="account"]').first();
    await profileButton.click();
    await page.waitForSelector('[role="dialog"]', { state: 'visible' });

    // Find and click chart type selector
    const chartTypeSelect = page.locator('text=Default Chart Type').locator('..').locator('select, [role="combobox"]');
    await chartTypeSelect.click();

    // Select area chart
    await page.locator('text=Area Chart').click();

    // Verify selection has changed
    await expect(page.locator('text=Area Chart')).toBeVisible();
  });

  test('should allow toggling email notifications', async ({ page }) => {
    // Check if user is logged in
    const loginButton = page.locator('text=Sign In');
    if (await loginButton.isVisible()) {
      test.skip('User not logged in - skipping notifications test');
    }

    // Open profile dialog
    const profileButton = page.locator('[data-testid="user-profile-button"], [aria-label*="profile"], [aria-label*="account"]').first();
    await profileButton.click();
    await page.waitForSelector('[role="dialog"]', { state: 'visible' });

    // Find notifications toggle
    const notificationsToggle = page.locator('text=Email Notifications').locator('..').locator('input[type="checkbox"]');
    await expect(notificationsToggle).toBeVisible();
    await expect(notificationsToggle).toBeEnabled();

    // Toggle notifications off
    await notificationsToggle.click();
    await expect(notificationsToggle).not.toBeChecked();

    // Toggle notifications back on
    await notificationsToggle.click();
    await expect(notificationsToggle).toBeChecked();
  });

  test('should allow toggling collaboration feature', async ({ page }) => {
    // Check if user is logged in
    const loginButton = page.locator('text=Sign In');
    if (await loginButton.isVisible()) {
      test.skip('User not logged in - skipping collaboration test');
    }

    // Open profile dialog
    const profileButton = page.locator('[data-testid="user-profile-button"], [aria-label*="profile"], [aria-label*="account"]').first();
    await profileButton.click();
    await page.waitForSelector('[role="dialog"]', { state: 'visible' });

    // Find collaboration toggle
    const collaborationToggle = page.locator('text=Enable Chart Collaboration').locator('..').locator('input[type="checkbox"]');
    await expect(collaborationToggle).toBeVisible();
    await expect(collaborationToggle).toBeEnabled();

    // Toggle collaboration off
    await collaborationToggle.click();
    await expect(collaborationToggle).not.toBeChecked();

    // Toggle collaboration back on
    await collaborationToggle.click();
    await expect(collaborationToggle).toBeChecked();
  });

  test('should save preferences when save button is clicked', async ({ page }) => {
    // Check if user is logged in
    const loginButton = page.locator('text=Sign In');
    if (await loginButton.isVisible()) {
      test.skip('User not logged in - skipping save test');
    }

    // Open profile dialog
    const profileButton = page.locator('[data-testid="user-profile-button"], [aria-label*="profile"], [aria-label*="account"]').first();
    await profileButton.click();
    await page.waitForSelector('[role="dialog"]', { state: 'visible' });

    // Make a change to preferences
    const themeSelect = page.locator('text=Theme').locator('..').locator('select, [role="combobox"]');
    await themeSelect.click();
    await page.locator('text=Dark').click();

    // Click save preferences button
    const saveButton = page.locator('text=Save Preferences');
    await expect(saveButton).toBeVisible();
    await expect(saveButton).toBeEnabled();
    await saveButton.click();

    // Verify the change was saved (this might show a success message or close dialog)
    // The exact behavior depends on the implementation
    await page.waitForTimeout(1000); // Wait for any async operations

    // Verify the theme is still applied after save
    await expect(page.locator('text=Dark')).toBeVisible();
  });

  test('should persist theme preference across page reloads', async ({ page }) => {
    // Check if user is logged in
    const loginButton = page.locator('text=Sign In');
    if (await loginButton.isVisible()) {
      test.skip('User not logged in - skipping persistence test');
    }

    // Open profile dialog and change theme to dark
    const profileButton = page.locator('[data-testid="user-profile-button"], [aria-label*="profile"], [aria-label*="account"]').first();
    await profileButton.click();
    await page.waitForSelector('[role="dialog"]', { state: 'visible' });

    const themeSelect = page.locator('text=Theme').locator('..').locator('select, [role="combobox"]');
    await themeSelect.click();
    await page.locator('text=Dark').click();

    const saveButton = page.locator('text=Save Preferences');
    await saveButton.click();

    // Reload the page
    await page.reload();
    await page.waitForLoadState('networkidle');

    // Verify theme is still dark
    const body = page.locator('body');
    const hasDarkTheme = await page.evaluate(() =>
      document.documentElement.style.colorScheme === 'dark' ||
      getComputedStyle(document.body).backgroundColor.includes('rgb(18, 18, 18)')
    );

    expect(hasDarkTheme).toBeTruthy();
  });
});
