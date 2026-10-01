// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test, type Page } from '@playwright/test';

import { SEEDED_WDI } from '../fixtures';

// Live QA: the world map shows a value for every country World Development Indicators are
// seeded for, on every indicator, with a working tooltip, and loads its outline from the app's
// own bundle rather than a CDN. Checks MAP-7's acceptance: every seeded country has a value for
// each indicator (none renders as "no data"), tooltips show the value and its date, and no
// network request for the outline leaves the app. Kosovo, which MAP-6 always shows as no data
// (no ISO numeric code), isn't part of this fixture's countries, so there's nothing to exclude
// here; see world-map.md's MAP-6/MAP-7 rows for why it's excluded in general.

/** ISO 3166-1 numeric code of each seeded country, world-atlas's feature id. */
const ISO_NUMERIC: Record<keyof typeof SEEDED_WDI.countries, number> = {
  USA: 840,
  CHN: 156,
  DEU: 276,
  JPN: 392,
  IND: 356,
  BRA: 76,
};

/**
 * The map's value formatting: grouped thousands, at most two decimals, no trailing zeros.
 * @param value - The seeded fixture's numeric value.
 * @returns The formatted value, e.g. `82,769.4`.
 */
const formatValue = (value: number) => value.toLocaleString('en-US', { maximumFractionDigits: 2 });

/**
 * The map's date formatting for an annual series: just the year.
 * @param date - `YYYY-MM-DD`.
 * @returns The year, e.g. `2023`.
 */
const year = (date: string) => date.slice(0, 4);

const countryPath = (page: Page, isoNumeric: number) =>
  page.locator(`svg path.country[data-iso-numeric="${isoNumeric}"]`);

test.describe('World map release spec', () => {
  test('every seeded country has a value for every indicator, with a working tooltip', async ({
    page,
    baseURL,
  }) => {
    const appOrigin = new URL(baseURL ?? 'http://localhost:3000').origin;
    const externalOutlineRequests: string[] = [];
    page.on('request', request => {
      const url = new URL(request.url());
      if (
        url.hostname === 'cdn.jsdelivr.net' ||
        (url.origin !== appOrigin && /world-atlas|countries-\d+m/.test(url.pathname))
      ) {
        externalOutlineRequests.push(url.href);
      }
    });

    await page.goto('/global');

    const countryCodes = Object.keys(SEEDED_WDI.countries) as Array<
      keyof typeof SEEDED_WDI.countries
    >;
    const legend = page.getByTestId('map-legend');

    for (const [i, indicator] of SEEDED_WDI.indicators.entries()) {
      if (i > 0) {
        await page.getByRole('combobox', { name: 'Indicator' }).click();
        await page.getByRole('option', { name: indicator.name }).click();
      }
      await expect(legend).toContainText(`Unit: ${indicator.unit}`);
      await expect(legend).toContainText(
        countryCodes.length === 1 ? '1 country' : `${countryCodes.length} countries`
      );

      // Every seeded country shows a value on the map; none is left as "no data".
      for (const code of countryCodes) {
        await expect(countryPath(page, ISO_NUMERIC[code])).toHaveAttribute(
          'data-has-data',
          'true'
        );
      }

      // The values table reaches every seeded country with its exact value and date. Exact cell
      // text, not a substring: a date cell that fell back to the full `Jan 1, 2023` format (the
      // app's behavior when the frequency lookup fails) would still contain "2023" and pass a
      // looser check, masking that regression.
      const tableToggle = page.getByRole('button', {
        name: `Values as a table (${countryCodes.length} countries)`,
      });
      await tableToggle.click();
      const table = page.getByRole('table', { name: 'Values by country' });
      for (const code of countryCodes) {
        const latest = indicator.latest[code];
        const row = table.getByRole('row').filter({ hasText: SEEDED_WDI.countries[code] });
        await expect(row.getByRole('cell').nth(1)).toHaveText(formatValue(latest.value));
        await expect(row.getByRole('cell').nth(2)).toHaveText(year(latest.date));
      }
      await tableToggle.click();

      // The tooltip for one seeded country shows its value, unit and date. Germany, not the
      // first seeded country (the United States): the US outline, Alaska included, spans most of
      // the map's width, so its bounding-box centre (and Japan's, the same way) falls outside its
      // own shape and the hover would land on France instead.
      const sample = 'DEU' as const;
      const sampleLatest = indicator.latest[sample];
      await countryPath(page, ISO_NUMERIC[sample]).hover();
      const tooltip = page.getByTestId('country-tooltip');
      await expect(tooltip).toContainText(SEEDED_WDI.countries[sample]);
      await expect(tooltip).toContainText(`${formatValue(sampleLatest.value)} ${indicator.unit}`);
      await expect(tooltip).toContainText(year(sampleLatest.date));
    }

    expect(externalOutlineRequests).toEqual([]);
  });
});
