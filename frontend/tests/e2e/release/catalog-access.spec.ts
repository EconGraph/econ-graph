// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';

import { expect, test, type APIRequestContext } from '@playwright/test';

import { devUserToken, readAsDevUser, seededSeriesId } from './auth/helpers';
import { DEPLOYED } from './env';
import { SEEDED } from './fixtures';
import { latestShown, splitSeriesCsv } from './seriesPage';

// Catalog access (config/keycloak/README.md, "Catalog access"): FRED is readable by admins only.
// Visitors and signed-in users who aren't admins see no FRED source, series or search result,
// while the public sources stay readable; an admin reads FRED's series and pages. Live QA: the
// visitor checks run against a deployed build too; the deployed realm has no admin or ordinary
// dev user to read as, so the others are fixture mode only.

const FRED = SEEDED.fredGdp.sourceName;

/**
 * Runs a GraphQL query through the frontend's /graphql proxy, as `token`'s user or anonymously.
 * @param request - The test's request context.
 * @param query - The query.
 * @param variables - Its variables.
 * @param token - An access token, or none for a visitor.
 * @returns The response's `data`.
 */
async function graphql(
  request: APIRequestContext,
  query: string,
  variables: object = {},
  token?: string
) {
  const response = await request.post('/graphql', {
    data: { query, variables },
    headers: token ? { Authorization: `Bearer ${token}` } : undefined,
  });
  expect(response.ok()).toBe(true);
  const body = await response.json();
  expect(body.errors).toBeUndefined();
  return body.data;
}

/**
 * What one reader can see of FRED and of a public source.
 * @param request - The test's request context.
 * @param token - The reader's access token, or none for a visitor.
 * @returns The data sources' names, FRED's GDP and USREC by external id, the external ids
 *   "gross domestic product" finds in order, and BLS's CPI by external id.
 */
async function view(request: APIRequestContext, token?: string) {
  const byId =
    'query($s: String!, $e: String!) { seriesByExternalId(sourceName: $s, externalId: $e) { externalId } }';
  const sources = await graphql(request, '{ dataSources { name } }', {}, token);
  const gdp = await graphql(request, byId, { s: FRED, e: 'GDP' }, token);
  const usrec = await graphql(request, byId, { s: FRED, e: 'USREC' }, token);
  const cpi = await graphql(
    request,
    byId,
    { s: SEEDED.blsCpi.sourceName, e: SEEDED.blsCpi.externalId },
    token
  );
  const search = await graphql(
    request,
    'query($q: String!) { searchSeries(query: $q, first: 50) { series { externalId title } } }',
    { q: 'gross domestic product' },
    token
  );
  return {
    sources: sources.dataSources.map((s: { name: string }) => s.name) as string[],
    gdp: gdp.seriesByExternalId,
    usrec: usrec.seriesByExternalId,
    cpi: cpi.seriesByExternalId,
    searchTitles: search.searchSeries.series.map((s: { title: string }) => s.title) as string[],
  };
}

test.describe('catalog access: FRED is admin-only', () => {
  test('a visitor sees no FRED source, series or search result, and public sources', async ({
    request,
    page,
  }) => {
    const seen = await view(request);
    expect(seen.sources).not.toContain(FRED);
    expect(seen.sources).toContain(SEEDED.blsCpi.sourceName);
    expect(seen.gdp).toBeNull();
    expect(seen.usrec).toBeNull();
    expect(seen.cpi).toEqual({ externalId: SEEDED.blsCpi.externalId });
    expect(seen.searchTitles).not.toContain(SEEDED.fredGdp.title);
    expect(seen.searchTitles).toContain(SEEDED.beaGdp.title);

    // The sources page lists no FRED.
    await page.goto('/sources');
    await expect(page.getByRole('region', { name: SEEDED.blsCpi.sourceName })).toBeVisible();
    await expect(page.getByRole('region', { name: FRED })).toHaveCount(0);
  });

  test('a signed-in user who is not an admin sees no FRED either', async ({ request }) => {
    test.skip(DEPLOYED, 'the deployed realm has no dev user to read as');
    const seen = await view(request, await devUserToken(request, 'grace'));
    expect(seen.sources).not.toContain(FRED);
    expect(seen.gdp).toBeNull();
    expect(seen.usrec).toBeNull();
    expect(seen.cpi).toEqual({ externalId: SEEDED.blsCpi.externalId });
    expect(seen.searchTitles).not.toContain(SEEDED.fredGdp.title);
  });

  test('an admin reads FRED: its source, series, search results and series page', async ({
    request,
    page,
  }) => {
    test.skip(DEPLOYED, 'the deployed realm has no admin to read as');
    const token = await devUserToken(request, 'staff-admin');
    const seen = await view(request, token);
    expect(seen.sources).toContain(FRED);
    expect(seen.sources).toContain(SEEDED.blsCpi.sourceName);
    expect(seen.gdp).toEqual({ externalId: 'GDP' });
    // The recession indicator, by a direct lookup rather than search.
    expect(seen.usrec).toEqual({ externalId: SEEDED.fredUsrec.externalId });
    expect(seen.cpi).toEqual({ externalId: SEEDED.blsCpi.externalId });
    expect(seen.searchTitles).toContain(SEEDED.fredGdp.title);

    // FRED's GDP page shows its seeded newest point.
    await readAsDevUser(page, token);
    await page.goto(`/series/${await seededSeriesId(request, SEEDED.fredGdp, token)}`);
    await expect(page.getByRole('heading', { level: 1 })).toHaveText(SEEDED.fredGdp.title);
    await expect.poll(() => latestShown(page)).toEqual(SEEDED.fredGdp.latestShown);

    // Its CSV carries FRED's notice and terms link (ECO-767).
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      page.getByRole('button', { name: 'Download CSV' }).click(),
    ]);
    const raw = readFileSync(await download.path(), 'utf8');
    const text = raw.charCodeAt(0) === 0xfeff ? raw.slice(1) : raw;
    expect(splitSeriesCsv(text.trim().split('\r\n')).notice).toEqual([
      'Notice,This product uses the FRED® API but is not endorsed or certified by the Federal Reserve Bank of St. Louis.',
      'Terms of use,https://fred.stlouisfed.org/docs/api/terms_of_use.html',
    ]);
  });
});
