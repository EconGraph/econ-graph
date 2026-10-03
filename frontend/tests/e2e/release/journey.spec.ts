// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

import { seededSeriesId, signInOnKeycloak, uniqueTitle } from './auth/helpers';
import { DEPLOYED, SKIP_AUTH, USERS } from './env';
import { SEEDED } from './fixtures';
import {
  downloadLevelsCsv,
  expectWithinCadence,
  latestShown,
  parseShown,
  searchAndOpen,
  toIsoDate,
} from './seriesPage';

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

test.describe('release journey', () => {
  test.slow();

  test('search, open a series, chart it, transform it, annotate it', async ({ page, request }) => {
    const seriesId = await seededSeriesId(request, series);

    await test.step('search finds the series', async () => {
      await searchAndOpen(page, series, seriesId);
    });

    await test.step('the series page charts it, newest point first', async () => {
      await expect(page.getByRole('heading', { level: 1 })).toHaveText(series.title);
      await expect(page.getByRole('img', { name: `Line chart of ${series.title}` })).toBeVisible();

      const latest = await latestShown(page);
      if (DEPLOYED) {
        // Live: no older than the series' publication cadence allows (see liveMaxAgeDays).
        expectWithinCadence(latest.date, series.liveMaxAgeDays, series.externalId);
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
      const rows = await downloadLevelsCsv(page, series);
      const [date, value] = rows[rows.length - 1];
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
