// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

import { seededSeriesId } from './auth/helpers';
import { DEPLOYED } from './env';
import { SEEDED } from './fixtures';

test.describe('release smoke', () => {
  test('home page loads', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', err => errors.push(err.message));

    const response = await page.goto('/');
    expect(response?.ok()).toBe(true);
    await expect(page.getByRole('main')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('the frontend reaches the seeded backend', async ({ request }) => {
    // Through the frontend's own origin, so this also checks the /graphql proxy. By search rather
    // than the plain series list, so it also holds against a deployed build's full catalog.
    // FRED's series are admin-only: catalog-access.spec.ts checks them.
    for (const s of Object.values(SEEDED)) {
      if (s.source === 'FRED') continue;
      await seededSeriesId(request, s);
    }
  });

  test('the backend is a release-profile build', async ({ request }) => {
    // The `mcp` build flag is off in release, so nothing answers at /mcp (config/flags). Straight
    // to the backend: the frontend doesn't proxy /mcp. A deployed run has no local backend.
    test.skip(DEPLOYED, 'checks the local backend scripts/release-e2e.sh built');
    const backendOrigin = `http://localhost:${Number(process.env.RELEASE_BACKEND_PORT ?? 18080)}`;
    const response = await request.post(`${backendOrigin}/mcp`, {
      data: { jsonrpc: '2.0', id: 1, method: 'tools/list' },
    });
    expect(response.status()).toBe(404);
  });

  test('every page footer links to the Terms of Use, and the link resolves', async ({
    page,
    request,
  }) => {
    // ECO-881: the terms pass the public sources' conditions on, so every page must reach them.
    const pages = [
      '/',
      '/explore',
      `/series/${await seededSeriesId(request, SEEDED.fhfaHpi)}`,
      '/sources',
      '/global',
      '/about',
      '/privacy',
      '/terms',
      '/this-page-does-not-exist',
    ];
    for (const path of pages) {
      await page.goto(path);
      const terms = page.getByRole('contentinfo').getByRole('link', { name: 'Terms of Use' });
      await expect(terms, `footer Terms of Use link on ${path}`).toBeVisible();
      await expect(terms).toHaveAttribute('href', '/terms');
    }

    await page.goto('/');
    await page.getByRole('contentinfo').getByRole('link', { name: 'Terms of Use' }).click();
    await expect(page).toHaveURL(/\/terms$/);
    await expect(page.getByRole('heading', { level: 1, name: 'Terms of Use' })).toBeVisible();

    // Loaded directly too, as a link shared from outside the app would be.
    const response = await page.goto('/terms');
    expect(response?.ok()).toBe(true);
    await expect(page.getByRole('heading', { level: 1, name: 'Terms of Use' })).toBeVisible();
  });

  test('explore search finds a seeded series', async ({ page }) => {
    await page.goto('/explore');
    // The page's own search box, not the header's ("Search economic series...").
    await page.getByPlaceholder('Search economic series (e.g.', { exact: false }).fill('Gross Domestic');
    await page.getByRole('button', { name: 'Search', exact: true }).click();
    await expect(page.getByText(SEEDED.beaGdp.title, { exact: true }).first()).toBeVisible();
  });
});
