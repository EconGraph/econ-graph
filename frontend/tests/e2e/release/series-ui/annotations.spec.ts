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
    await dialog.getByRole('textbox', { name: 'Note' }).fill('Written by the release e2e suite.');
    await dialog.getByRole('checkbox', { name: 'Public' }).check();
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
});
