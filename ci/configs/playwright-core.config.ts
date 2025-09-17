import { defineConfig, devices } from '@playwright/test';

/**
 * Core E2E Tests Configuration
 * Tests basic functionality: navigation, authentication, dashboard, about
 */
export default defineConfig({
  testDir: './tests/e2e/core',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 2 : undefined, // Allow 2 workers for core tests
  reporter: [['html', { open: 'never' }]],
  use: {
    baseURL: 'http://localhost:18473',
    headless: true,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },
    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'] },
    },
  ],
  webServer: {
    command: 'cd dev-server && npm start',
    url: 'http://localhost:18473',
    reuseExistingServer: !process.env.CI,
    timeout: 120 * 1000,
  },
});
