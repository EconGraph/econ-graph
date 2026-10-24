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
    for (const s of Object.values(SEEDED)) {
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

  test('explore search finds a seeded series', async ({ page }) => {
    await page.goto('/explore');
    // The page's own search box, not the header's ("Search economic series...").
    await page.getByPlaceholder('Search economic series (e.g.', { exact: false }).fill('Gross Domestic');
    await page.getByRole('button', { name: 'Search', exact: true }).click();
    await expect(page.getByText(SEEDED.fredGdp.title).first()).toBeVisible();
  });
});
