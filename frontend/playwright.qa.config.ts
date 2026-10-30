// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { defineConfig, devices } from '@playwright/test';

/**
 * The QA scenario suite's browser scenario (scripts/qa/scenarios/anonymous-browser.sh): the
 * specs under tests/qa/browser/, signed out, against the deployed site at QA_TARGET. Starts no
 * server. See docs/release/qa-scenarios.md.
 */

const target = process.env.QA_TARGET?.replace(/\/+$/, '');
if (!target) {
  throw new Error(
    'QA_TARGET is not set. Run scripts/qa/run-scenarios.sh; see docs/release/qa-scenarios.md.'
  );
}

const launchOptions = process.env.QA_CHROMIUM_PATH
  ? { executablePath: process.env.QA_CHROMIUM_PATH }
  : {};

export default defineConfig({
  testDir: './tests/qa/browser',
  forbidOnly: true,
  // Results are evidence: a flaky check should be fixed, not retried into a pass.
  retries: 0,
  // One browser at a time, gentle on a production site.
  workers: 1,
  timeout: 120_000,
  outputDir: process.env.QA_PLAYWRIGHT_OUTPUT ?? 'test-results-qa',
  use: {
    baseURL: target,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    ...devices['Desktop Chrome'],
    launchOptions,
  },
});
