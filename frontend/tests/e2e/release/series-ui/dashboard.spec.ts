// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

import { devUserToken, readAsDevUser } from '../auth/helpers';
import { DEPLOYED } from '../env';
import { SEEDED } from '../fixtures';

// The dashboard's featured-indicator cards (src/config/dashboard-series.json) are FRED series,
// which only admins can read. An admin's card shows the real latest value and period for the
// series it links to, not placeholder figures; a visitor's shows no FRED data.
test.describe('dashboard cards', () => {
  test('an admin’s GDP card shows the seeded series’ real latest value and period', async ({
    page,
    request,
  }) => {
    test.skip(DEPLOYED, 'FRED is admin-only and the deployed realm has no admin to read as');
    await readAsDevUser(page, await devUserToken(request, 'staff-admin'));
    await page.goto('/');

    // The card is an article named for the series it links to.
    const card = page.getByRole('article', { name: SEEDED.fredGdp.title });
    await expect(card).toBeVisible();

    await expect(card.getByText(SEEDED.fredGdp.units)).toBeVisible();
    // e2e-seed.json's FRED GDP fixture's latest non-missing observation is 32101.6 (billions of
    // dollars) for 2026-04-01, not a hardcoded placeholder.
    await expect(card.getByText('32,101.6')).toBeVisible();
    await expect(card.getByText('Apr 1, 2026')).toBeVisible();
    await expect(card).toContainText('FRED');
    await expect(card).not.toContainText('BEA');
    await expect(card.getByRole('link', { name: SEEDED.fredGdp.title })).toHaveAttribute(
      'href',
      /^\/series\//
    );
    await expect(page.getByText(/\$27\.36T/)).toHaveCount(0);
  });

  test('a visitor’s GDP card shows no FRED value and links nowhere', async ({ page }) => {
    await page.goto('/');

    const card = page.getByRole('article', { name: SEEDED.fredGdp.title });
    await expect(card.getByText('No data yet')).toBeVisible();
    await expect(card.getByText(SEEDED.fredGdp.units)).toHaveCount(0);
    await expect(card.getByRole('link')).toHaveCount(0);
  });
});
