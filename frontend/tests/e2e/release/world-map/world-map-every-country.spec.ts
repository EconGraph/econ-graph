// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';

import { expect, test, type Page } from '@playwright/test';

import { DEPLOYED } from '../env';
import { SEEDED_WDI } from '../fixtures';

// ECO-262: every country the release stack seeds World Development Indicators for, on every
// seeded indicator, checked against the recorded World Bank responses themselves rather than
// against fixtures.ts: its outline is colored, its tooltip and its table row show the newest
// non-missing value with the indicator's unit and year, and every other outline (countries the
// recorded responses hold but the seed skipped, and countries with no response at all) is "no
// data". Repeated after each indicator switch, so a stale value or a mapping left over from the
// previous indicator fails. world-map.spec.ts keeps the core release checks (MAP-7); aggregate
// exclusion on the map is world-map-mocked.spec.ts's, since the seed loads no aggregate.
// Fixture mode only: a deployed catalog has every country, with values this can't predict.

const FIXTURES = '../backend/crates/econ-graph-crawler/tests/fixtures';

interface WorldBankRow {
  indicator: { id: string; value: string };
  country: { id: string; value: string };
  countryiso3code: string;
  date: string;
  value: number | null;
}

interface SeedEntry {
  source: string;
  external_id: string;
  routes: { path: string; fixture: string }[];
}

/**
 * ISO 3166-1 numeric code (world-atlas's feature id) of each country the seed may load. A
 * country added to the seed without one here fails the spec, rather than going unchecked.
 */
const ISO_NUMERIC: Record<string, number> = {
  USA: 840,
  CHN: 156,
  DEU: 276,
  JPN: 392,
  IND: 356,
  BRA: 76,
  FRA: 250,
  GBR: 826,
};

/** A country's expected map entry, from the recorded response. */
interface Expected {
  iso3: string;
  name: string;
  value: number;
  year: string;
}

/**
 * Per seeded indicator, what the map should show: read from e2e-seed.json (which series are
 * seeded, and from which recorded response) and the recorded World Bank responses.
 * @returns The indicators in seed order, each with its seeded countries and the recorded
 *   response's other countries.
 */
function expectedFromFixtures() {
  const seed = JSON.parse(readFileSync(`${FIXTURES}/e2e-seed.json`, 'utf8')) as {
    series: SeedEntry[];
  };
  const indicators = new Map<string, { file: string; iso3: Set<string> }>();
  for (const entry of seed.series) {
    if (entry.source !== 'WORLD_BANK') continue;
    // `wdi/{indicator}.{ISO3}`; the indicator code itself holds dots.
    const match = /^wdi\/(.+)\.([A-Z]{3})$/.exec(entry.external_id);
    if (!match) throw new Error(`unexpected WDI id ${entry.external_id}`);
    const [, code, iso3] = match;
    const file = entry.routes.find(r => r.path.endsWith(`/indicator/${code}`))?.fixture;
    if (!file) throw new Error(`no recorded response for ${entry.external_id}`);
    const indicator = indicators.get(code) ?? { file, iso3: new Set<string>() };
    indicator.iso3.add(iso3);
    indicators.set(code, indicator);
  }

  return [...indicators].map(([code, { file, iso3 }]) => {
    const [, rows] = JSON.parse(readFileSync(`${FIXTURES}/${file}`, 'utf8')) as [
      unknown,
      WorldBankRow[],
    ];
    const ofIndicator = rows.filter(r => r.indicator.id === code);
    const newest = (country: string): Expected => {
      const withValue = ofIndicator
        .filter(r => r.countryiso3code === country && r.value !== null)
        .sort((a, b) => b.date.localeCompare(a.date));
      if (withValue.length === 0) throw new Error(`${code}: no value for ${country}`);
      const [row] = withValue;
      return { iso3: country, name: row.country.value, value: row.value as number, year: row.date };
    };
    const unseeded = [...new Set(ofIndicator.map(r => r.countryiso3code))].filter(
      c => !iso3.has(c) && c in ISO_NUMERIC
    );
    return {
      code,
      name: ofIndicator[0].indicator.value,
      countries: [...iso3].sort().map(newest),
      unseeded,
    };
  });
}

/**
 * The map's value formatting: grouped thousands, at most two decimals, no trailing zeros.
 * @param value - The recorded value.
 * @returns The formatted value, e.g. `82,769.4`.
 */
const formatValue = (value: number) => value.toLocaleString('en-US', { maximumFractionDigits: 2 });

const countryPath = (page: Page, isoNumeric: number) =>
  page.locator(`svg path.country[data-iso-numeric="${isoNumeric}"]`);

const numericOf = (iso3: string) => {
  const numeric = ISO_NUMERIC[iso3];
  if (numeric === undefined) throw new Error(`no ISO numeric code for ${iso3}; add it`);
  return numeric;
};

test.describe('World map, every seeded country', () => {
  test.skip(DEPLOYED, 'compares with the recorded fixtures, which a deployed build does not serve');

  test('each country shows its recorded value, unit and year on every indicator', async ({
    page,
  }) => {
    test.slow();
    const indicators = expectedFromFixtures();
    expect(indicators.map(i => i.code)).toEqual(SEEDED_WDI.indicators.map(i => i.code));

    await page.goto('/global');
    const legend = page.getByTestId('map-legend');
    const tooltip = page.getByTestId('country-tooltip');

    for (const [i, indicator] of indicators.entries()) {
      // The unit is the series' own (World Bank responses carry none), as fixtures.ts records it.
      const unit = SEEDED_WDI.indicators[i].unit;
      if (i > 0) {
        await page.getByRole('combobox', { name: 'Indicator' }).click();
        await page.getByRole('option', { name: indicator.name, exact: true }).click();
      }
      await expect(page.getByRole('combobox', { name: 'Indicator' })).toHaveText(indicator.name);
      await expect(legend).toContainText(`Unit: ${unit}`);
      await expect(legend).toContainText(`${indicator.countries.length} countries with data.`);

      // Exactly the seeded countries are colored, each on its own outline.
      const colored = page.locator('svg path.country[data-has-data="true"]');
      await expect(colored).toHaveCount(indicator.countries.length);
      const coloredIds = await colored.evaluateAll(paths =>
        paths.map(p => Number(p.getAttribute('data-iso-numeric'))).sort((a, b) => a - b)
      );
      expect(coloredIds, indicator.code).toEqual(
        indicator.countries.map(c => numericOf(c.iso3)).sort((a, b) => a - b)
      );

      // Park the real pointer off the map (picking the indicator leaves it over a country), so
      // the only tooltip is the one each dispatched event below opens.
      await page.mouse.move(0, 0);
      await expect(tooltip).toHaveCount(0);
      for (const country of indicator.countries) {
        // Dispatched rather than hovered: an outline's bounding-box centre can fall on another
        // country (the United States' does, Alaska included).
        await countryPath(page, numericOf(country.iso3)).dispatchEvent('mouseover');
        await expect(tooltip, `${indicator.code} ${country.iso3}`).toContainText(country.name);
        await expect(tooltip).toContainText(`${formatValue(country.value)} ${unit}`);
        await expect(tooltip).toContainText(country.year);
        await countryPath(page, numericOf(country.iso3)).dispatchEvent('mouseout');
        await expect(tooltip).toHaveCount(0);
      }

      // Countries the recorded response has values for but the seed skipped are "no data".
      for (const iso3 of indicator.unseeded) {
        const outline = countryPath(page, numericOf(iso3));
        await expect(outline, `${indicator.code} ${iso3}`).toHaveAttribute(
          'data-has-data',
          'false'
        );
        await outline.dispatchEvent('mouseover');
        await expect(tooltip).toContainText('No data');
        await outline.dispatchEvent('mouseout');
      }

      // The table lists exactly the seeded countries, with the same values and years.
      const toggle = page.getByRole('button', {
        name: `Values as a table (${indicator.countries.length} countries)`,
      });
      await toggle.click();
      const rows = page.getByRole('table', { name: 'Values by country' }).locator('tbody tr');
      await expect(rows).toHaveCount(indicator.countries.length);
      for (const country of indicator.countries) {
        const row = rows.filter({ hasText: country.name });
        await expect(row).toHaveCount(1);
        await expect(row.getByRole('cell').nth(1)).toHaveText(formatValue(country.value));
        await expect(row.getByRole('cell').nth(2)).toHaveText(country.year);
      }
      await toggle.click();
    }
  });
});
