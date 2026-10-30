// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

/**
 * Phone-only (the release-mobile project). ECO-744: signed out, a labelled "Sign in" button
 * wrapped to two lines and left the search field a few characters wide. On a phone it is an
 * icon button, so the header stays one row and the search field keeps room to type in.
 */
test('signed out, the header keeps Sign in to one row and the search field usable', async ({
  page,
}) => {
  await page.goto('/');
  const signIn = page.getByRole('button', { name: 'Sign in', exact: true });
  await expect(signIn).toBeVisible();
  const search = page.getByRole('textbox', { name: 'search economic series' });

  const button = await signIn.boundingBox();
  const field = await search.boundingBox();
  expect(button && field).toBeTruthy();
  // One line: no taller than a toolbar icon button, where the wrapped label was about 61px.
  expect(button!.height).toBeLessThanOrEqual(48);
  // Room for "Search econ…" rather than "Sear…", and the two don't overlap.
  expect(field!.width).toBeGreaterThanOrEqual(120);
  expect(button!.x).toBeGreaterThanOrEqual(field!.x + field!.width - 1);
});
