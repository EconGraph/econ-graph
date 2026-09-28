// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';

import { expect, test, type Page } from '@playwright/test';

import { seededSeriesId, signInOnKeycloak, uniqueTitle } from './auth/helpers';
import { DEPLOYED, SKIP_AUTH, USERS } from './env';
import { SEEDED } from './fixtures';

// Live QA: the same journey on the deployed build (REL-QA check 3), where the latest point is
// checked against GDP's publication cadence instead of the fixture's values.
//
// Exit criterion 5's cross-area journey, in one browser session: find a series by searching, open
// it, read its chart, apply every transformation, download the shown points, then sign in and
// annotate it. The areas' own specs cover each page in depth; this checks the pages hand off to
// each other.

const series = SEEDED.fredGdp;

// Covers a Keycloak redirect: the code exchange and the backend's first JWKS fetch.
const AFTER_SIGN_IN = { timeout: 15_000 };

const recentRows = (page: Page) =>
  page.getByRole('table', { name: 'Recent observations' }).locator('tbody tr');

/**
 * The newest observation the series page lists: the first row of its "Recent Data" table.
 * @param page - A series page.
 * @returns The row's date and value cells, as shown.
 */
async function latestShown(page: Page) {
  const cells = recentRows(page).first().getByRole('cell');
  return { date: await cells.nth(0).innerText(), value: await cells.nth(1).innerText() };
}

/**
 * Parses a number as the series page formats it (`32,101.60`).
 * @param shown - The cell text.
 * @returns The number.
 */
const parseShown = (shown: string) => Number(shown.replace(/,/g, ''));

/**
 * The series page's displayed date ("Apr 1, 2026") as the CSV's `YYYY-MM-DD`.
 * @param shown - The date as `formatIsoDate` renders it.
 * @returns The ISO calendar date.
 */
const toIsoDate = (shown: string) => {
  const parsed = new Date(shown);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${parsed.getFullYear()}-${pad(parsed.getMonth() + 1)}-${pad(parsed.getDate())}`;
};

test.describe('release journey', () => {
  test.slow();

  test('search, open a series, chart it, transform it, annotate it', async ({ page, request }) => {
    const seriesId = await seededSeriesId(request, series);

    await test.step('search finds the series', async () => {
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
      // A deployed catalog can have more than one FRED series with this title (GDPA, the annual
      // one, for instance), so open each match until one is the seeded series.
      await search();
      const count = await results.count();
      for (let i = 0; i < count; i++) {
        if (i > 0) await search();
        await results.nth(i).getByRole('button', { name: 'View Details' }).click();
        await expect(page).toHaveURL(/\/series\/[0-9a-f-]{36}$/);
        if (page.url().endsWith(`/series/${seriesId}`)) return;
      }
      throw new Error(`no ${series.sourceName} "${series.title}" result opened /series/${seriesId}`);
    });

    await test.step('the series page charts it, newest point first', async () => {
      await expect(page.getByRole('heading', { level: 1 })).toHaveText(series.title);
      await expect(page.getByRole('img', { name: `Line chart of ${series.title}` })).toBeVisible();

      const latest = await latestShown(page);
      if (DEPLOYED) {
        // Live: no older than the series' publication cadence allows (see liveMaxAgeDays).
        const ageDays = (Date.now() - new Date(latest.date).getTime()) / 86_400_000;
        expect(ageDays, `latest ${series.externalId} point ${latest.date}`).toBeLessThanOrEqual(
          series.liveMaxAgeDays
        );
      } else {
        expect(latest).toEqual(series.latestShown);
      }
    });

    await test.step('every transformation recomputes the values', async () => {
      const levels = await latestShown(page);
      for (const t of series.transformations) {
        await page.getByRole('combobox', { name: 'Transformation' }).click();
        await page.getByRole('option', { name: t.label, exact: true }).click();
        if (t.latestShown === null) {
          await expect(
            page.getByText(`Not enough observations to compute ${t.description.toLowerCase()}`)
          ).toBeVisible();
          await page.getByRole('button', { name: 'Show levels' }).click();
          continue;
        }
        // Labelled from the transformation the backend says it applied.
        const header = page.getByRole('table', { name: 'Recent observations' }).locator('thead');
        await expect(header).toContainText(t.description || series.units);
        if (!DEPLOYED) {
          await expect.poll(() => latestShown(page)).toEqual(t.latestShown);
        } else if (t.description) {
          // Live values are unknown, but a transformed value is a number unlike the level.
          const shown = await latestShown(page);
          expect(Number.isFinite(parseShown(shown.value)), shown.value).toBe(true);
          expect(shown.value).not.toEqual(levels.value);
        }
      }
    });

    await test.step('download shows the levels just charted', async () => {
      // The loop above ends back on levels (SEEDED.fredGdp.transformations' last entry).
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
      const [date, value] = lines[lines.length - 1].split(',');
      const shown = await latestShown(page);
      expect(date).toBe(toIsoDate(shown.date));
      if (!DEPLOYED) {
        expect(Number(value)).toBe(parseShown(series.latestShown.value));
      } else {
        // Live values are unknown; the page rounds to 2 decimal places for display.
        expect(Math.abs(Number(value) - parseShown(shown.value))).toBeLessThan(0.01);
      }
    });

    if (SKIP_AUTH) {
      test.info().annotations.push({
        type: 'skipped step',
        description: 'annotate: RELEASE_SKIP_AUTH=1',
      });
      return;
    }

    await test.step('sign in and annotate it', async () => {
      const seriesUrl = page.url();
      await page.getByRole('button', { name: 'Sign in to annotate' }).click();
      await signInOnKeycloak(page, USERS.journey);
      await expect(page).toHaveURL(seriesUrl, AFTER_SIGN_IN);

      const title = uniqueTitle('Journey e2e annotation');
      const list = page.getByRole('list', { name: 'Annotations' });
      const remove = async () => {
        // Short timeouts: this also runs as cleanup after a failure, where a missing button must
        // not turn the real error into a test timeout.
        await list.getByRole('button', { name: `Delete ${title}` }).click({ timeout: 5_000 });
        const confirm = page.getByRole('dialog', { name: 'Delete annotation?' });
        await confirm.getByRole('button', { name: 'Delete' }).click({ timeout: 5_000 });
        await expect(page.getByText(title)).toHaveCount(0);
      };

      await page.getByRole('button', { name: 'Add annotation' }).click(AFTER_SIGN_IN);
      const dialog = page.getByRole('dialog', { name: 'Add annotation' });
      await dialog.getByRole('textbox', { name: 'Title' }).fill(title);
      await dialog.getByRole('textbox', { name: 'Note' }).fill('Written by the release journey.');
      await dialog.getByRole('button', { name: 'Save' }).click();
      let removed = false;
      try {
        await expect(dialog).toBeHidden();
        await expect(list.getByText(title, { exact: true })).toBeVisible();
        // Clean up: the database is shared with the other specs, with later runs, and in
        // deployed mode with real users.
        await remove();
        removed = true;
      } finally {
        // If a check above failed after the save, still try to delete what was saved.
        if (!removed) await remove().catch(() => undefined);
      }
    });
  });
});
