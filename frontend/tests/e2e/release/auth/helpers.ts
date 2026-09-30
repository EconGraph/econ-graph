// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, type APIRequestContext, type Page } from '@playwright/test';

import { OIDC_ISSUER, type ReleaseUser } from '../env';
import { SEEDED } from '../fixtures';

// The realm the frontend signs in with (env.ts): in fixture mode the dev realm
// docker-compose.release-e2e.yml imports (config/keycloak/econ-graph-realm.json, users from
// config/keycloak/dev/), whose users' passwords are `<username>-dev-password` and which
// config/keycloak/README.md keeps stable. Unset in deployed mode with RELEASE_SKIP_AUTH=1, so
// it is parsed only when someone signs in.

/**
 * Signs in on Keycloak's own login page. Call it once the app has redirected there, for example
 * after pressing Sign in; it waits for the redirect back to the app's origin.
 *
 * Never sign the same user in from two specs at once. When two specs signed alice in in parallel,
 * Keycloak refused one locally with `user_temporarily_disabled` (the realm's `bruteForceProtected`
 * had locked her; the page said "Invalid username or password"), and the same test timed out at
 * this step in CI. `USERS` in env.ts gives each spec its own user.
 * @param page - A page showing, or about to show, Keycloak's login form.
 * @param user - A realm user, from `USERS`.
 */
export async function signInOnKeycloak(page: Page, user: ReleaseUser) {
  const issuer = new URL(OIDC_ISSUER);
  const keycloakOrigin = issuer.origin;
  const realmPath = issuer.pathname.replace(/\/+$/, '');
  // The authorization endpoint of this realm, not some other page on Keycloak's origin.
  await page.waitForURL(
    url => url.origin === keycloakOrigin && url.pathname.startsWith(`${realmPath}/`)
  );
  await page.locator('#username').fill(user.username);
  await page.locator('#password').fill(user.password);
  await page.locator('#kc-login').click();
  // Fail with Keycloak's own reason when it refuses the login, not with a bare test timeout. The
  // refusal wait resolves null if it fails (e.g. the page closes), so only a shown error can win.
  const refused = page.locator('#input-error-username');
  const refusal = refused.waitFor().then(
    async () => `Keycloak refused ${user.username}: ${await refused.innerText()}`,
    () => null
  );
  const outcome = await Promise.race([
    page.waitForURL(url => url.origin !== keycloakOrigin).then(() => null),
    refusal,
  ]);
  if (outcome) throw new Error(outcome);
}

/** A seeded series, as `SEEDED` in fixtures.ts describes it. */
export interface SeededSeries {
  sourceName: string;
  externalId: string;
  title: string;
}

/**
 * Looks up a seeded series' id, to open its page. Setup only: through the frontend's /graphql
 * proxy, it finds the source's id in `dataSources`, searches that source for the series' title,
 * and picks the result with the series' external id. Search, not the plain series list, so it
 * also finds the series among a deployed build's thousands.
 * @param request - The test's request context (baseURL is the frontend).
 * @param series - The series, from `SEEDED`.
 * @returns The series id (UUID).
 */
export async function seededSeriesId(
  request: APIRequestContext,
  series: SeededSeries = SEEDED.fredGdp
): Promise<string> {
  const graphql = async (query: string, variables?: object) => {
    const response = await request.post('/graphql', { data: { query, variables } });
    expect(response.ok()).toBe(true);
    const body = await response.json();
    expect(body.errors).toBeUndefined();
    return body.data;
  };

  const { dataSources } = await graphql('{ dataSources { id name } }');
  const source = dataSources.find((s: { name: string }) => s.name === series.sourceName);
  expect(source, `data source ${series.sourceName}`).toBeDefined();

  // `source` takes the source's id; a name would be ignored and every source searched.
  const { searchSeries } = await graphql(
    'query($q: String!, $source: String) { searchSeries(query: $q, source: $source, first: 50) { series { id externalId sourceId } } }',
    { q: series.title, source: source.id }
  );
  const node = searchSeries.series.find(
    (n: { externalId: string; sourceId: string }) =>
      n.externalId === series.externalId && n.sourceId === source.id
  );
  expect(node, `seeded series ${series.sourceName} ${series.externalId}`).toBeDefined();
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
