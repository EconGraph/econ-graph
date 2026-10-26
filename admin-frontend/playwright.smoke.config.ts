import { defineConfig, devices } from "@playwright/test";

// Serve the production output, so Node globals supplied by Vitest cannot hide
// browser startup failures. Keep this independent of backend-dependent E2E tests.
export default defineConfig({
  testDir: "./tests/smoke",
  outputDir: "./test-results-smoke",
  forbidOnly: !!process.env.CI,
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:4174",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: devices["Desktop Chrome"] }],
  webServer: {
    command: "npm run preview -- --host 127.0.0.1 --port 4174 --strictPort",
    url: "http://127.0.0.1:4174/admin/",
    reuseExistingServer: false,
  },
});
