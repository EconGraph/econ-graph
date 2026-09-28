// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

/**
 * Which stack the release suite runs against, read once from the environment. Shared by
 * playwright.release.config.ts and the specs.
 *
 * - Fixture mode (default): scripts/release-e2e.sh's local stack, seeded from recorded fixtures.
 *   Specs assert exact seeded values.
 * - Deployed-URL mode: `RELEASE_BASE_URL` points at a deployed frontend (REL-QA check 3). No
 *   local servers or database; specs assert live data against each source's cadence instead, and
 *   sign in as the QA-only realm users.
 */

/** The deployed frontend's origin, e.g. `https://qa.econgraph.example`; unset in fixture mode. */
export const RELEASE_BASE_URL = process.env.RELEASE_BASE_URL?.replace(/\/+$/, '') || undefined;

/** True when the suite runs against a deployed build rather than the fixture stack. */
export const DEPLOYED = RELEASE_BASE_URL !== undefined;

/**
 * True when `RELEASE_SKIP_AUTH=1`: the auth/ specs are not run and the journey stops before its
 * signed-in step. For a QA run while sign-in is known broken (plan review E); never set in CI.
 */
export const SKIP_AUTH = process.env.RELEASE_SKIP_AUTH === '1';

/**
 * The Keycloak realm the frontend signs in with. Fixture mode: the dev realm
 * docker-compose.release-e2e.yml starts. Deployed mode: `RELEASE_OIDC_ISSUER`, the same value as
 * the build's `VITE_OIDC_ISSUER`.
 */
export const OIDC_ISSUER = DEPLOYED
  ? (process.env.RELEASE_OIDC_ISSUER ?? '')
  : 'http://localhost:8081/realms/econ-graph';

/** A realm user the specs sign in as. */
export interface ReleaseUser {
  username: string;
  password: string;
  /** First and last name, as the header's user menu shows it. */
  displayName: string;
  email: string;
}

const devUser = (username: string, displayName: string): ReleaseUser => ({
  username,
  password: `${username}-dev-password`,
  displayName,
  email: `${username}@example.com`,
});

/**
 * One of the two QA-only realm users (config/keycloak/qa/). Passwords come from the environment
 * so a deployed realm can use its own; the other fields default to the QA users file.
 * @param n - 1 or 2.
 * @param username - The QA users file's username.
 * @param displayName - The QA users file's first and last name.
 * @returns The user, with an empty password when `RELEASE_QA_USER<n>_PASSWORD` is unset.
 */
const qaUser = (n: 1 | 2, username: string, displayName: string): ReleaseUser => {
  const name = process.env[`RELEASE_QA_USER${n}`] ?? username;
  return {
    username: name,
    password: process.env[`RELEASE_QA_USER${n}_PASSWORD`] ?? '',
    displayName: process.env[`RELEASE_QA_USER${n}_NAME`] ?? displayName,
    email: process.env[`RELEASE_QA_USER${n}_EMAIL`] ?? `${name}@example.com`,
  };
};

/**
 * Who each spec signs in as. `annotator` writes public and private annotations and `viewer` checks
 * it sees only the public one (auth/sign-in.spec.ts); `journey` annotates at the end of
 * journey.spec.ts.
 *
 * The dev realm has a user per role, so no user signs in from two specs at once (Keycloak's
 * brute-force detection locks a user out when that happens). The deployed realm has only the two
 * QA users, so there `journey` is the viewer, and deployed mode runs one spec at a time
 * (playwright.release.config.ts).
 */
export const USERS: Record<'annotator' | 'viewer' | 'journey', ReleaseUser> = DEPLOYED
  ? {
      annotator: qaUser(1, 'qa-alice', 'QA Alice'),
      viewer: qaUser(2, 'qa-bob', 'QA Bob'),
      journey: qaUser(2, 'qa-bob', 'QA Bob'),
    }
  : {
      annotator: devUser('alice', 'Alice Tester'),
      viewer: devUser('bob', 'Bob Tester'),
      journey: devUser('dave', 'Dave Tester'),
    };

/**
 * Fails fast with what to set when deployed mode lacks what the signed-in specs need.
 * Called from playwright.release.config.ts, unless the auth specs are skipped.
 */
export function checkDeployedAuthEnv() {
  const missing = [
    !OIDC_ISSUER && 'RELEASE_OIDC_ISSUER',
    !USERS.annotator.password && 'RELEASE_QA_USER1_PASSWORD',
    !USERS.viewer.password && 'RELEASE_QA_USER2_PASSWORD',
  ].filter(Boolean);
  if (missing.length > 0) {
    throw new Error(
      `RELEASE_BASE_URL is set, so the signed-in specs need ${missing.join(', ')}. ` +
        'Set RELEASE_SKIP_AUTH=1 to run without them; see tests/e2e/release/README.md.'
    );
  }
}
