import { defineConfig, devices } from '@playwright/test';

/**
 * Analysis E2E Tests Configuration
 * Tests analysis features: professional analysis, global analysis, series explorer
 */
export default defineConfig({
  testDir: './tests/e2e/analysis',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined, // Single worker for analysis tests (more complex)
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
