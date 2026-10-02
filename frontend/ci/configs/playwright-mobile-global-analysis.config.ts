import { defineConfig, devices } from '@playwright/test';

/**
 * Mobile Global Analysis E2E Tests Configuration
 * Tests global analysis features on mobile devices.
 *
 * Same mocked specs as playwright-global-analysis.config.ts (ECO-282); the release spec
 * (world-map.spec.ts) needs the release fixture stack and doesn't run here.
 */
export default defineConfig({
  testDir: '../../tests/e2e/release/world-map',
  testIgnore: ['world-map.spec.ts'],
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined, // Single worker for mobile global analysis tests
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
      name: 'mobile-chrome',
      use: { ...devices['Pixel 5'] },
    },
    {
      name: 'mobile-safari',
      use: { ...devices['iPhone 12'] },
    },
  ],
});
