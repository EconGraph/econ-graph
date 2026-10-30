// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';

import { expect, test } from '@playwright/test';

import { DEPLOYED } from '../env';
import { SEEDED } from '../fixtures';
import { seededSeriesId } from '../auth/helpers';
import { asShown, parseShown } from '../seriesPage';

// Live QA: downloading CSV from a series page saves every point the chart shows, in its own
// metadata-then-data format (src/utils/seriesCsv.ts): five `label,value` rows (Title, Source,
// Units, Transformation, Retrieved at), then a `date,value` header, then one row per point,
// oldest first. Unlike the Recent Data table, the export is not capped to the last 10 points and
// keeps missing values as an empty field rather than dropping them.
test.describe('download CSV', () => {
  test('downloads every point shown on the series chart, with its metadata header', async ({
    page,
    request,
  }) => {
    const series = SEEDED.blsCpi;
    const seriesId = await seededSeriesId(request, series);
    await page.goto(`/series/${seriesId}`);
    // bls/cpi_monthly.json, oldest first: the chart's default range shows every monthly point,
    // including the missing one ("-" in the fixture), which the CSV keeps as an empty value, and
    // not the annual average (period M13).
    const shownObservations: Array<{ date: string; value: number | null }> = [
      { date: '2023-11-01', value: null },
      { date: '2023-12-01', value: 306.746 },
      { date: '2024-01-01', value: 308.417 },
      { date: '2024-02-01', value: 310.326 },
      { date: '2024-03-01', value: 312.332 },
    ];
    await expect(page.getByRole('columnheader', { name: 'Date' })).toBeVisible();

    const [download] = await Promise.all([
      page.waitForEvent('download'),
      page.getByRole('button', { name: 'Download CSV' }).click(),
    ]);

    expect(download.suggestedFilename()).toBe(`${series.externalId}.csv`);
    const raw = readFileSync(await download.path(), 'utf8');
    // Drop the UTF-8 byte-order mark before splitting into lines.
    const text = raw.charCodeAt(0) === 0xfeff ? raw.slice(1) : raw;
    const lines = text.trim().split('\r\n');

    expect(lines[0]).toBe(`Title,${series.title}`);
    expect(lines[1]).toBe(`Source,${series.sourceName}`);
    expect(lines[2]).toBe(`Units,${series.units}`);
    expect(lines[3]).toBe('Transformation,None');
    expect(lines[4]).toMatch(/^Retrieved at,/);
    expect(lines[5]).toBe('date,value');

    const rows = lines.slice(6).map(line => {
      const [date, value] = line.split(',');
      return { date, value: value === '' ? null : Number(value) };
    });

    if (!DEPLOYED) {
      // Fixture mode: one data row per observation, in the fixture's own (oldest-first) order.
      expect(rows).toEqual(shownObservations);
    } else {
      // Deployed mode: live CPI has decades of observations, so check structure instead of
      // exact values — strictly ascending dates, every value numeric or missing, and the export
      // has the same latest non-missing point as the page's Recent Data table.
      expect(rows.length).toBeGreaterThan(0);
      for (let i = 1; i < rows.length; i++) {
        expect(rows[i].date > rows[i - 1].date).toBe(true);
      }
      for (const row of rows) {
        expect(row.value === null || Number.isFinite(row.value)).toBe(true);
      }

      const latest = [...rows].reverse().find(row => row.value !== null);
      if (!latest || latest.value === null) {
        throw new Error('CSV has no non-missing observations');
      }
      const latestRow = page
        .getByRole('table', { name: 'Recent observations' })
        .getByRole('row')
        .nth(1);
      // Monthly CPI's date reads as a calendar date ("Mar 1, 2024"), and its value with the
      // decimals the series carries, up to 2.
      const dateCell = latestRow.getByRole('cell').nth(0);
      const valueCell = latestRow.getByRole('cell').nth(1);
      await expect(dateCell).toHaveText(/^[A-Z][a-z]{2} \d{1,2}, \d{4}$/);
      const shownDate = await dateCell.innerText();
      expect(asShown(latest.date, shownDate)).toBe(shownDate);
      expect(Math.abs(parseShown(await valueCell.innerText()) - latest.value)).toBeLessThan(0.01);
    }
  });
});
