import { defineConfig, devices } from '@playwright/test';

/**
 * Mobile Analysis Tests Configuration
 * Mobile-specific analysis features: charts, interactions, data visualization
 * @see https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  testDir: './tests/e2e',
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
    baseURL: 'http://localhost:18473',

    /* Run in headless mode by default */
    headless: true,

    /* Collect trace when retrying the failed test. See https://playwright.dev/docs/trace-viewer */
    trace: 'on-first-retry',

    /* Take screenshot on failure */
    screenshot: 'only-on-failure',

    /* Record video on failure */
    video: 'retain-on-failure',
  },

  /* Configure projects for mobile browsers - analysis features */
  projects: [
    /* Test against mobile viewports - Chrome only for CI stability */
    {
      name: 'Mobile Chrome Analysis',
      use: { ...devices['Pixel 5'] },
      testMatch: [
        '**/professional-analysis.spec.ts',
        '**/global-analysis.spec.ts',
      ],
    },
    {
      name: 'Mobile Chrome iPhone Analysis',
      use: { ...devices['iPhone 12'] },
      testMatch: [
        '**/professional-analysis.spec.ts',
        '**/global-analysis.spec.ts',
      ],
    },
  ],

  /* Run your local dev server before starting the tests */
  webServer: {
    command: 'cd dev-server && npm start',
    url: 'http://localhost:18473',
    reuseExistingServer: !process.env.CI,
    timeout: 120 * 1000,
  },
});
