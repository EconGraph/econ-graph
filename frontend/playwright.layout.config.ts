import { defineConfig } from '@playwright/test';

// Production entrypoints and styles; deterministic API responses live in tests/layout.
export default defineConfig({
  testDir: './tests/layout',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report-layout' }]],
  outputDir: 'test-results-layout',
  use: {
    browserName: 'chromium',
    locale: 'en-US',
    timezoneId: 'UTC',
    colorScheme: 'light',
    reducedMotion: 'reduce',
    deviceScaleFactor: 1,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    launchOptions: process.env.LAYOUT_CHROMIUM_PATH
      ? { executablePath: process.env.LAYOUT_CHROMIUM_PATH }
      : {},
  },
  projects: [
    { name: 'desktop', use: { viewport: { width: 1440, height: 1000 } } },
    {
      name: 'mobile',
      use: { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true },
    },
  ],
  webServer: [
    {
      command: 'npm run build && npx vite preview --host 127.0.0.1 --port 18181 --strictPort',
      url: 'http://127.0.0.1:18181',
      env: { FLAGS_PROFILE: 'release', VITE_OIDC_ISSUER: '', VITE_GRAPHQL_URL: '/graphql' },
      timeout: 180_000,
      reuseExistingServer: false,
    },
    {
      command: 'npx vite build && npx vite preview --host 127.0.0.1 --port 18182 --strictPort',
      cwd: '../admin-frontend',
      url: 'http://127.0.0.1:18182/admin/',
      timeout: 180_000,
      reuseExistingServer: false,
    },
  ],
});
