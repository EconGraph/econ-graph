// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

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
    // Through the frontend's own origin, so this also checks the /graphql proxy.
    const response = await request.post('/graphql', {
      data: {
        query: '{ seriesList(pagination: { first: 50 }) { nodes { externalId title } } }',
      },
    });
    expect(response.ok()).toBe(true);
    const body = await response.json();
    expect(body.errors).toBeUndefined();
    const externalIds = body.data.seriesList.nodes.map((n: { externalId: string }) => n.externalId);
    for (const s of Object.values(SEEDED)) {
      expect(externalIds).toContain(s.externalId);
    }
  });

  // Search is broken on main at both ends: the backend can't decode its own result rows
  // (fixed by #165) and the page sends a query the schema rejects (fixed by series-ui UI-9).
  // Change `test.fixme` to `test` when both have merged.
  test.fixme('explore search finds a seeded series', async ({ page }) => {
    await page.goto('/explore');
    await page.getByPlaceholder(/Search economic series/).fill('Gross Domestic');
    await expect(page.getByText(SEEDED.fredGdp.title).first()).toBeVisible();
  });
});
