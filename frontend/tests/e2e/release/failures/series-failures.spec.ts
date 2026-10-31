// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test, type Page } from '@playwright/test';

import { seededSeriesId } from '../auth/helpers';
import { DEPLOYED } from '../env';
import { SEEDED } from '../fixtures';
import { applyTransformation, latestShown, recentRows } from '../seriesPage';
import { failGraphQL, QUERY_ERROR_TIMEOUT } from './faults';

// ECO-264's sampled browser checks: the real backend, with chosen GraphQL requests failed in the
// browser (faults.ts), so these prove the page's own wiring of error, empty and recovery states.
// The permutations (every error shape, every state) are the component tests'
// (src/pages/__tests__/SeriesDetail.test.tsx, SeriesExplorer.test.tsx). Live QA: the same
// specs run against a deployed build; only the fixture values are skipped there.

// A public series: BLS CPI.
const series = SEEDED.blsCpi;
// Its quarter-over-quarter change, which its five months can compute (year-over-year can't).
const quarterly = series.transformations[1];

const searchBox = (page: Page) =>
  page.getByPlaceholder('Search economic series (e.g., GDP, unemployment, inflation)');
const searchButton = (page: Page) => page.getByRole('button', { name: 'Search', exact: true });
const resultCards = (page: Page) => page.locator('.MuiCard-root');

test.describe('search failures', () => {
  test('a failed search says so, and searching again recovers', async ({ page }) => {
    test.slow();
    const fault = await failGraphQL(page, 'SearchSeries');
    await page.goto('/explore');
    await searchBox(page).fill(series.title);
    await searchButton(page).click();

    await expect(page.getByRole('alert').filter({ hasText: 'Search failed' })).toBeVisible({
      timeout: QUERY_ERROR_TIMEOUT,
    });
    // An error, not an empty result: no "No series found", no result count, no cards.
    await expect(page.getByText('No series found')).toHaveCount(0);
    await expect(page.getByText(/^Found \d+ results?$/)).toHaveCount(0);
    await expect(resultCards(page)).toHaveCount(0);
    expect(fault.failed()).toBeGreaterThan(0);

    fault.heal();
    await searchButton(page).click();
    await expect(page.getByText(series.title, { exact: true }).first()).toBeVisible();
    await expect(page.getByRole('alert').filter({ hasText: 'Search failed' })).toHaveCount(0);
  });

  test('a search that matches nothing is an empty result, not an error', async ({ page }) => {
    await page.goto('/explore');
    await searchBox(page).fill('zzqxv no such series');
    await searchButton(page).click();

    await expect(page.getByRole('heading', { name: 'No series found' })).toBeVisible();
    await expect(page.getByRole('alert').filter({ hasText: /failed|Could not/ })).toHaveCount(0);
    await expect(resultCards(page)).toHaveCount(0);
  });

  test('a failed source listing offers Retry, which recovers', async ({ page }) => {
    test.slow();
    const fault = await failGraphQL(page, 'GetSeriesList');
    await page.goto('/sources');
    await page
      .getByRole('region', { name: series.sourceName })
      .getByRole('link', { name: 'Browse Series' })
      .click();
    const alert = page.getByRole('alert').filter({ hasText: 'Could not list series' });
    await expect(alert).toBeVisible({ timeout: QUERY_ERROR_TIMEOUT });

    fault.heal();
    await alert.getByRole('button', { name: 'Retry' }).click();
    await expect(page.getByText(series.title, { exact: true })).toBeVisible();
    await expect(alert).toHaveCount(0);
    expect(fault.failed()).toBeGreaterThan(0);
  });
});

test.describe('series page failures', () => {
  test.slow();
  let seriesPath: string;

  test.beforeAll(async ({ request }) => {
    seriesPath = `/series/${await seededSeriesId(request, series)}`;
  });

  test('a series that fails to load offers Retry, which recovers', async ({ page }) => {
    const fault = await failGraphQL(page, 'GetSeriesDetail', 'network');
    await page.goto(seriesPath);

    const alert = page.getByRole('alert').filter({ hasText: 'Could not load this series' });
    await expect(alert).toBeVisible({ timeout: QUERY_ERROR_TIMEOUT });
    // A failure, not "Series not found".
    await expect(page.getByText(/Series not found/)).toHaveCount(0);

    fault.heal();
    await alert.getByRole('button', { name: 'Retry' }).click();
    await expect(page.getByRole('heading', { name: series.title })).toBeVisible();
    await expect(alert).toHaveCount(0);
  });

  test('observations that fail to load show no values, and Retry recovers', async ({ page }) => {
    const fault = await failGraphQL(page, 'GetSeriesData', 'graphql-error');
    await page.goto(seriesPath);
    await expect(page.getByRole('heading', { name: series.title })).toBeVisible();

    const alert = page
      .getByRole('alert')
      .filter({ hasText: 'Could not load observations for this series' });
    await expect(alert).toBeVisible({ timeout: QUERY_ERROR_TIMEOUT });
    // No stand-in values and no "no observations yet" message: the read failed, it wasn't empty.
    await expect(page.getByRole('table', { name: 'Recent observations' })).toHaveCount(0);
    await expect(page.getByText('No observations for this series yet.')).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Download CSV' })).toHaveCount(0);

    fault.heal();
    await alert.getByRole('button', { name: 'Retry' }).click();
    await expect(recentRows(page).first()).toBeVisible();
    if (!DEPLOYED) expect(await latestShown(page)).toEqual(series.latestShown);
  });

  test('a failed transformation never labels the levels as transformed', async ({ page }) => {
    const levelsHeader = page.getByRole('columnheader', { name: series.units, exact: true });
    const qoqHeader = page.getByRole('columnheader', { name: quarterly.description, exact: true });
    await page.goto(seriesPath);
    await expect(levelsHeader).toBeVisible();
    const levels = await latestShown(page);

    const fault = await failGraphQL(
      page,
      'GetSeriesData',
      'http-503',
      variables => variables.transformation === 'QUARTER_OVER_QUARTER'
    );
    await page.getByRole('combobox', { name: 'Transformation' }).click();
    await page.getByRole('option', { name: quarterly.label, exact: true }).click();

    // Wait until the switch has happened: Quarter-over-Quarter is selected and its request has failed
    // at least once. The app then retries for about 7 seconds (1, 2 and 4 s backoff) before
    // giving up, keeping the previous data behind a "Loading transformation" bar.
    await expect(page.getByRole('combobox', { name: 'Transformation' })).toHaveText(
      quarterly.label
    );
    await expect.poll(() => fault.failed()).toBeGreaterThan(0);

    // While it is retried, the levels stay on screen labelled as levels.
    const alert = page
      .getByRole('alert')
      .filter({ hasText: 'Could not load observations for this series' });
    await expect(page.getByRole('progressbar', { name: 'Loading transformation' })).toBeVisible();
    await expect(levelsHeader).toBeVisible();
    await expect(qoqHeader).toHaveCount(0);

    // Once it fails, no values are shown at all, under either label.
    await expect(alert).toBeVisible({ timeout: QUERY_ERROR_TIMEOUT });
    await expect(page.getByRole('table', { name: 'Recent observations' })).toHaveCount(0);
    await expect(qoqHeader).toHaveCount(0);

    // "Show levels" goes back to the levels, still failing nothing else.
    await alert.getByRole('button', { name: 'Show levels' }).click();
    await expect(levelsHeader).toBeVisible();
    expect(await latestShown(page)).toEqual(levels);

    // And once the backend answers, the transformation shows its own values.
    fault.heal();
    await applyTransformation(page, quarterly.label);
    await expect(qoqHeader).toBeVisible();
    await expect(levelsHeader).toHaveCount(0);
    if (!DEPLOYED) {
      await expect
        .poll(() => latestShown(page))
        .toEqual(quarterly.latestShown);
    }
  });
});
