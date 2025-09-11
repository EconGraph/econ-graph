import { test, expect } from '@playwright/test';

test.describe('Authentication', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('should display login button when not authenticated', async ({ page }) => {
    // Look for login button or sign in link
    const loginButton = page.getByRole('button', { name: /login|sign in/i }).or(
      page.getByRole('link', { name: /login|sign in/i })
    );

    await expect(loginButton).toBeVisible();
  });

  test('should open login dialog when login button is clicked', async ({ page }) => {
    const loginButton = page.getByRole('button', { name: /login|sign in/i }).or(
      page.getByRole('link', { name: /login|sign in/i })
    );

    await loginButton.click();

    // Check for login dialog or form
    const loginDialog = page.locator('[role="dialog"]').or(
      page.locator('form').or(
        page.locator('[data-testid="login-dialog"]')
      )
    );

    await expect(loginDialog).toBeVisible();
  });

  test('should display email and password fields in login form', async ({ page }) => {
    const loginButton = page.getByRole('button', { name: /login|sign in/i }).or(
      page.getByRole('link', { name: /login|sign in/i })
    );

    await loginButton.click();

    // Check for email and password fields
    await expect(page.getByLabel(/email/i)).toBeVisible();
    await expect(page.getByLabel(/password/i)).toBeVisible();
  });

  test('should display OAuth login options', async ({ page }) => {
    const loginButton = page.getByRole('button', { name: /login|sign in/i }).or(
      page.getByRole('link', { name: /login|sign in/i })
    );

    await loginButton.click();

    // Check for Google and Facebook login buttons
    const googleButton = page.getByRole('button', { name: /google/i }).or(
      page.getByText(/google/i)
    );
    const facebookButton = page.getByRole('button', { name: /facebook/i }).or(
      page.getByText(/facebook/i)
    );

    // At least one OAuth option should be available
    const hasOAuth = await googleButton.isVisible() || await facebookButton.isVisible();
    expect(hasOAuth).toBeTruthy();
  });

  test('should show registration option', async ({ page }) => {
    const loginButton = page.getByRole('button', { name: /login|sign in/i }).or(
      page.getByRole('link', { name: /login|sign in/i })
    );

    await loginButton.click();

    // Look for registration/signup link or button
    const registerLink = page.getByRole('link', { name: /register|sign up|create account/i }).or(
      page.getByRole('button', { name: /register|sign up|create account/i })
    );

    await expect(registerLink).toBeVisible();
  });

  test('should handle login form validation', async ({ page }) => {
    const loginButton = page.getByRole('button', { name: /login|sign in/i }).or(
      page.getByRole('link', { name: /login|sign in/i })
    );

    await loginButton.click();

    // Try to submit empty form
    const submitButton = page.getByRole('button', { name: /login|sign in|submit/i });
    if (await submitButton.isVisible()) {
      await submitButton.click();

      // Should show validation errors
      const errorMessage = page.locator('[role="alert"]').or(
        page.locator('.error').or(
          page.locator('[data-testid="error"]')
        )
      );

      // Error message should appear (either validation or network error)
      await expect(errorMessage).toBeVisible();
    }
  });

  test('should close login dialog when cancel is clicked', async ({ page }) => {
    const loginButton = page.getByRole('button', { name: /login|sign in/i }).or(
      page.getByRole('link', { name: /login|sign in/i })
    );

    await loginButton.click();

    const cancelButton = page.getByRole('button', { name: /cancel|close/i });
    if (await cancelButton.isVisible()) {
      await cancelButton.click();

      const loginDialog = page.locator('[role="dialog"]').or(
        page.locator('[data-testid="login-dialog"]')
      );

      await expect(loginDialog).not.toBeVisible();
    }
  });

  test('should handle OAuth login attempts gracefully', async ({ page }) => {
    const loginButton = page.getByRole('button', { name: /login|sign in/i }).or(
      page.getByRole('link', { name: /login|sign in/i })
    );

    await loginButton.click();

    // Try to click Google login button
    const googleButton = page.getByRole('button', { name: /google/i });
    if (await googleButton.isVisible()) {
      // This should trigger OAuth flow or show error
      await googleButton.click();

      // Wait for either OAuth popup or error message
      await page.waitForTimeout(2000);

      // Should either redirect to OAuth or show error
      const hasError = await page.locator('[role="alert"]').isVisible();
      const hasOAuthPopup = page.url().includes('google') || page.url().includes('oauth');

      expect(hasError || hasOAuthPopup).toBeTruthy();
    }
  });
});
