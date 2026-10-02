// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

import { DEPLOYED } from '../env';
import { SEEDED } from '../fixtures';
import { expectWithinCadence } from '../seriesPage';

// Live QA: the dashboard's featured-indicator cards show the real latest value and period for
// the series they link to, not placeholder figures.
test.describe('dashboard cards', () => {
  test('the GDP card shows the seeded series’ real latest value and period', async ({ page }) => {
    await page.goto('/');

    // The card is an article named for the series it links to.
    const card = page.getByRole('article', { name: SEEDED.fredGdp.title });
    await expect(card).toBeVisible();

    await expect(card.getByText(SEEDED.fredGdp.units)).toBeVisible();
    if (DEPLOYED) {
      // Live GDP moves with every release: the period must be recent for a quarterly series.
      const shown = (await card.textContent())?.match(/[A-Z][a-z]{2} \d{1,2}, \d{4}/)?.[0];
      expect(shown, 'a period on the GDP card').toBeDefined();
      expectWithinCadence(shown!, SEEDED.fredGdp.liveMaxAgeDays, 'dashboard GDP');
    } else {
      // e2e-seed.json's FRED GDP fixture's latest non-missing observation is 32101.6 (billions of
      // dollars) for 2026-04-01, not a hardcoded placeholder.
      await expect(card.getByText('32,101.6')).toBeVisible();
      await expect(card.getByText('Apr 1, 2026')).toBeVisible();
    }
    await expect(card).toContainText('FRED');
    await expect(card).not.toContainText('BEA');
    await expect(card.getByRole('link', { name: SEEDED.fredGdp.title })).toHaveAttribute(
      'href',
      /^\/series\//
    );
    await expect(page.getByText(/\$27\.36T/)).toHaveCount(0);
  });
});
