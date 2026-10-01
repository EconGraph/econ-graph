// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

import { seededSeriesId } from '../auth/helpers';
import { DEPLOYED } from '../env';
import { SWEPT_SOURCES } from '../fixtures';
import {
  applyTransformation,
  downloadLevelsCsv,
  expectWithinCadence,
  latestShown,
  parseShown,
  searchAndOpen,
  toIsoDate,
} from '../seriesPage';

// Live QA: REL-QA check 3's per-source sweep on the deployed build. For each train 1 source
// (FRED, BLS, Census BDS, BEA, FHFA, WDI), search for one of its series, open it, check the
// chart's newest point, apply every transformation, and download the CSV. In fixture mode every
// value is the seeded one; deployed, the newest point must be within the source's publication
// cadence (`liveMaxAgeDays`) and the other checks are structural.
//
// The journey (journey.spec.ts) and the series-ui specs go deep on FRED GDP and BLS CPI; this
// checks the same pages hand off for every source, since each source has its own adapter,
// frequency and units, and a page that works for FRED can still break for an annual Census
// series.

/** The value column's header for each transformation (TRANSFORMATION_OPTIONS' descriptions). */
const TRANSFORMATION_DESCRIPTIONS: Record<string, string> = {
  'Year-over-Year': 'Year-over-Year % Change',
  'Quarter-over-Quarter': 'Quarter-over-Quarter % Change',
  'Month-over-Month': 'Month-over-Month % Change',
  'Change since first observation': '% Change since First Observation',
  'Log difference': 'Log Difference',
};

test.describe('six-source sweep', () => {
  // Search, six transformations and a download per test.
  test.slow();

  for (const series of SWEPT_SOURCES) {
    test(`${series.source}: search, chart, transform, download`, async ({ page, request }) => {
      const seriesId = await seededSeriesId(request, series);

      await test.step('search finds and opens the series', async () => {
        await searchAndOpen(page, series, seriesId);
      });

      await test.step('the series page charts it, newest point first', async () => {
        await expect(page.getByRole('heading', { level: 1 })).toHaveText(series.title);
        await expect(
          page.getByRole('img', { name: `Line chart of ${series.title}` })
        ).toBeVisible();
        await expect(page.getByText(series.sourceName, { exact: true }).first()).toBeVisible();

        const latest = await latestShown(page);
        if (DEPLOYED) {
          // Live: no older than the source's publication cadence allows (see liveMaxAgeDays).
          expectWithinCadence(latest.date, series.liveMaxAgeDays, series.externalId);
        } else {
          expect(latest).toEqual(series.latestShown);
        }
      });

      await test.step('every transformation recomputes the values', async () => {
        const levels = await latestShown(page);
        // Every option the page offers, so a transformation added later is swept too.
        await page.getByRole('combobox', { name: 'Transformation' }).click();
        await expect(page.getByRole('option', { name: 'None', exact: true })).toBeVisible();
        const offered = await page.getByRole('option').allInnerTexts();
        await page.keyboard.press('Escape');
        const labels = offered.map(o => o.trim()).filter(o => o !== 'None');
        expect([...labels].sort()).toEqual(Object.keys(series.transformations).sort());

        for (const label of labels) {
          const expected = series.transformations[label];
          const notEnoughExpected = DEPLOYED
            ? series.liveNotEnough.includes(label)
            : expected === null;
          await applyTransformation(page, label);
          const notEnough = page.getByText(/^Not enough observations to compute /);
          if (notEnoughExpected) {
            // Too few observations, e.g. month-over-month on an annual series.
            await expect(notEnough).toBeVisible();
            await page.getByRole('button', { name: 'Show levels' }).click();
            await expect(notEnough).toHaveCount(0);
            continue;
          }
          await expect(notEnough, `${label} has values`).toHaveCount(0);
          const header = page.getByRole('table', { name: 'Recent observations' }).locator('thead');
          await expect(header).toContainText(TRANSFORMATION_DESCRIPTIONS[label]);
          if (DEPLOYED) {
            // Live values are unknown, but a transformed value is a number unlike the level.
            const shown = await latestShown(page);
            expect(Number.isFinite(parseShown(shown.value)), shown.value).toBe(true);
            expect(shown.value).not.toEqual(levels.value);
          } else {
            await expect.poll(() => latestShown(page)).toEqual(expected);
          }
        }
        await applyTransformation(page, 'None');
        await expect.poll(() => latestShown(page)).toEqual(levels);
      });

      await test.step('the CSV holds the points charted', async () => {
        const rows = await downloadLevelsCsv(page, series);
        if (DEPLOYED) {
          expect(rows.length).toBeGreaterThan(0);
        } else {
          expect(rows).toHaveLength(series.csvRows);
        }
        // The newest non-missing row is the point the page lists first.
        const newest = [...rows].reverse().find(([, value]) => value !== '');
        expect(newest?.[0]).toBe(toIsoDate((await latestShown(page)).date));
      });
    });
  }
});

