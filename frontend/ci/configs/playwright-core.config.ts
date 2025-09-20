import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '/Users/josephmalicki/src/econ-graph5/frontend/tests/e2e/core',
  projects: [
    {
      name: 'chromium',
      use: { headless: true },
    },
  ],
});
