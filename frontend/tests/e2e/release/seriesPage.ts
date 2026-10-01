// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';

import { expect, type Page } from '@playwright/test';

import type { SeededSeries } from './auth/helpers';

/**
 * Helpers for driving the explore page and a series page (src/pages/SeriesDetail.tsx), shared by
 * the journey and the six-source sweep so a page change needs one edit.
 */

/**
 * The rows of the series page's "Recent observations" table: its last 10 non-missing points,
 * newest first.
 * @param page - A series page.
 * @returns The rows' locator.
 */
export const recentRows = (page: Page) =>
  page.getByRole('table', { name: 'Recent observations' }).locator('tbody tr');

/**
 * The newest observation the series page lists: the first row of its "Recent observations" table.
 * @param page - A series page.
 * @returns The row's date and value cells, as shown.
 */
export async function latestShown(page: Page) {
  const cells = recentRows(page).first().getByRole('cell');
  return { date: await cells.nth(0).innerText(), value: await cells.nth(1).innerText() };
}

/**
 * Parses a number as the series page formats it (`32,101.60`).
 * @param shown - The cell text.
 * @returns The number.
 */
export const parseShown = (shown: string) => Number(shown.replace(/,/g, ''));

/**
 * The series page's displayed date ("Apr 1, 2026") as the CSV's `YYYY-MM-DD`.
 * @param shown - The date as `formatIsoDate` renders it.
 * @returns The ISO calendar date.
 */
export const toIsoDate = (shown: string) => {
  const parsed = new Date(shown);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${parsed.getFullYear()}-${pad(parsed.getMonth() + 1)}-${pad(parsed.getDate())}`;
};

/**
 * Deployed mode: fails unless the shown date is at most `maxAgeDays` before today.
 * @param shown - The date as the page shows it.
 * @param maxAgeDays - The series' `liveMaxAgeDays`.
 * @param what - Names the series in the failure message.
 */
export function expectWithinCadence(shown: string, maxAgeDays: number, what: string) {
  const ageDays = (Date.now() - new Date(shown).getTime()) / 86_400_000;
  expect(ageDays, `latest ${what} point ${shown}`).toBeLessThanOrEqual(maxAgeDays);
}

/**
 * Searches the explore page for the series' title and opens the result that is this series,
 * trying each result from its source in turn: a deployed catalog can hold several series with
 * one title (FRED's GDP and GDPA, for instance).
 * @param page - The test's page.
 * @param series - The seeded series.
 * @param seriesId - Its id, from `seededSeriesId`, to tell it from same-titled series.
 */
export async function searchAndOpen(page: Page, series: SeededSeries, seriesId: string) {
  const results = page
    .locator('.MuiCard-root')
    .filter({ has: page.getByText(series.title, { exact: true }) })
    .filter({ has: page.getByText(series.sourceName, { exact: true }) });
  const search = async () => {
    await page.goto('/explore');
    await page
      // The page's own search box, not the header's ("Search economic series...").
      .getByPlaceholder('Search economic series (e.g.', { exact: false })
      .fill(series.title);
    await page.getByRole('button', { name: 'Search', exact: true }).click();
    await expect(results.first()).toBeVisible();
  };
  await search();
  const count = await results.count();
  for (let i = 0; i < count; i++) {
    if (i > 0) await search();
    await results.nth(i).getByRole('button', { name: 'View Details' }).click();
    await expect(page).toHaveURL(/\/series\/[0-9a-f-]{36}$/);
    if (page.url().endsWith(`/series/${seriesId}`)) return;
  }
  throw new Error(`no ${series.sourceName} "${series.title}" result opened /series/${seriesId}`);
}

/**
 * Picks a transformation and waits out the loading bar: the previous result stays on screen while
 * the new one loads, so assertions after this should still retry.
 * @param page - A series page.
 * @param label - The option's visible text.
 */
export async function applyTransformation(page: Page, label: string) {
  await page.getByRole('combobox', { name: 'Transformation' }).click();
  await page.getByRole('option', { name: label, exact: true }).click();
  await expect(page.getByRole('progressbar', { name: 'Loading transformation' })).toHaveCount(0, {
    timeout: 15_000,
  });
}

// The CSV rules below are copied from src/utils/seriesCsv.ts on purpose, as an independent
// check of the file the browser saves; a change there should fail here until both agree.

/**
 * A metadata field as src/utils/seriesCsv.ts's `csvTextField` writes it: prefixed with `'` when
 * it would start a spreadsheet formula, then quoted when it holds a comma, quote or line break.
 * @param text - The raw field.
 * @returns The field as written.
 */
function csvTextField(text: string) {
  const field = /^[=+\-@\t\r]/.test(text) ? `'${text}` : text;
  return /[",\r\n]/.test(field) ? `"${field.replace(/"/g, '""')}"` : field;
}

/**
 * Downloads the series page's CSV of levels and checks its file name (seriesCsvFileName's rule)
 * and its metadata header.
 * @param page - A series page showing levels.
 * @param series - The series, with the units its page shows.
 * @returns The data rows after the `date,value` header, each split into `[date, value]`; a
 *   missing value is `''`.
 */
export async function downloadLevelsCsv(
  page: Page,
  series: SeededSeries & { units: string }
): Promise<string[][]> {
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('button', { name: 'Download CSV' }).click(),
  ]);
  const fileName = series.externalId.replace(/[^A-Za-z0-9._-]+/g, '_').replace(/^[._]+/, '');
  expect(download.suggestedFilename()).toBe(`${fileName}.csv`);
  const raw = readFileSync(await download.path(), 'utf8');
  // Drop the UTF-8 byte-order mark before splitting into lines.
  const text = raw.charCodeAt(0) === 0xfeff ? raw.slice(1) : raw;
  const lines = text.trim().split('\r\n');
  expect(lines.slice(0, 4)).toEqual([
    `Title,${csvTextField(series.title)}`,
    `Source,${csvTextField(series.sourceName)}`,
    `Units,${csvTextField(series.units)}`,
    'Transformation,None',
  ]);
  expect(lines[4]).toMatch(/^Retrieved at,/);
  expect(lines[5]).toBe('date,value');
  // Data rows hold only ISO dates and plain numbers, so a comma always separates the two.
  return lines.slice(6).map(line => line.split(','));
}
