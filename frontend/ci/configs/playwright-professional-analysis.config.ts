import { defineConfig, devices } from '@playwright/test';

/**
 * Professional Analysis E2E Tests Configuration
 * Tests professional analysis features: advanced charting, technical indicators, correlation analysis
 */
export default defineConfig({
  testDir: '/Users/josephmalicki/src/econ-graph5/frontend/tests/e2e/professional-analysis',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined, // Single worker for professional analysis tests (complex charting)
  reporter: [['html', { open: 'never' }]],
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:3000',
    headless: true,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    actionTimeout: 30000,
    navigationTimeout: 30000,
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
});
