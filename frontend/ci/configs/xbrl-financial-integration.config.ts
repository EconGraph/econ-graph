/**
 * XBRL Financial Integration Test Configuration
 *
 * This configuration defines the test suite for XBRL financial statement
 * integration tests, ensuring proper setup and execution of financial
 * data processing and visualization components.
 */

import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '../src/__tests__/integration/xbrl-financial',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: [
    ['html'],
    ['json', { outputFile: 'test-results/xbrl-financial-integration-results.json' }],
    ['junit', { outputFile: 'test-results/xbrl-financial-integration-results.xml' }],
  ],
  use: {
    baseURL: 'http://localhost:3000',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'xbrl-financial-integration',
      testMatch: /.*\.test\.(ts|tsx)$/,
      use: {
        // Use Chrome for financial component testing
        browserName: 'chromium',
        viewport: { width: 1280, height: 720 },
        // Enable financial data visualization features
        launchOptions: {
          args: [
            '--enable-features=VizDisplayCompositor',
            '--disable-web-security',
            '--disable-features=VizDisplayCompositor',
          ],
        },
      },
    },
  ],

  // Test timeout configuration
  timeout: 30000,
  expect: {
    timeout: 10000,
  },

  // Web server configuration for integration tests
  webServer: {
    command: 'npm start',
    url: 'http://localhost:3000',
    reuseExistingServer: !process.env.CI,
    timeout: 120000,
    env: {
      REACT_APP_API_BASE_URL: 'http://localhost:8000',
      REACT_APP_ENABLE_XBRL_FEATURES: 'true',
      REACT_APP_ENABLE_FINANCIAL_ANALYSIS: 'true',
      REACT_APP_MOCK_API_RESPONSES: 'true',
    },
  },

  // Global setup and teardown
  globalSetup: require.resolve('./setup/xbrl-financial-global-setup.ts'),
  globalTeardown: require.resolve('./setup/xbrl-financial-global-teardown.ts'),
});
