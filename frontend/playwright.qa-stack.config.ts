// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { defineConfig } from '@playwright/test';

import { frontendUrl, webServer } from './playwright.release.config';
import { OIDC_ISSUER } from './tests/e2e/release/env';

/**
 * The QA scenario suite against the release end-to-end stack: the same backend, frontend
 * release build and Keycloak as playwright.release.config.ts, started the same way, with
 * tests/qa/stack/scenarios.spec.ts running scripts/qa/run-scenarios.sh --profile ci against them.
 *
 * Run it with `RELEASE_E2E_CONFIG=playwright.qa-stack.config.ts scripts/release-e2e.sh` from the
 * repo root, which starts Postgres and Keycloak and seeds the database first. See
 * docs/release/qa-scenarios.md.
 */

// Read by tests/qa/stack/scenarios.spec.ts.
process.env.QA_STACK_TARGET = frontendUrl;
process.env.QA_STACK_OIDC_ISSUER = OIDC_ISSUER;

export default defineConfig({
  testDir: './tests/qa/stack',
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: 1,
  // The whole suite, browser scenario included, in one test.
  timeout: 40 * 60 * 1000,
  reporter: process.env.CI ? [['github'], ['list']] : [['list']],
  outputDir: 'test-results-qa-stack',
  webServer,
});
