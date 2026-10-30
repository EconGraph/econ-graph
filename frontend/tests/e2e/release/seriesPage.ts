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

// The series page shows a quarterly, semi-annual or annual observation as its period
// (src/utils/dates.ts's formatObservationDate) and anything else as a calendar date.
const QUARTER = /^(\d{4}) Q([1-4])$/;
const HALF = /^(\d{4}) H([12])$/;
const YEAR = /^(\d{4})$/;

/**
 * A CSV's `YYYY-MM-DD` date written the way the series page wrote `like`: as its quarter
 * ("2026 Q2"), half ("2024 H1"), year ("2022") or calendar date ("Apr 1, 2026").
 * @param iso - The CSV's date.
 * @param like - A date as the page shows it, for its style.
 * @returns `iso` in that style.
 */
export function asShown(iso: string, like: string): string {
  const [year, month, day] = iso.split('-').map(Number);
  if (QUARTER.test(like)) return `${year} Q${Math.floor((month - 1) / 3) + 1}`;
  if (HALF.test(like)) return `${year} H${month <= 6 ? 1 : 2}`;
  if (YEAR.test(like)) return String(year);
  return new Date(year, month - 1, day).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/**
 * When a shown date's period starts: a quarter, half or year at its first day.
 * @param shown - The date as the page shows it.
 * @returns Local midnight on that day.
 */
function periodStart(shown: string): Date {
  const quarter = QUARTER.exec(shown);
  if (quarter) return new Date(Number(quarter[1]), (Number(quarter[2]) - 1) * 3, 1);
  const half = HALF.exec(shown);
  if (half) return new Date(Number(half[1]), (Number(half[2]) - 1) * 6, 1);
  if (YEAR.test(shown)) return new Date(Number(shown), 0, 1);
  return new Date(shown);
}

/**
 * Deployed mode: fails unless the shown date (its period's start, for a quarter or year) is at
 * most `maxAgeDays` before today.
 * @param shown - The date as the page shows it.
 * @param maxAgeDays - The series' `liveMaxAgeDays`.
 * @param what - Names the series in the failure message.
 */
export function expectWithinCadence(shown: string, maxAgeDays: number, what: string) {
  const ageDays = (Date.now() - periodStart(shown).getTime()) / 86_400_000;
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

/** Labels of the rows between a series CSV's metadata and its data (src/utils/seriesCsv.ts). */
const NOTICE_LABELS = new Set([
  'Derived values',
  'Citation',
  'Notice',
  'Retrieved from source',
  'Terms of use',
]);

/**
 * Split a series CSV's lines into its five metadata rows, the source-notice rows after them,
 * and the data rows after the `date,value` header. Every seeded source has a notice (ECO-767),
 * so the notice rows must include `Notice` and `Terms of use`, and hold nothing else.
 * @param lines - The file's lines, byte-order mark removed.
 * @returns The metadata, notice and data rows.
 */
export function splitSeriesCsv(lines: string[]): {
  meta: string[];
  notice: string[];
  data: string[];
} {
  const header = lines.indexOf('date,value');
  expect(header, 'the date,value header follows the metadata').toBeGreaterThan(4);
  const notice = lines.slice(5, header);
  const labels = notice.map(line => line.slice(0, line.indexOf(',')));
  expect(
    labels.filter(label => !NOTICE_LABELS.has(label)),
    'unexpected header rows'
  ).toEqual([]);
  expect(labels).toContain('Notice');
  expect(labels).toContain('Terms of use');
  return { meta: lines.slice(0, 5), notice, data: lines.slice(header + 1) };
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
  // Data rows hold only ISO dates and plain numbers, so a comma always separates the two.
  return splitSeriesCsv(lines).data.map(line => line.split(','));
}
