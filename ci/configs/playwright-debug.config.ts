import { defineConfig, devices } from '@playwright/test';

/**
 * Debug E2E Tests Configuration
 * Tests debugging and visual features
 */
export default defineConfig({
  testDir: '../../tests/e2e',
  testMatch: ['**/*debug*.spec.ts'],
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0, // Fewer retries for debug tests
  workers: process.env.CI ? 2 : undefined,
  reporter: [['html', { open: 'never' }]],
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:3000',
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
    // Debug tests only need Chrome for consistency
  ],
});
