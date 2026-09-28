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
      // fred/observations_gdp.json has 5 observations, one of them missing ("."), so 4 shown
      // points (SeriesDetail.tsx filters nulls before capping at the last 10).
      const shownPointCount = 4;
      await expect(page.getByRole('columnheader', { name: 'Date' })).toBeVisible();

      const downloadPromise = page.waitForEvent('download');
      await page.getByRole('button', { name: 'Download CSV' }).click();
      const download = await downloadPromise;

      expect(download.suggestedFilename()).toMatch(/\.csv$/);
      const csvPath = await download.path();
      const csv = readFileSync(csvPath as string, 'utf-8');
      const lines = csv.trim().split('\n');
      expect(lines[0].toLowerCase()).toContain('date');
      // One line per shown point, plus the header.
      expect(lines.length).toBe(shownPointCount + 1);
    }
  );
});
