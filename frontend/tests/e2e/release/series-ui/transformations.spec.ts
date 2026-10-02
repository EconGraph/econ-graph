// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test, type Page } from '@playwright/test';

import { DEPLOYED } from '../env';
import { SEEDED } from '../fixtures';
import { seededSeriesId } from '../auth/helpers';

/**
 * The Transformation select on a series page. It has no labelId wired to its InputLabel
 * (SeriesChart.tsx), so it has no accessible name; find it by its containing FormControl's
 * visible label text instead.
 * @param page - The test's page, already on a series page.
 * @returns The select's combobox locator.
 */
function transformationSelect(page: Page) {
  return page.locator('.MuiFormControl-root', { hasText: 'Transformation' }).getByRole('combobox');
}

/**
 * The chip beside the chart naming the applied transformation (empty/absent for NONE).
 * @param page - The test's page.
 * @param label - The transformation's description text, as shown in the chip and the column
 *   header.
 * @returns The chip locator.
 */
function transformationChip(page: Page, label: string) {
  return page.locator('.MuiChip-root', { hasText: label });
}

/**
 * Picks a transformation. Previous data stays on screen while the new one loads, so this also
 * waits out the loading indicator when one appears, rather than asserting on stale data.
 * @param page - The test's page.
 * @param option - The option's visible text in the select.
 */
async function applyTransformation(page: Page, option: string) {
  await transformationSelect(page).click();
  await page.getByRole('option', { name: option, exact: true }).click();
  await expect(page.getByRole('progressbar', { name: 'Loading transformation' })).toHaveCount(0, {
    timeout: 15_000,
  });
}

/**
 * The Recent Data table's newest row (SeriesDetail.tsx shows the last 10 points, newest first).
 * @param page - The test's page.
 * @returns Its date and value cells' text.
 */
async function latestShown(page: Page): Promise<{ date: string; value: string }> {
  const cells = page
    .getByRole('table', { name: 'Recent observations' })
    .locator('tbody tr')
    .first()
    .getByRole('cell');
  const [date, value] = await Promise.all([cells.nth(0).innerText(), cells.nth(1).innerText()]);
  return { date, value };
}

// Live QA: apply each transformation on a series page and check the chart and the value column
// both switch to it. The chart itself is drawn on a canvas (not queryable), so the "Recent
// Data" table's value column header, which the page derives from the same transformation,
// stands in for it.
test.describe('series transformations', () => {
  // GDP is quarterly, so year-over-year (a year back), quarter-over-quarter and change-since-
  // first-observation all have data; month-over-month never will for a quarterly series (see
  // the CPI test below).
  test('quarterly transformations update the value column and the chip', async ({
    page,
    request,
  }) => {
    const seriesId = await seededSeriesId(request, SEEDED.fredGdp);
    await page.goto(`/series/${seriesId}`);
    await expect(transformationSelect(page)).toBeVisible();

    const cases = [
      {
        option: 'Year-over-Year',
        columnHeader: 'Year-over-Year % Change',
        expected: SEEDED.fredGdp.transformations[0].latestShown,
      },
      {
        option: 'Quarter-over-Quarter',
        columnHeader: 'Quarter-over-Quarter % Change',
        expected: SEEDED.fredGdp.transformations[1].latestShown,
      },
      {
        option: 'Change since first available value',
        columnHeader: '% Change since First Available Value',
        expected: SEEDED.fredGdp.transformations[3].latestShown,
      },
    ];

    for (const { option, columnHeader, expected } of cases) {
      await applyTransformation(page, option);
      await expect(page.getByRole('columnheader', { name: columnHeader, exact: true })).toBeVisible();
      await expect(transformationChip(page, columnHeader)).toBeVisible();
      // Fixture mode only: deployed runs compare against live data, not recorded fixtures.
      if (!DEPLOYED) await expect.poll(() => latestShown(page)).toEqual(expected);
    }

    // Back to none: the chip disappears and the column header reads the series' own units.
    await applyTransformation(page, 'None');
    await expect(page.getByRole('columnheader', { name: 'Billions of Dollars', exact: true })).toBeVisible();
    if (!DEPLOYED) {
      await expect
        .poll(() => latestShown(page))
        .toEqual(SEEDED.fredGdp.transformations[4].latestShown);
    }
    for (const { columnHeader } of cases) {
      await expect(page.getByRole('columnheader', { name: columnHeader, exact: true })).toHaveCount(
        0
      );
      await expect(transformationChip(page, columnHeader)).toHaveCount(0);
    }
  });

  // CPI is monthly, the one seeded series month-over-month can actually be computed for.
  test('month-over-month updates the value column and the chip', async ({ page, request }) => {
    const seriesId = await seededSeriesId(request, SEEDED.blsCpi);
    await page.goto(`/series/${seriesId}`);
    await expect(transformationSelect(page)).toBeVisible();

    await applyTransformation(page, 'Month-over-Month');
    await expect(
      page.getByRole('columnheader', { name: 'Month-over-Month % Change', exact: true })
    ).toBeVisible();
    await expect(transformationChip(page, 'Month-over-Month % Change')).toBeVisible();
    if (!DEPLOYED) {
      await expect
        .poll(() => latestShown(page))
        .toEqual(SEEDED.blsCpi.transformations[0].latestShown);
    }
  });

  // The schema has LOG_DIFFERENCE, but the backend computes ratio - 1 rather than a log, so the
  // frontend doesn't offer it (frontend/src/utils/transformations.ts). Un-fixme once the backend
  // fix (#230) and the frontend option it unblocks (#245) merge.
  test.fixme(
    'log difference is offered and computes an actual logarithm (blocked on #230, #245)',
    async ({ page, request }) => {
      const seriesId = await seededSeriesId(request, SEEDED.fredGdp);
      await page.goto(`/series/${seriesId}`);

      await applyTransformation(page, 'Log difference');
      await expect(page.getByRole('columnheader', { name: 'Log difference', exact: true })).toBeVisible();
    }
  );
});
