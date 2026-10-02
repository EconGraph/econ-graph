import { defineConfig, devices } from '@playwright/test';

/**
 * Global Analysis E2E Tests Configuration
 * Tests global analysis features: world map, country selection, economic indicators.
 *
 * The world map's mocked specs (world-map-mocked.spec.ts, world-atlas.spec.ts) live under
 * tests/e2e/release/world-map/ alongside MAP-7's seeded-backend release spec
 * (world-map.spec.ts), which needs the release fixture stack and doesn't belong in this
 * standalone suite, hence testIgnore below (ECO-282).
 */
export default defineConfig({
  testDir: '../../tests/e2e/release/world-map',
  testIgnore: ['world-map.spec.ts'],
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined, // Single worker for global analysis tests (complex map interactions)
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
