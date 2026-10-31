// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';

import { expect, test, type Download, type Page } from '@playwright/test';

import { devUserToken, readAsDevUser, seededSeriesId } from '../auth/helpers';
import { DEPLOYED } from '../env';
import {
  failGraphQL,
  graphqlOperation,
  QUERY_ERROR_TIMEOUT,
  type FaultKind,
} from '../failures/faults';
import { SEEDED_MULTIPAGE } from '../fixtures';
import { recentRows, splitSeriesCsv } from '../seriesPage';

// ECO-261: a series too long for one `seriesData` page, driven through the series page —
// loading, a date range, a transformation and the CSV download — and checked observation by
// observation against an expected series this spec derives itself from the raw upstream response
// the seed loads (fred/observations_multipage_e2e.json), never from app code. That catches a
// truncated, skipped, repeated or reordered page, a lost missing value and a stale revision. A
// later page that fails must leave the page reporting an error, never a partial series.
// The pagination code's own unit tests are src/hooks/__tests__/; this is the end-to-end check.
// Fixture mode only: a deployed build has no such series. Live QA stands in with the CSV
// download spec's deployed branch. The series is FRED's, which only admins read, so every request
// carries an admin's token (auth/helpers.ts' readAsDevUser).

/**
 * The backend's page cap, which the frontend requests (SERIES_PAGE_SIZE). Copied rather than
 * imported, so a change to either side shows up here.
 */
const PAGE_SIZE = 10_000;

/** The range picked on the chart, inclusive, as typed (MM/DD/YYYY) and as ISO dates. */
const RANGE = {
  start: { iso: '1961-05-15', typed: '05151961', chip: 'From: May 15, 1961' },
  end: { iso: '2019-12-31', typed: '12312019', chip: 'To: Dec 31, 2019' },
};

const PERCENT_CHANGE = {
  label: 'Change since first available value',
  description: '% Change since First Available Value',
  code: 'PERCENT_CHANGE',
};

interface Observation {
  date: string;
  value: number | null;
}

interface FredRow {
  date: string;
  value: string;
  realtime_start: string;
}

/**
 * The series as the page should show it, from the raw FRED response the seed loads: each date's
 * latest vintage (greatest `realtime_start`), oldest date first, with FRED's "." as missing.
 * @returns Every observation.
 */
function expectedLevels(): Observation[] {
  const raw = JSON.parse(readFileSync(SEEDED_MULTIPAGE.observationsFixture, 'utf8')) as {
    observations: FredRow[];
  };
  const latest = new Map<string, FredRow>();
  for (const row of raw.observations) {
    const seen = latest.get(row.date);
    if (!seen || row.realtime_start > seen.realtime_start) latest.set(row.date, row);
  }
  return [...latest.values()]
    .sort((a, b) => (a.date < b.date ? -1 : a.date > b.date ? 1 : 0))
    .map(row => ({ date: row.date, value: row.value === '.' ? null : Number(row.value) }));
}

/**
 * "Change since first available value": each value's percent change from the series' first
 * non-missing, non-zero value (over the whole series, not the shown range); missing stays missing.
 * @param levels - The whole series.
 * @returns The transformed series.
 */
function expectedPercentChange(levels: Observation[]): Observation[] {
  const base = levels.find(o => o.value !== null && o.value !== 0)?.value;
  if (base === undefined || base === null) throw new Error('fixture has no usable base value');
  return levels.map(o => ({
    date: o.date,
    value: o.value === null ? null : ((o.value - base) / base) * 100,
  }));
}

const inRange = (observations: Observation[]) =>
  observations.filter(o => o.date >= RANGE.start.iso && o.date <= RANGE.end.iso);

/**
 * Fails with the first difference between two series, rather than a 22,000-row diff: ordering
 * and repeated dates first, then length, then the first date or value that differs.
 * @param actual - The series the page produced.
 * @param expected - The expected series.
 * @param what - Names the series in failure messages.
 * @param tolerance - Allowed absolute difference between values (0: exact).
 */
function expectSameSeries(
  actual: Observation[],
  expected: Observation[],
  what: string,
  tolerance = 0
) {
  const outOfOrder = actual.findIndex((o, i) => i > 0 && o.date <= actual[i - 1].date);
  expect(
    outOfOrder === -1 ? null : { index: outOfOrder, previous: actual[outOfOrder - 1], at: actual[outOfOrder] },
    `${what}: dates strictly ascending (no repeats, no reordering)`
  ).toBeNull();
  expect(actual.length, `${what}: number of observations`).toBe(expected.length);
  const differs = (a: Observation, e: Observation) =>
    a.date !== e.date ||
    (a.value === null) !== (e.value === null) ||
    (a.value !== null && e.value !== null && !(Math.abs(a.value - e.value) <= tolerance));
  const first = expected.findIndex((e, i) => differs(actual[i], e));
  expect(
    first === -1 ? null : { index: first, actual: actual[first], expected: expected[first] },
    `${what}: first observation that differs`
  ).toBeNull();
}

interface Csv {
  fileName: string;
  meta: string[];
  rows: Observation[];
}

/**
 * Clicks Download CSV and parses the file (format: src/utils/seriesCsv.ts; see
 * csv-download.spec.ts): five metadata rows, the source's notice rows, a `date,value` header,
 * then one row per point with
 * a missing value as an empty field.
 * @param page - A series page.
 * @returns The file's name, metadata rows and observations.
 */
async function downloadCsv(page: Page): Promise<Csv> {
  const [download]: [Download, void] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('button', { name: 'Download CSV' }).click(),
  ]);
  const raw = readFileSync(await download.path(), 'utf8');
  const text = raw.charCodeAt(0) === 0xfeff ? raw.slice(1) : raw;
  const lines = text.trim().split('\r\n');
  const { meta, data } = splitSeriesCsv(lines);
  // One check for the whole file: a per-row expect is a trace step per row.
  expect(
    data.find(line => !/^\d{4}-\d{2}-\d{2},[^,]*$/.test(line)) ?? null,
    'first data row that is not "date,value"'
  ).toBeNull();
  return {
    fileName: download.suggestedFilename(),
    meta,
    rows: data.map(line => {
      const [date, value] = line.split(',');
      return { date, value: value === '' ? null : Number(value) };
    }),
  };
}

/** One `GetSeriesData` request the page made, and what it got back. */
interface PageRead {
  transformation: string;
  after: string | undefined;
  status: number;
  nodes?: number;
  totalCount?: number;
}

/**
 * Records every `GetSeriesData` response the page receives, in order.
 * @param page - The test's page.
 * @returns The live list.
 */
function recordPageReads(page: Page): PageRead[] {
  const reads: PageRead[] = [];
  page.on('response', async response => {
    const request = response.request();
    if (graphqlOperation(request) !== 'GetSeriesData') return;
    const variables = request.postDataJSON()?.variables ?? {};
    const read: PageRead = {
      transformation: variables.transformation,
      after: variables.after,
      status: response.status(),
    };
    reads.push(read);
    try {
      const body = await response.json();
      read.nodes = body?.data?.seriesData?.nodes?.length;
      read.totalCount = body?.data?.seriesData?.totalCount;
    } catch {
      // A failed read (fault) has no JSON body.
    }
  });
  return reads;
}

/**
 * Types an ISO date into one of the chart's date pickers (MM/DD/YYYY sections).
 * @param page - A series page.
 * @param label - `Start Date` or `End Date`.
 * @param typed - The digits to type, e.g. `05151961`.
 */
async function typeDate(page: Page, label: string, typed: string) {
  await page
    .getByRole('group', { name: label })
    .getByRole('spinbutton', { name: 'Month' })
    .click();
  await page.keyboard.type(typed);
}

const observationsError = (page: Page) =>
  page.getByRole('alert').filter({ hasText: 'Could not load observations for this series.' });

/**
 * Records whether the chart or the Recent observations table ever appeared on the page, so a
 * partial series shown for a moment before the error still fails.
 * @param page - The test's page, before it navigates.
 */
async function watchForSeriesShown(page: Page) {
  await page.addInitScript(() => {
    const w = window as unknown as { __seriesShown?: boolean };
    w.__seriesShown = false;
    new window.MutationObserver(() => {
      if (
        document.querySelector(
          '[data-testid="series-chart"], table[aria-label="Recent observations"]'
        )
      ) {
        w.__seriesShown = true;
      }
    }).observe(document, { childList: true, subtree: true });
  });
}

const seriesWasShown = (page: Page) =>
  page.evaluate(() => (window as unknown as { __seriesShown?: boolean }).__seriesShown === true);

test.describe('multi-page series (ECO-261)', () => {
  test.skip(DEPLOYED, 'fixture mode only: the multi-page series is a seeded synthetic fixture');

  const levels = expectedLevels();
  const percentChange = expectedPercentChange(levels);
  let seriesPath: string;

  test.beforeAll(async ({ request }) => {
    const token = await devUserToken(request, 'staff-admin');
    seriesPath = `/series/${await seededSeriesId(request, SEEDED_MULTIPAGE, token)}`;
  });

  // Before each test's own fault routes, which fall back to this one.
  test.beforeEach(async ({ page, request }) => {
    await readAsDevUser(page, await devUserToken(request, 'staff-admin'));
  });

  test('the expected fixture spans three pages, with missing values', () => {
    // Pinned here so a fixture that shrinks (and the expected values with it) fails, rather
    // than quietly testing one page. See fred/make_observations_multipage.py.
    expect(levels.length).toBe(22_000);
    expect(levels.length).toBeGreaterThan(2 * PAGE_SIZE);
    expect(levels[0]).toEqual({ date: '1960-01-01', value: 100 });
    expect(levels[levels.length - 1]).toEqual({ date: '2020-03-25', value: 319.99 });
    expect(levels.filter(o => o.value === null)).toHaveLength(7);
    // Revised dates take their latest vintage, one per page, and a date first published missing
    // takes its later value.
    for (const [date, value] of [
      ['1971-10-31', 143.21],
      ['2001-01-25', 250],
      ['2017-06-30', 310],
      ['1993-10-19', 223.45],
    ] as const) {
      expect(levels.find(o => o.date === date)).toEqual({ date, value });
    }
    // The range keeps points from all three pages, and missing ones.
    const ranged = inRange(levels);
    expect(ranged).toHaveLength(21_415);
    expect(ranged.filter(o => o.value === null)).toHaveLength(6);
    expect(ranged[ranged.length - 1].date > levels[2 * PAGE_SIZE].date).toBe(true);
  });

  test('loads every page, then exports the chosen range and transformation exactly', async ({
    page,
  }) => {
    test.slow();
    const reads = recordPageReads(page);
    await page.goto(seriesPath);

    // Loading: three pages, each continuing where the last stopped.
    await expect(page.getByRole('button', { name: 'Download CSV' })).toBeEnabled({
      timeout: 30_000,
    });
    await expect
      .poll(() => reads.filter(r => r.transformation === 'NONE').slice(0, 3))
      .toEqual([
      { transformation: 'NONE', after: undefined, status: 200, nodes: 10_000, totalCount: 22_000 },
      { transformation: 'NONE', after: '10000', status: 200, nodes: 10_000, totalCount: 22_000 },
      { transformation: 'NONE', after: '20000', status: 200, nodes: 2_000, totalCount: 22_000 },
    ]);

    // The Recent observations table: the last ten non-missing points, newest first — all from
    // the last page.
    const lastTen = levels
      .filter(o => o.value !== null)
      .slice(-10)
      .reverse();
    await expect(recentRows(page)).toHaveCount(10);
    for (const [i, o] of lastTen.entries()) {
      const cells = recentRows(page).nth(i).getByRole('cell');
      await expect(cells.nth(0)).toHaveText(
        new Date(`${o.date}T00:00:00Z`).toLocaleDateString('en-US', {
          year: 'numeric',
          month: 'short',
          day: 'numeric',
          timeZone: 'UTC',
        })
      );
      await expect(cells.nth(1)).toHaveText(
        (o.value as number).toLocaleString('en-US', {
          minimumFractionDigits: 2,
          maximumFractionDigits: 2,
        })
      );
    }

    // The whole series, as loaded: the CSV at the chart's default range holds every point.
    const all = await downloadCsv(page);
    expect(all.fileName).toBe(`${SEEDED_MULTIPAGE.externalId}.csv`);
    expect(all.meta.slice(0, 4)).toEqual([
      `Title,${SEEDED_MULTIPAGE.title}`,
      `Source,${SEEDED_MULTIPAGE.sourceName}`,
      `Units,${SEEDED_MULTIPAGE.units}`,
      'Transformation,None',
    ]);
    expectSameSeries(all.rows, levels, 'levels, whole series');

    // Range selection: the CSV holds exactly the chosen dates, missing ones included.
    await typeDate(page, 'Start Date', RANGE.start.typed);
    await expect(page.getByText(RANGE.start.chip)).toBeVisible();
    await typeDate(page, 'End Date', RANGE.end.typed);
    await expect(page.getByText(RANGE.end.chip)).toBeVisible();
    const ranged = await downloadCsv(page);
    expectSameSeries(ranged.rows, inRange(levels), 'levels, chosen range');

    // Transformation: read again, page by page, and exported over the same range.
    await page.getByRole('combobox', { name: 'Transformation' }).click();
    await page.getByRole('option', { name: PERCENT_CHANGE.label, exact: true }).click();
    await expect(page.getByRole('progressbar', { name: 'Loading transformation' })).toHaveCount(
      0,
      { timeout: 30_000 }
    );
    await expect(
      page.getByRole('table', { name: 'Recent observations' }).getByRole('columnheader').nth(1)
    ).toHaveText(PERCENT_CHANGE.description);
    await expect
      .poll(() =>
        reads
          .filter(r => r.transformation === PERCENT_CHANGE.code)
          .slice(0, 3)
          .map(({ after, status, nodes }) => ({ after, status, nodes }))
      )
      .toEqual([
      { after: undefined, status: 200, nodes: 10_000 },
      { after: '10000', status: 200, nodes: 10_000 },
      { after: '20000', status: 200, nodes: 2_000 },
    ]);
    await expect(page.getByText(RANGE.start.chip)).toBeVisible();
    const transformed = await downloadCsv(page);
    expect(transformed.fileName).toBe(
      `${SEEDED_MULTIPAGE.externalId}-${PERCENT_CHANGE.code.toLowerCase()}.csv`
    );
    expect(transformed.meta.slice(2, 4)).toEqual([
      'Units,%',
      `Transformation,${PERCENT_CHANGE.description}`,
    ]);
    // The backend computes in decimals; this computes in doubles.
    expectSameSeries(transformed.rows, inRange(percentChange), 'percent change, chosen range', 1e-9);
  });

  // The app retries a failed read three times (src/index.tsx); every retry reads the earlier
  // pages again and fails on the same later page, so the page ends in its error state.
  for (const kind of ['http-503', 'graphql-error', 'network'] as FaultKind[]) {
    test(`a third page that fails (${kind}) is an error, never a partial series`, async ({
      page,
    }) => {
      test.slow();
      const reads = recordPageReads(page);
      await watchForSeriesShown(page);
      const fault = await failGraphQL(
        page,
        'GetSeriesData',
        kind,
        v => v.after === String(2 * PAGE_SIZE)
      );
      await page.goto(seriesPath);

      await expect(observationsError(page)).toBeVisible({ timeout: QUERY_ERROR_TIMEOUT });
      expect(fault.failed()).toBeGreaterThan(0);
      // The first two pages did arrive, so a partial series was there to be shown.
      expect(reads.filter(r => r.status === 200 && r.nodes === PAGE_SIZE).map(r => r.after)).toEqual(
        expect.arrayContaining([undefined, String(PAGE_SIZE)])
      );
      await expect(page.getByTestId('series-chart')).toHaveCount(0);
      await expect(page.getByRole('button', { name: 'Download CSV' })).toHaveCount(0);
      await expect(page.getByRole('table', { name: 'Recent observations' })).toHaveCount(0);
      expect(await seriesWasShown(page), 'chart or recent data shown before the error').toBe(false);

      // Once the page can be read, Retry loads all of it.
      fault.heal();
      await observationsError(page).getByRole('button', { name: 'Retry' }).click();
      await expect(page.getByRole('button', { name: 'Download CSV' })).toBeEnabled({
        timeout: 30_000,
      });
      await expect(observationsError(page)).toHaveCount(0);
      // The watcher does see the series once it is shown, so its `false` above means something.
      expect(await seriesWasShown(page), 'watcher saw the series after Retry').toBe(true);
      expectSameSeries((await downloadCsv(page)).rows, levels, 'levels after Retry');
    });
  }

  test('a second page that fails while transforming is an error, not a partial transformation', async ({
    page,
  }) => {
    test.slow();
    await page.goto(seriesPath);
    await expect(page.getByRole('button', { name: 'Download CSV' })).toBeEnabled({
      timeout: 30_000,
    });

    const fault = await failGraphQL(
      page,
      'GetSeriesData',
      'graphql-error',
      v => v.transformation === PERCENT_CHANGE.code && v.after === String(PAGE_SIZE)
    );
    await page.getByRole('combobox', { name: 'Transformation' }).click();
    await page.getByRole('option', { name: PERCENT_CHANGE.label, exact: true }).click();

    await expect(observationsError(page)).toBeVisible({ timeout: QUERY_ERROR_TIMEOUT });
    expect(fault.failed()).toBeGreaterThan(0);
    // No chart and no export, transformed or otherwise, while the error shows; the error
    // offers levels instead.
    await expect(page.getByTestId('series-chart')).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Download CSV' })).toHaveCount(0);
    await expect(observationsError(page).getByRole('button', { name: 'Show levels' })).toBeVisible();
    // Nothing on the page is labeled as the transformation.
    await expect(page.getByText(PERCENT_CHANGE.description)).toHaveCount(0);

    fault.heal();
    await observationsError(page).getByRole('button', { name: 'Retry' }).click();
    await expect(page.getByRole('button', { name: 'Download CSV' })).toBeEnabled({
      timeout: 30_000,
    });
    const transformed = await downloadCsv(page);
    expect(transformed.meta[3]).toBe(`Transformation,${PERCENT_CHANGE.description}`);
    expectSameSeries(transformed.rows, percentChange, 'percent change after Retry', 1e-9);
  });
});
