// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test, type Page } from '@playwright/test';

import { seededSeriesId, signInOnKeycloak, uniqueTitle } from '../auth/helpers';
import { USERS } from '../env';

// Live QA: a public annotation is visible to an anonymous viewer, in the panel and on the
// chart, and clicking its row selects it. Uses its own user (USERS.publicAnnotator), never
// alice or bob: those are reserved for auth/sign-in.spec.ts's serial group, and signing one dev
// user in from two specs at once trips Keycloak's brute-force lock.
test.describe('public annotations for an anonymous viewer', () => {
  test.slow();
  test.describe.configure({ mode: 'serial' });

  const title = uniqueTitle('Public series-ui annotation');
  // One unbroken 300-character URL: before ECO-402 a note like this widened the series page
  // for every viewer (the panel grew to fit the word and the chart stretched with it).
  const note = `Written by the release e2e suite. See https://example.test/${'x'.repeat(300)}`;
  let seriesPath: string;

  test.beforeAll(async ({ request }) => {
    seriesPath = `/series/${await seededSeriesId(request)}`;
  });

  const annotationsList = (page: Page) => page.getByRole('list', { name: 'Annotations' });

  test('carol adds a public annotation', async ({ page }) => {
    await page.goto(seriesPath);
    await page.getByRole('button', { name: 'Sign in to annotate' }).click();
    await signInOnKeycloak(page, USERS.publicAnnotator);
    await expect(page.getByRole('button', { name: 'Add annotation' })).toBeVisible({
      timeout: 15_000,
    });

    await page.getByRole('button', { name: 'Add annotation' }).click();
    const dialog = page.getByRole('dialog', { name: 'Add annotation' });
    await dialog.getByRole('textbox', { name: 'Title' }).fill(title);
    await dialog.getByRole('textbox', { name: 'Note' }).fill(note);
    await dialog.getByRole('switch', { name: 'Public' }).check();
    await dialog.getByRole('button', { name: 'Save' }).click();
    await expect(dialog).toBeHidden();
    await expect(annotationsList(page).getByText(title, { exact: true })).toBeVisible();
  });

  test('an anonymous viewer sees it in the panel and can select it', async ({ page }) => {
    await page.goto(seriesPath);
    const list = annotationsList(page);

    // Wait for the row's controls to render (Comments shows for everyone) before checking
    // Edit/Delete are absent, not still loading: this viewer isn't signed in, let alone the
    // annotation's owner.
    await expect(list.getByRole('button', { name: `Comments on ${title}` })).toBeVisible();
    await expect(page.getByRole('button', { name: `Edit ${title}` })).toHaveCount(0);
    await expect(page.getByRole('button', { name: `Delete ${title}` })).toHaveCount(0);

    // On the chart: chartjs-plugin-annotation draws it as a point marker on the canvas, which
    // Playwright can't query directly, but the chart only requests annotation markers for rows
    // the panel also lists, and clicking a row is how a viewer selects one either way. Click the
    // row's title, not the row itself: the row also contains the Comments control below it.
    const row = list.getByRole('listitem').filter({ hasText: title });
    await expect(row.locator('[aria-current="true"]')).toHaveCount(0);
    await row.getByText(title, { exact: true }).click();
    await expect(row.locator('[aria-current="true"]')).toHaveCount(1);
  });

  test("a long word in its note doesn't widen the page", async ({ page }) => {
    await page.goto(seriesPath);
    await expect(annotationsList(page).getByText(title, { exact: true })).toBeVisible();
    // Scoped to this run's row: the note text is the same every run, so a row left behind by an
    // interrupted run would otherwise match too.
    await expect(
      annotationsList(page).getByRole('listitem').filter({ hasText: title }).getByText(note)
    ).toBeVisible();

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth
    );
    expect(overflow).toBeLessThanOrEqual(0);
  });

  test('carol deletes the annotation', async ({ page }) => {
    await page.goto(seriesPath);
    await page.getByRole('button', { name: 'Sign in to annotate' }).click();
    await signInOnKeycloak(page, USERS.publicAnnotator);
    const list = annotationsList(page);
    await expect(list.getByText(title, { exact: true })).toBeVisible({ timeout: 15_000 });

    await list.getByRole('button', { name: `Delete ${title}` }).click();
    const dialog = page.getByRole('dialog', { name: 'Delete annotation?' });
    await dialog.getByRole('button', { name: 'Delete' }).click();
    await expect(dialog).toBeHidden();
    await expect(page.getByText(title)).toHaveCount(0);
  });

  // ECO-439: the backend stores the annotation but the response is lost (a dropped connection,
  // a proxy timeout), so the page reports a failure and carol saves again. The retry resends
  // the first attempt's idempotency key, and the backend returns the stored annotation instead
  // of adding a second. Carol, in this serial group, so no other spec signs her in meanwhile.
  test('saving again after a lost response stores one annotation', async ({ page }) => {
    const retried = uniqueTitle('Lost-response annotation');
    await page.goto(seriesPath);
    await page.getByRole('button', { name: 'Sign in to annotate' }).click();
    await signInOnKeycloak(page, USERS.publicAnnotator);
    await expect(page.getByRole('button', { name: 'Add annotation' })).toBeVisible({
      timeout: 15_000,
    });

    const keys: unknown[] = [];
    await page.route('**/graphql', async route => {
      const body = route.request().postDataJSON() as {
        query?: string;
        variables?: { input?: { idempotencyKey?: unknown } };
      } | null;
      if (!/\bmutation\s+CreateAnnotation\b/.test(body?.query ?? '')) {
        await route.fallback();
        return;
      }
      keys.push(body?.variables?.input?.idempotencyKey);
      if (keys.length > 1) {
        await route.fallback();
        return;
      }
      await route.fetch(); // the backend commits it
      await route.abort('connectionreset');
    });

    await page.getByRole('button', { name: 'Add annotation' }).click();
    const dialog = page.getByRole('dialog', { name: 'Add annotation' });
    await dialog.getByRole('textbox', { name: 'Title' }).fill(retried);
    await dialog.getByRole('button', { name: 'Save' }).click();
    await expect(dialog.getByRole('alert')).toBeVisible();
    await dialog.getByRole('button', { name: 'Save' }).click();
    await expect(dialog).toBeHidden();

    // What the backend stores, not just what the page shows.
    await page.unrouteAll({ behavior: 'wait' });
    await page.reload();
    const list = annotationsList(page);
    await expect(list.getByText(retried, { exact: true }).first()).toBeVisible({
      timeout: 15_000,
    });
    const stored = await list.getByText(retried, { exact: true }).count();
    // Remove every copy before failing, so a failure leaves nothing behind.
    for (let left = stored; left > 0; left--) {
      await list.getByRole('button', { name: `Delete ${retried}` }).first().click();
      const confirm = page.getByRole('dialog', { name: 'Delete annotation?' });
      await confirm.getByRole('button', { name: 'Delete' }).click();
      await expect(confirm).toBeHidden();
      await expect(list.getByText(retried, { exact: true })).toHaveCount(left - 1);
    }
    expect(stored).toBe(1);
    expect(keys).toHaveLength(2);
    expect(keys[0]).toEqual(expect.any(String));
    expect(keys[1]).toBe(keys[0]);
  });
});
