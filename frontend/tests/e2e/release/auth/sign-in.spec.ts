// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test, type Page } from '@playwright/test';

import { seededSeriesId, signInOnKeycloak, uniqueTitle, type DevUser } from './helpers';

// Live QA: the same flows on the deployed site with two real accounts, on any series page.
//
// Serial, in one file: bob and the anonymous visitor look for the annotations alice creates, and
// the last test deletes them. Sign-ins of one user also must not overlap (see helpers.ts). Titles
// are unique per run, so the checks hold however many annotations earlier runs left behind.
test.describe.configure({ mode: 'serial' });

// Waits that span a Keycloak redirect: the code exchange, the backend's first JWKS fetch and the
// series queries on a debug build. The default 5s expect timeout is too tight on a cold stack.
const AFTER_SIGN_IN = { timeout: 15_000 };

test.describe('sign-in and annotation privacy', () => {
  test.slow();

  const publicTitle = uniqueTitle('Public e2e annotation');
  const privateTitle = uniqueTitle('Private e2e annotation');
  let seriesPath: string;

  test.beforeAll(async ({ request }) => {
    seriesPath = `/series/${await seededSeriesId(request)}`;
  });

  const annotationsList = (page: Page) => page.getByRole('list', { name: 'Annotations' });

  /**
   * Opens the series page and signs in from its annotations panel.
   * @param page - The test's page (a fresh browser context).
   * @param user - Who signs in.
   */
  async function openSeriesAs(page: Page, user: DevUser) {
    await page.goto(seriesPath);
    await page.getByRole('button', { name: 'Sign in to annotate' }).click();
    await signInOnKeycloak(page, user);
    await expect(page).toHaveURL(new RegExp(`${seriesPath}$`), AFTER_SIGN_IN);
    await expect(page.getByRole('button', { name: 'Add annotation' })).toBeVisible(AFTER_SIGN_IN);
  }

  async function addAnnotation(page: Page, title: string, isPublic: boolean) {
    await page.getByRole('button', { name: 'Add annotation' }).click();
    const dialog = page.getByRole('dialog', { name: 'Add annotation' });
    await dialog.getByRole('textbox', { name: 'Title' }).fill(title);
    await dialog.getByRole('textbox', { name: 'Note' }).fill('Written by the release e2e suite.');
    const publicSwitch = dialog.getByRole('checkbox', { name: 'Public' });
    await expect(publicSwitch).not.toBeChecked();
    if (isPublic) await publicSwitch.check();
    await dialog.getByRole('button', { name: 'Save' }).click();
    await expect(dialog).toBeHidden();
    await expect(annotationsList(page).getByText(title, { exact: true })).toBeVisible();
  }

  test('alice signs in from the header and returns to the page she started on', async ({
    page,
  }) => {
    await page.goto('/about');
    await page.getByRole('button', { name: 'Sign in', exact: true }).click();

    await signInOnKeycloak(page, 'alice');

    // Back through the callback route to where sign-in started, signed in.
    await expect(page).toHaveURL(/\/about$/, AFTER_SIGN_IN);
    const userMenu = page.getByRole('button', { name: 'user menu' });
    await expect(userMenu).toBeVisible(AFTER_SIGN_IN);
    await userMenu.click();
    await expect(page.getByRole('menu').getByText('Alice Tester')).toBeVisible();
    await expect(page.getByRole('menu').getByText('alice@example.com')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toHaveCount(0);
  });

  test('alice adds a public and a private annotation', async ({ page }) => {
    await openSeriesAs(page, 'alice');

    await addAnnotation(page, publicTitle, true);
    await addAnnotation(page, privateTitle, false);

    // Both are hers: she can edit and delete both, and only the private one is badged.
    const list = annotationsList(page);
    for (const title of [publicTitle, privateTitle]) {
      await expect(list.getByRole('button', { name: `Edit ${title}` })).toBeVisible();
      await expect(list.getByRole('button', { name: `Delete ${title}` })).toBeVisible();
    }
    const rowOf = (title: string) => list.getByRole('listitem').filter({ hasText: title });
    await expect(rowOf(privateTitle).getByTestId('private-badge')).toBeVisible();
    await expect(rowOf(publicTitle).getByTestId('private-badge')).toHaveCount(0);

    // Still there after a reload (and a silent sign-in), i.e. saved, not just in the page's cache.
    await page.reload();
    await expect(list.getByText(publicTitle, { exact: true })).toBeVisible(AFTER_SIGN_IN);
    await expect(list.getByText(privateTitle, { exact: true })).toBeVisible();
  });

  test('bob sees only the public annotation, without edit or delete', async ({ page }) => {
    await openSeriesAs(page, 'bob');
    const list = annotationsList(page);

    await expect(list.getByText(publicTitle, { exact: true })).toBeVisible();
    // The list and the chart come from one query, so once the public one shows, a leaked private
    // one would too.
    await expect(page.getByText(privateTitle)).toHaveCount(0);
    // The row's controls have rendered (Comments shows for everyone), so Edit and Delete are
    // missing, not still loading: alice's annotation is not bob's to change.
    await expect(list.getByRole('button', { name: `Comments on ${publicTitle}` })).toBeVisible();
    await expect(page.getByRole('button', { name: `Edit ${publicTitle}` })).toHaveCount(0);
    await expect(page.getByRole('button', { name: `Delete ${publicTitle}` })).toHaveCount(0);
  });

  test('an anonymous visitor sees only the public annotation', async ({ page }) => {
    await page.goto(seriesPath);
    const list = annotationsList(page);

    await expect(list.getByText(publicTitle, { exact: true })).toBeVisible();
    await expect(page.getByText(privateTitle)).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Sign in to annotate' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Add annotation' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: `Edit ${publicTitle}` })).toHaveCount(0);
    await expect(page.getByRole('button', { name: `Delete ${publicTitle}` })).toHaveCount(0);
  });

  // Cleans up this run's annotations on the seeded series when the earlier tests pass.
  test('alice deletes both annotations', async ({ page }) => {
    await openSeriesAs(page, 'alice');
    const list = annotationsList(page);

    for (const title of [publicTitle, privateTitle]) {
      await list.getByRole('button', { name: `Delete ${title}` }).click();
      const dialog = page.getByRole('dialog', { name: 'Delete annotation?' });
      await dialog.getByRole('button', { name: 'Delete' }).click();
      await expect(dialog).toBeHidden();
      await expect(page.getByText(title)).toHaveCount(0);
    }
  });
});
