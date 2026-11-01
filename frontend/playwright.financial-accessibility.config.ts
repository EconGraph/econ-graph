import { defineConfig, devices } from '@playwright/test';

// Focused component/browser regressions; the fixture is not a production route
// or build entry, and uses the normal Vite config and production styles.
export default defineConfig({
  testDir: './tests/accessibility',
  testMatch: '**/*.spec.ts',
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:4175',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    launchOptions: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE
      ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE }
      : undefined,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: 'npm run dev -- --host 127.0.0.1 --port 4175 --strictPort',
    url: 'http://127.0.0.1:4175/tests/accessibility/fixtures/financial.html',
    reuseExistingServer: !process.env.CI,
  },
});
