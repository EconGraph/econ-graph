// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';

import { expect, test } from '@playwright/test';

import { DEPLOYED } from '../env';
import { SEEDED } from '../fixtures';
import { seededSeriesId } from '../auth/helpers';

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
    const series = SEEDED.fredGdp;
    const seriesId = await seededSeriesId(request, series);
    await page.goto(`/series/${seriesId}`);
    // fred/observations_gdp.json, oldest first: the chart's default range shows every point,
    // including the missing one ("." in the fixture), which the CSV keeps as an empty value.
    const shownObservations: Array<{ date: string; value: number | null }> = [
      { date: '2025-04-01', value: 30485.729 },
      { date: '2025-07-01', value: 31095.089 },
      { date: '2025-10-01', value: null },
      { date: '2026-01-01', value: 31722.514 },
      { date: '2026-04-01', value: 32101.6 },
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
      // Deployed mode: live GDP has decades of observations, so check structure instead of
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
      const expectedDate = new Date(`${latest.date}T00:00:00Z`).toLocaleDateString('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        timeZone: 'UTC',
      });
      const expectedValue = latest.value.toLocaleString('en-US', {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
      });
      await expect(latestRow.getByRole('cell').nth(0)).toHaveText(expectedDate);
      await expect(latestRow.getByRole('cell').nth(1)).toHaveText(expectedValue);
    }
  });
});
