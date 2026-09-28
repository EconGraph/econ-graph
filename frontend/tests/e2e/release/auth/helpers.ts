// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, type APIRequestContext, type Page } from '@playwright/test';

import { SEEDED } from '../fixtures';

// The realm docker-compose.release-e2e.yml imports (config/keycloak/econ-graph-realm.json, users
// from config/keycloak/dev/). Keep in sync with oidcIssuer in playwright.release.config.ts. The
// users' passwords are `<username>-dev-password`; config/keycloak/README.md keeps them stable.
const KEYCLOAK_ORIGIN = 'http://localhost:8081';
const REALM_PATH = '/realms/econ-graph';

export type DevUser = 'alice' | 'bob';

/**
 * Signs in on Keycloak's own login page. Call it once the app has redirected there, for example
 * after pressing Sign in; it waits for the redirect back to the app's origin.
 *
 * Never sign the same user in from two specs at once. When two specs signed alice in in parallel,
 * Keycloak refused one locally with `user_temporarily_disabled` (the realm's `bruteForceProtected`
 * had locked her; the page said "Invalid username or password"), and the same test timed out at
 * this step in CI.
 * @param page - A page showing, or about to show, Keycloak's login form.
 * @param user - A dev-realm user.
 */
export async function signInOnKeycloak(page: Page, user: DevUser) {
  // The authorization endpoint of this realm, not some other page on port 8081.
  await page.waitForURL(
    url => url.origin === KEYCLOAK_ORIGIN && url.pathname.startsWith(`${REALM_PATH}/`)
  );
  await page.locator('#username').fill(user);
  await page.locator('#password').fill(`${user}-dev-password`);
  await page.locator('#kc-login').click();
  // Fail with Keycloak's own reason when it refuses the login, not with a bare test timeout. The
  // refusal wait resolves null if it fails (e.g. the page closes), so only a shown error can win.
  const refused = page.locator('#input-error-username');
  const refusal = refused.waitFor().then(
    async () => `Keycloak refused ${user}: ${await refused.innerText()}`,
    () => null
  );
  const outcome = await Promise.race([
    page.waitForURL(url => url.origin !== KEYCLOAK_ORIGIN).then(() => null),
    refusal,
  ]);
  if (outcome) throw new Error(outcome);
}

/**
 * Looks up a seeded series' id, to open its page. Setup only: it reads the public series list
 * through the frontend's /graphql proxy, the way the smoke spec does.
 * @param request - The test's request context (baseURL is the frontend).
 * @param externalId - The series' external id, from `SEEDED`.
 * @returns The series id (UUID).
 */
export async function seededSeriesId(
  request: APIRequestContext,
  externalId: string = SEEDED.fredGdp.externalId
): Promise<string> {
  const response = await request.post('/graphql', {
    data: { query: '{ seriesList(pagination: { first: 50 }) { nodes { id externalId } } }' },
  });
  expect(response.ok()).toBe(true);
  const body = await response.json();
  expect(body.errors).toBeUndefined();
  const node = body.data.seriesList.nodes.find(
    (n: { externalId: string }) => n.externalId === externalId
  );
  expect(node, `seeded series ${externalId}`).toBeDefined();
  return node.id as string;
}

/**
 * A title no earlier run or parallel spec has used. The database is shared by parallel specs and
 * may be reused between runs (RELEASE_E2E_DATABASE_URL, Playwright run without the script,
 * --repeat-each), so specs never assume the annotation list starts empty.
 * @param label - What the annotation is, for reading a failure.
 * @returns The unique title.
 */
export function uniqueTitle(label: string): string {
  return `${label} ${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}
