// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

import { SEEDED } from '../fixtures';

// Live QA: search for a real series by name and open its page from the results.
test.describe('search and open a series', () => {
  test('finds a seeded series by title and opens it', async ({ page }) => {
    await page.goto('/explore');

    // Exact match: the header also has a search box, whose placeholder ends in "..." instead.
    await page
      .getByPlaceholder('Search economic series (e.g., GDP, unemployment, inflation)')
      .fill('Gross Domestic Product');
    // Exact match: substring matching on "Search" also picks up the sidebar nav item and the
    // search box's own "Clear search text" button.
    await page.getByRole('button', { name: 'Search', exact: true }).click();

    const resultText = page.getByText(SEEDED.fredGdp.title, { exact: true });
    await expect(resultText).toBeVisible();

    // The result cards are MUI Cards with no other stable role/testid; scope by the
    // library's own class so the click lands on this series' card, not another result's.
    const card = page.locator('.MuiCard-root', { has: resultText });
    await card.getByRole('button', { name: 'View Details' }).click();

    await expect(page).toHaveURL(/\/series\/[^/]+$/);
    await expect(page.getByRole('heading', { name: SEEDED.fredGdp.title })).toBeVisible();
  });
});
