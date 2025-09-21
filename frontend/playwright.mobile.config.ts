import { defineConfig, devices } from '@playwright/test';

/**
 * Mobile browser e2e tests configuration
 * @see https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  testDir: '/app/tests/e2e',
  /* Run tests in files in parallel */
  fullyParallel: true,
  /* Fail the build on CI if you accidentally left test.only in the source code. */
  forbidOnly: !!process.env.CI,
  /* Retry on CI only */
  retries: process.env.CI ? 2 : 0,
  /* Opt out of parallel tests on CI. */
  workers: process.env.CI ? 1 : undefined,
  /* Reporter to use. See https://playwright.dev/docs/test-reporters */
  reporter: [['html', { open: 'never' }]],
  /* Shared settings for all the projects below. See https://playwright.dev/docs/api/class-testoptions. */
  use: {
    /* Base URL to use in actions like `await page.goto('/')`. */
    baseURL: process.env.BASE_URL || 'http://localhost:3000',

    /* Run in headless mode by default */
    headless: true,

    /* Collect trace when retrying the failed test. See https://playwright.dev/docs/trace-viewer */
    trace: 'on-first-retry',

    /* Take screenshot on failure */
    screenshot: 'only-on-failure',

    /* Record video on failure */
    video: 'retain-on-failure',
  },

  /* Configure projects for mobile browsers only */
  projects: [
    /* Test against mobile viewports - Chrome only for CI stability */
    {
      name: 'Mobile Chrome',
      use: { ...devices['Pixel 5'] },
    },
    {
      name: 'Mobile Chrome (Galaxy S5)',
      use: { ...devices['Galaxy S5'] },
    },
    {
      name: 'Mobile Chrome (iPhone 12)',
      use: { ...devices['iPhone 12'] },
    },
    {
      name: 'Mobile Chrome (iPhone SE)',
      use: { ...devices['iPhone SE'] },
    },
  ],
});
