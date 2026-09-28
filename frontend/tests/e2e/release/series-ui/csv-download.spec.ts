// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';

import { expect, test } from '@playwright/test';

import { SEEDED } from '../fixtures';
import { seededSeriesId } from '../auth/helpers';

// Live QA: downloading CSV from a series page saves the points currently shown on its chart.
//
// There is no such button on the series page today (src/pages/SeriesDetail.tsx) — only the
// Explorer's "Export Results" exports the current search results, not one series' own points.
// Un-fixme once the series page grows a "Download CSV" action (#223).
test.describe('download CSV', () => {
  test.fixme(
    'downloads the points shown on the series chart (blocked on #223)',
    async ({ page, request }) => {
      const seriesId = await seededSeriesId(request, SEEDED.fredGdp);
      await page.goto(`/series/${seriesId}`);
      // fred/observations_gdp.json's 4 non-missing observations (its 5th, 2025-10-01, is "." and
      // dropped before the chart or this table ever see it).
      const shownObservations = [
        { date: '2025-04-01', value: 30485.729 },
        { date: '2025-07-01', value: 31095.089 },
        { date: '2026-01-01', value: 31722.514 },
        { date: '2026-04-01', value: 32101.6 },
      ];
      await expect(page.getByRole('columnheader', { name: 'Date' })).toBeVisible();

      const downloadPromise = page.waitForEvent('download');
      await page.getByRole('button', { name: 'Download CSV' }).click();
      const download = await downloadPromise;

      expect(download.suggestedFilename()).toMatch(/\.csv$/);
      const csvPath = await download.path();
      const csv = readFileSync(csvPath as string, 'utf-8');
      const lines = csv.trim().split('\n');
      const header = lines[0].toLowerCase().split(',');
      const dateCol = header.findIndex(h => h.includes('date'));
      const valueCol = header.findIndex(h => h.includes('value'));
      expect(dateCol, 'a date column').toBeGreaterThanOrEqual(0);
      expect(valueCol, 'a value column').toBeGreaterThanOrEqual(0);

      // One line per shown point, plus the header.
      expect(lines.length).toBe(shownObservations.length + 1);
      const rows = lines.slice(1).map(line => {
        const cells = line.split(',');
        return { date: cells[dateCol].trim(), value: Number(cells[valueCol].replace(/[^0-9.-]/g, '')) };
      });
      for (const observation of shownObservations) {
        expect(rows).toContainEqual(
          expect.objectContaining({
            date: expect.stringContaining(observation.date),
            value: observation.value,
          })
        );
      }
    }
  );
});
