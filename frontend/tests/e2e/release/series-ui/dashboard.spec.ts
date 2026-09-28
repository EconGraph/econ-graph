// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

import { SEEDED } from '../fixtures';

// Live QA: the dashboard's featured-indicator cards show the real latest value and period for
// the series they link to, not placeholder figures.
//
// As of this writing (src/pages/Dashboard.tsx), `featuredIndicators` hardcodes value/change/
// period/source for every card ('$27.36T', 'Q3 2024', source 'BEA' for a series that is
// actually seeded from FRED, etc.) and only `seriesId` comes from a real search. Un-fixme once
// the cards are wired to the series' own latest observation (#212, #200).
test.describe('dashboard cards', () => {
  test.fixme(
    'the GDP card shows the seeded series’ real latest value and period (blocked on #212, #200)',
    async ({ page }) => {
      await page.goto('/');

      // The card's title is currently the hardcoded 'Real Gross Domestic Product', but #212/
      // #200 may switch it to the series' own title; match either so this doesn't fail for the
      // wrong reason once that changes.
      const card = page.locator('.MuiCard-root').filter({
        hasText: new RegExp(`Real Gross Domestic Product|${SEEDED.fredGdp.title}`),
      });
      await expect(card).toBeVisible();

      // e2e-seed.json's FRED GDP fixture's latest non-missing observation is 32101.6 (billions
      // of dollars) for 2026-04-01 (Q2 2026), not the hardcoded '$27.36T' / 'Q3 2024'.
      await expect(card.getByText(/\$32\.10T/)).toBeVisible();
      await expect(card.getByText(/Q2 2026/)).toBeVisible();
      await expect(card.getByText('FRED')).toBeVisible();
      await expect(card.getByText('BEA')).toHaveCount(0);
    }
  );
});
