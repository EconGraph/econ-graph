import { defineConfig, devices } from '@playwright/test';

const baseURL = process.env.TRAIN5_BASE_URL;
const candidate = process.env.TRAIN5_CANDIDATE_SHA;
const storageState = process.env.TRAIN5_AUTH_STORAGE_STATE;
if (!baseURL || !candidate || !/^[0-9a-f]{40}$/.test(candidate) || !storageState) {
  throw new Error(
    'Set TRAIN5_BASE_URL, TRAIN5_CANDIDATE_SHA (full commit SHA) and TRAIN5_AUTH_STORAGE_STATE for a signed-in real-backend run.'
  );
}
export default defineConfig({
  testDir: './tests/e2e/train5',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  metadata: { candidate_sha: candidate, backend_mode: 'real', authenticated: true },
  reporter: [
    ['list'],
    [
      'json',
      { outputFile: process.env.TRAIN5_PLAYWRIGHT_REPORT ?? 'test-results/train5-playwright.json' },
    ],
  ],
  use: { baseURL, storageState, trace: 'retain-on-failure' },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        ...(process.env.RELEASE_CHROMIUM_PATH
          ? { launchOptions: { executablePath: process.env.RELEASE_CHROMIUM_PATH } }
          : {}),
      },
    },
  ],
});
