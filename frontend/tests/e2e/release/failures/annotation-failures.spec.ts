// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test, type Page } from '@playwright/test';

import { seededSeriesId, signInOnKeycloak, uniqueTitle } from '../auth/helpers';
import { SKIP_AUTH, USERS } from '../env';
import { failGraphQL, graphqlOperation } from './faults';

// ECO-264: a failed annotation save or delete says so, leaves the panel showing what is really
// stored, and trying again succeeds without a duplicate. Signed in as USERS.failureAnnotator
// (erin), whom no other spec signs in, and serial, so erin never signs in twice at once.
// Live QA: runs as qa-bob against a deployed build (env.ts).
test.describe('annotation save and delete failures', () => {
  test.skip(SKIP_AUTH, 'RELEASE_SKIP_AUTH=1: sign-in is known broken for this run');
  test.slow();
  test.describe.configure({ mode: 'serial' });

  let seriesPath: string;

  test.beforeAll(async ({ request }) => {
    seriesPath = `/series/${await seededSeriesId(request)}`;
  });

  const annotationsList = (page: Page) => page.getByRole('list', { name: 'Annotations' });
  const rowsTitled = (page: Page, title: string) =>
    annotationsList(page).getByText(title, { exact: true });

  /**
   * Reloads the series page and waits for the panel to finish loading, so a count of zero rows
   * means none are stored.
   * @param page - The signed-in series page.
   */
  async function reloadAnnotations(page: Page) {
    await page.reload();
    await expect(page.getByRole('button', { name: 'Add annotation' })).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId('annotations-loading')).toHaveCount(0);
    await expect(
      annotationsList(page).or(page.getByText('No annotations on this series yet.'))
    ).toBeVisible();
  }

  async function signIn(page: Page) {
    await page.goto(seriesPath);
    await page.getByRole('button', { name: 'Sign in to annotate' }).click();
    await signInOnKeycloak(page, USERS.failureAnnotator);
    await expect(page.getByRole('button', { name: 'Add annotation' })).toBeVisible({
      timeout: 15_000,
    });
  }

  async function fillAndSave(page: Page, title: string) {
    await page.getByRole('button', { name: 'Add annotation' }).click();
    const dialog = page.getByRole('dialog', { name: 'Add annotation' });
    await dialog.getByRole('textbox', { name: 'Title' }).fill(title);
    await dialog.getByRole('button', { name: 'Save' }).click();
    return dialog;
  }

  async function deleteRow(page: Page, title: string) {
    await annotationsList(page).getByRole('button', { name: `Delete ${title}` }).click();
    const dialog = page.getByRole('dialog', { name: 'Delete annotation?' });
    await dialog.getByRole('button', { name: 'Delete' }).click();
    return dialog;
  }

  /**
   * Deletes every stored annotation titled `title`, after a reload so the panel shows what the
   * backend stores.
   * @param page - The signed-in series page.
   * @param title - The annotation title.
   * @returns How many were stored before deleting.
   */
  async function deleteAllTitled(page: Page, title: string): Promise<number> {
    await reloadAnnotations(page);
    const stored = await rowsTitled(page, title).count();
    for (let i = stored; i > 0; i--) {
      await annotationsList(page).getByRole('button', { name: `Delete ${title}` }).first().click();
      await page
        .getByRole('dialog', { name: 'Delete annotation?' })
        .getByRole('button', { name: 'Delete' })
        .click();
      await expect(rowsTitled(page, title)).toHaveCount(i - 1);
    }
    return stored;
  }

  /**
   * Best-effort cleanup for a `finally`: removes injected faults and any copies of `title` left
   * behind by a test that failed part way, without masking the test's own failure.
   * @param page - The signed-in series page.
   * @param title - The annotation title.
   */
  async function cleanUp(page: Page, title: string) {
    try {
      await page.unrouteAll({ behavior: 'ignoreErrors' });
      await deleteAllTitled(page, title);
    } catch (error) {
      test.info().annotations.push({
        type: 'cleanup failed',
        description: `annotation "${title}": ${String(error)}`,
      });
    }
  }

  test('a failed save keeps the form open with the error, and saving again adds one', async ({
    page,
  }) => {
    await signIn(page);
    const title = uniqueTitle('Failed-save annotation');
    const fault = await failGraphQL(page, 'CreateAnnotation', 'http-503');

    const dialog = await fillAndSave(page, title);
    await expect(dialog.getByRole('alert')).toBeVisible();
    // The form keeps what was typed, so saving again needs no retyping.
    await expect(dialog.getByRole('textbox', { name: 'Title' })).toHaveValue(title);
    await expect(rowsTitled(page, title)).toHaveCount(0);
    expect(fault.failed()).toBe(1);

    fault.heal();
    await dialog.getByRole('button', { name: 'Save' }).click();
    await expect(dialog).toBeHidden();
    await expect(rowsTitled(page, title)).toHaveCount(1);

    // What the backend stores, not just what the page shows: one row.
    await reloadAnnotations(page);
    await expect(rowsTitled(page, title)).toHaveCount(1);

    await deleteRow(page, title);
    await expect(rowsTitled(page, title)).toHaveCount(0);
  });

  // The request reaches the backend, which stores the annotation, but the response is lost (a
  // dropped connection, a proxy timeout). The page reports a failure and keeps the form open, so
  // the user saves again. The retry sends the same idempotency key, so the backend returns the
  // stored annotation instead of inserting a second copy.
  test(
    'saving again after a lost response does not add a duplicate',
    async ({ page }) => {
      await signIn(page);
      const title = uniqueTitle('Lost-response annotation');
      let dropped = 0;
      await page.route('**/graphql', async route => {
        if (dropped > 0 || graphqlOperation(route.request()) !== 'CreateAnnotation') {
          await route.fallback();
          return;
        }
        dropped += 1;
        await route.fetch(); // the backend commits it
        await route.abort('connectionreset');
      });

      const dialog = await fillAndSave(page, title);
      await expect(dialog.getByRole('alert')).toBeVisible();
      await dialog.getByRole('button', { name: 'Save' }).click();
      await expect(dialog).toBeHidden();

      // Clean up every copy before failing on the count.
      const stored = await deleteAllTitled(page, title);
      expect(stored).toBe(1);
    }
  );

  test('a failed delete keeps the row and says so, and deleting again removes it', async ({
    page,
  }) => {
    await signIn(page);
    const title = uniqueTitle('Failed-delete annotation');
    try {
      const created = await fillAndSave(page, title);
      await expect(created).toBeHidden();
      await expect(rowsTitled(page, title)).toHaveCount(1);

      const fault = await failGraphQL(page, 'DeleteAnnotation', 'graphql-error');
      const dialog = await deleteRow(page, title);
      await expect(dialog.getByRole('alert')).toBeVisible();
      await expect(dialog).toBeVisible();
      await dialog.getByRole('button', { name: 'Cancel' }).click();
      await expect(dialog).toBeHidden();
      // Still listed, as it is still stored.
      await expect(rowsTitled(page, title)).toHaveCount(1);
      await reloadAnnotations(page);
      await expect(rowsTitled(page, title)).toHaveCount(1);

      fault.heal();
      const retry = await deleteRow(page, title);
      await expect(retry).toBeHidden();
      await expect(rowsTitled(page, title)).toHaveCount(0);
      await reloadAnnotations(page);
      await expect(rowsTitled(page, title)).toHaveCount(0);
    } finally {
      // erin's annotation must not outlive a failure part way through.
      await cleanUp(page, title);
    }
  });
});
