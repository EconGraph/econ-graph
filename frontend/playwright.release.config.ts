// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { defineConfig, devices } from '@playwright/test';

/**
 * Release end-to-end suite: the frontend release build against a real backend on Postgres,
 * seeded from recorded fixtures. Only specs under tests/e2e/release/ run here.
 *
 * Run it with `scripts/release-e2e.sh` from the repo root, which starts Postgres, builds the
 * backend and the seed tool, and seeds the database first. See tests/e2e/release/README.md.
 */

const backendPort = Number(process.env.RELEASE_BACKEND_PORT ?? 18080);
const frontendPort = Number(process.env.RELEASE_FRONTEND_PORT ?? 18081);
const backendUrl = `http://localhost:${backendPort}`;
const frontendUrl = `http://localhost:${frontendPort}`;
const backendBin = process.env.RELEASE_BACKEND_BIN ?? '../backend/target/debug/econ-graph-backend';

// Keycloak's own port is fixed by docker-compose.release-e2e.yml (KC_HOSTNAME + the port
// mapping), unlike the backend/frontend ports above. Its realm does depend on frontendPort,
// though: config/keycloak/econ-graph-realm.json's KC_WEB_E2E_BASE_URL is hardcoded to
// `http://localhost:18081` in that compose file, so a redirect URI or CORS mismatch will show up
// if RELEASE_FRONTEND_PORT is ever overridden alongside a sign-in flow (AUTH-10).
const oidcIssuer = 'http://localhost:8081/realms/econ-graph';

// Deliberately not DATABASE_URL, which often points at a dev database: the backend migrates
// whatever database it is given.
const databaseUrl = process.env.RELEASE_E2E_DATABASE_URL;
if (!databaseUrl) {
  throw new Error(
    'RELEASE_E2E_DATABASE_URL is not set. Run scripts/release-e2e.sh from the repo root; see tests/e2e/release/README.md.'
  );
}

export default defineConfig({
  testDir: './tests/e2e/release',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  // Retries hide real failures in a suite whose data is fixed; a flaky spec should be fixed.
  retries: 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: process.env.CI
    ? [['github'], ['html', { open: 'never', outputFolder: 'playwright-report-release' }]]
    : [['list'], ['html', { open: 'never', outputFolder: 'playwright-report-release' }]],
  outputDir: 'test-results-release',
  use: {
    baseURL: frontendUrl,
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
  webServer: [
    {
      // Built by scripts/release-e2e.sh. It runs migrations on start; the seed has already run.
      command: backendBin,
      url: `${backendUrl}/health`,
      reuseExistingServer: !process.env.CI,
      timeout: 60 * 1000,
      stdout: 'pipe',
      env: {
        DATABASE_URL: databaseUrl,
        BACKEND_PORT: String(backendPort),
        CORS_ALLOWED_ORIGINS: frontendUrl,
        // Only signs tokens for this throwaway stack. Unrelated to Keycloak: still required by
        // the in-house JWT code AUTH-6 will remove.
        JWT_SECRET: 'release-e2e-only-jwt-secret',
        // The dev realm from docker-compose.release-e2e.yml's keycloak service.
        OIDC_ISSUER: oidcIssuer,
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
        // Unused until AUTH-7's sign-in UI lands and reads it; wired here so the release e2e
        // stack, and AUTH-10's spec, have it as soon as that UI does.
        VITE_OIDC_ISSUER: oidcIssuer,
      },
    },
  ],
});
