import { defineConfig, devices } from '@playwright/test';

/**
 * Mobile Core Tests Configuration
 * Essential mobile functionality: navigation, basic interactions, responsive design
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

  /* Configure projects for mobile browsers - core functionality only */
  projects: [
    /* Test against mobile viewports - Chrome only for CI stability */
    {
      name: 'Mobile Chrome Core',
      use: { ...devices['Pixel 5'] },
      testMatch: [
        '**/authentication.spec.ts',
        '**/series-explorer.spec.ts',
        '**/visual-check.spec.ts',
      ],
    },
    {
      name: 'Mobile Chrome Galaxy Core',
      use: { ...devices['Galaxy S5'] },
      testMatch: [
        '**/authentication.spec.ts',
        '**/series-explorer.spec.ts',
        '**/visual-check.spec.ts',
      ],
    },
  ],
});
