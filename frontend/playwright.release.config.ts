// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { defineConfig, devices, type PlaywrightTestConfig } from '@playwright/test';

import {
  DEPLOYED,
  OIDC_ISSUER,
  RELEASE_BASE_URL,
  SKIP_AUTH,
  checkDeployedAuthEnv,
} from './tests/e2e/release/env';

/**
 * Release end-to-end suite: the frontend release build against a real backend on Postgres,
 * seeded from recorded fixtures. Only specs under tests/e2e/release/ run here.
 *
 * Run it with `scripts/release-e2e.sh` from the repo root, which starts Postgres, builds the
 * backend and the seed tool, and seeds the database first. See tests/e2e/release/README.md.
 *
 * With RELEASE_BASE_URL set (deployed-URL mode, tests/e2e/release/env.ts) it runs against that
 * deployed frontend instead: no local servers, no database.
 */

const backendPort = Number(process.env.RELEASE_BACKEND_PORT ?? 18080);
const frontendPort = Number(process.env.RELEASE_FRONTEND_PORT ?? 18081);
const backendUrl = `http://localhost:${backendPort}`;
const frontendUrl = `http://localhost:${frontendPort}`;
const backendBin = process.env.RELEASE_BACKEND_BIN ?? '../backend/target/debug/econ-graph-backend';

// Keycloak's own port is fixed by docker-compose.release-e2e.yml (KC_HOSTNAME + the port
// mapping), unlike the backend/frontend ports above. Its realm's KC_WEB_E2E_BASE_URL reads
// RELEASE_FRONTEND_PORT (default 18081) from that same compose file, so it tracks frontendPort.
// OIDC_ISSUER (env.ts) is that realm in fixture mode.

// Deliberately not DATABASE_URL, which often points at a dev database: the backend migrates
// whatever database it is given. Deployed mode has no local backend, so needs none.
const databaseUrl = process.env.RELEASE_E2E_DATABASE_URL;
if (!DEPLOYED && !databaseUrl) {
  throw new Error(
    'RELEASE_E2E_DATABASE_URL is not set. Run scripts/release-e2e.sh from the repo root; see tests/e2e/release/README.md.'
  );
}
if (DEPLOYED && !SKIP_AUTH) checkDeployedAuthEnv();

const webServer: PlaywrightTestConfig['webServer'] = [
  {
    // Built by scripts/release-e2e.sh. It runs migrations on start; the seed has already run.
    command: backendBin,
    url: `${backendUrl}/health`,
    reuseExistingServer: !process.env.CI,
    timeout: 60 * 1000,
    stdout: 'pipe',
    env: {
      DATABASE_URL: databaseUrl ?? '', // set: checked above whenever this runs
      BACKEND_PORT: String(backendPort),
      CORS_ALLOWED_ORIGINS: frontendUrl,
      // Only signs tokens for this throwaway stack. Unrelated to Keycloak: still required by
      // the in-house JWT code AUTH-6 will remove.
      JWT_SECRET: 'release-e2e-only-jwt-secret',
      // The dev realm from docker-compose.release-e2e.yml's keycloak service.
      OIDC_ISSUER,
      OIDC_AUDIENCE: 'econ-graph-api',
      RUST_LOG: process.env.RUST_LOG ?? 'warn',
    },
  },
  {
    // The release build, served by `vite preview`, which proxies /graphql and /api to the
    // backend (vite.config.ts). The auth REST calls (AuthContext) go straight to
    // VITE_API_URL instead. FLAG_PROFILE=release selects release flags once the build-time
    // flag switch lands; until then the build ignores it.
    command: `npm run build && npx vite preview --port ${frontendPort} --strictPort`,
    url: frontendUrl,
    reuseExistingServer: !process.env.CI,
    timeout: 5 * 60 * 1000,
    env: {
      FLAG_PROFILE: 'release',
      BACKEND_URL: backendUrl,
      VITE_API_URL: backendUrl,
      // Read by AUTH-7's sign-in UI (oidc-client-ts). AUTH-10's real sign-in spec uses it too.
      VITE_OIDC_ISSUER: OIDC_ISSUER,
    },
  },
];

export default defineConfig({
  testDir: './tests/e2e/release',
  // The auth specs are one folder so a QA run can drop them while sign-in is broken.
  testIgnore: SKIP_AUTH ? ['auth/**'] : [],
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  // Retries hide real failures in a suite whose data is fixed; a flaky spec should be fixed.
  retries: 0,
  // Deployed mode runs one test at a time: the deployed realm has only two QA users to share
  // between specs, and Keycloak locks a user out whose sign-ins overlap.
  workers: DEPLOYED ? 1 : process.env.CI ? 2 : undefined,
  reporter: process.env.CI
    ? [['github'], ['html', { open: 'never', outputFolder: 'playwright-report-release' }]]
    : [['list'], ['html', { open: 'never', outputFolder: 'playwright-report-release' }]],
  outputDir: 'test-results-release',
  use: {
    baseURL: RELEASE_BASE_URL ?? frontendUrl,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'release',
      use: {
        ...devices['Desktop Chrome'],
        // A preinstalled Chromium whose revision differs from this Playwright's, e.g.
        // /opt/pw-browsers/chromium in Claude Code's cloud containers. CI installs its own.
        launchOptions: process.env.RELEASE_CHROMIUM_PATH
          ? { executablePath: process.env.RELEASE_CHROMIUM_PATH }
          : {},
      },
    },
  ],
  webServer: DEPLOYED ? undefined : webServer,
});
