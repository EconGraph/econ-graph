import type { PlaywrightTestConfig } from '@playwright/test';
import { testRunnerConfig } from '@storybook/test-runner';

const config: PlaywrightTestConfig = {
  ...testRunnerConfig,
  // Use the same port as Storybook
  use: {
    baseURL: 'http://localhost:6006',
  },
  // Configure test patterns
  testMatch: /.*\.stories\.(js|jsx|ts|tsx)$/,
  // Configure retries for CI
  retries: process.env.CI ? 2 : 0,
  // Configure timeout
  timeout: 30000,
  // Configure workers for parallel execution
  workers: process.env.CI ? 1 : undefined,
  // Configure reporter
  reporter: [
    ['html'],
    ['json', { outputFile: 'test-results/storybook-test-results.json' }],
    ['junit', { outputFile: 'test-results/storybook-test-results.xml' }],
  ],
  // Configure projects for different browsers
  projects: [
    {
      name: 'chromium',
      use: { ...testRunnerConfig.use, browserName: 'chromium' },
    },
    {
      name: 'firefox',
      use: { ...testRunnerConfig.use, browserName: 'firefox' },
    },
    {
      name: 'webkit',
      use: { ...testRunnerConfig.use, browserName: 'webkit' },
    },
  ],
};

export default config;
