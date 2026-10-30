// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';

import { expect, test } from '@playwright/test';

import { asShown, latestShown, parseShown, recentRows } from '../../e2e/release/seriesPage';

// The QA scenario suite's anonymous browsing (scripts/qa/scenarios/anonymous-browser.sh), against
// whatever catalog the target serves: nothing here assumes a fixture value. Signed out, read-only.

const QUERY = process.env.QA_SEARCH_QUERY ?? 'GDP';

// A data row of the series CSV: an ISO date and a plain number, or nothing for a missing value.
const CSV_ROW = /^\d{4}-\d{2}-\d{2},(-?\d+(\.\d+)?([eE][+-]?\d+)?)?$/;

test.describe('anonymous browsing', () => {
  test('the home page loads without errors', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    const response = await page.goto('/');
    expect(response?.ok()).toBe(true);
    await expect(page.getByRole('main')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('search, open a series, its chart and its CSV', async ({ page }) => {
    await test.step('search finds series', async () => {
      await page.goto('/explore');
      // The page's own search box, not the header's ("Search economic series...").
      await page.getByPlaceholder('Search economic series (e.g.', { exact: false }).fill(QUERY);
      await page.getByRole('button', { name: 'Search', exact: true }).click();
      await expect(page.getByRole('button', { name: 'View Details' }).first()).toBeVisible();
    });

    await test.step('a result opens its series page', async () => {
      await page.getByRole('button', { name: 'View Details' }).first().click();
      await expect(page).toHaveURL(/\/series\/[0-9a-f-]{36}$/);
    });

    const title = (await page.getByRole('heading', { level: 1 }).innerText()).trim();

    await test.step('the series page charts it', async () => {
      await expect(page.getByRole('img', { name: `Line chart of ${title}` })).toBeVisible();
      await expect(recentRows(page).first()).toBeVisible();
    });

    await test.step('the CSV holds the points the page shows', async () => {
      const [download] = await Promise.all([
        page.waitForEvent('download'),
        page.getByRole('button', { name: 'Download CSV' }).click(),
      ]);
      expect(download.suggestedFilename()).toMatch(/\.csv$/);
      const raw = readFileSync(await download.path(), 'utf8');
      const text = raw.charCodeAt(0) === 0xfeff ? raw.slice(1) : raw;
      const lines = text.trim().split('\r\n');
      expect(lines[0]).toMatch(/^Title,/);
      const header = lines.indexOf('date,value');
      expect(header, 'the CSV has a date,value header').toBeGreaterThan(0);
      const rows = lines.slice(header + 1);
      expect(rows.length).toBeGreaterThan(0);
      for (const row of rows) expect(row).toMatch(CSV_ROW);

      // The newest non-missing row is the page's newest observation, date and value. The page
      // rounds the value for display, so they agree to within half its last shown digit.
      const [date, value] = (rows.filter(row => !row.endsWith(',')).at(-1) ?? ',').split(',');
      const shown = await latestShown(page);
      expect(asShown(date, shown.date)).toBe(shown.date);
      const decimals = shown.value.split('.')[1]?.length ?? 0;
      expect(Math.abs(Number(value) - parseShown(shown.value))).toBeLessThanOrEqual(
        0.5 * 10 ** -decimals + 1e-9
      );
    });
  });

  test('the world map colors countries and lists their values', async ({ page }) => {
    await page.goto('/global');
    await expect(page.getByTestId('map-legend')).toContainText(/\d+ countr(y|ies)/, {
      timeout: 30_000,
    });
    await expect(page.locator('svg path.country[data-has-data="true"]').first()).toBeAttached();
    await page.getByRole('button', { name: /^Values as a table/ }).click();
    const rows = page.getByRole('table', { name: 'Values by country' }).getByRole('row');
    // A header row and at least one country.
    await expect(rows.nth(1)).toBeVisible();
  });
});
