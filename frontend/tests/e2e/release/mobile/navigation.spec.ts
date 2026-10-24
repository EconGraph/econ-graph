// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

/**
 * Phone-only (the release-mobile project). On a phone the nav sidebar is a modal drawer: while it
 * is open it covers the page and the rest of the app is aria-hidden. ECO-335: it used to start
 * open, so every page loaded behind it.
 */
test.describe('phone navigation', () => {
  for (const path of ['/', '/explore', '/global']) {
    test(`${path} loads with the nav drawer closed`, async ({ page }) => {
      await page.goto(path);
      await expect(page.getByRole('main')).toBeVisible();
      // Mounted (keepMounted) but hidden; includeHidden so this fails if it isn't mounted at all.
      const nav = page.getByRole('navigation', { name: 'Main navigation', includeHidden: true });
      await expect(nav).toHaveCount(1);
      await expect(nav).toBeHidden();
      await expect(page.locator('#root')).not.toHaveAttribute('aria-hidden', 'true');
    });
  }

  test('the menu opens the drawer, and picking a page closes it', async ({ page }) => {
    await page.goto('/');
    await page.getByRole('button', { name: 'open drawer' }).click();

    const nav = page.getByRole('navigation', { name: 'Main navigation' });
    await expect(nav).toBeVisible();
    // While it is open, the page behind it is hidden from assistive tech.
    await expect(page.locator('#root')).toHaveAttribute('aria-hidden', 'true');

    await nav.getByRole('button', { name: /^Navigate to Global Analysis/ }).click();

    await expect(page).toHaveURL(/\/global$/);
    await expect(nav).toBeHidden();
    await expect(page.getByRole('combobox', { name: 'Projection' })).toBeVisible();
  });

  test("the map's zoom controls leave the map uncovered", async ({ page }) => {
    // Overlaid on the map they covered Europe at this width, so countries there could not be
    // hovered or tapped; on a phone they sit in a bar under the map.
    await page.goto('/global');
    const map = await page.getByRole('img', { name: 'World map' }).boundingBox();
    const controls = await page.getByTestId('map-zoom-controls').boundingBox();
    expect(map && controls).toBeTruthy();
    expect(controls!.y).toBeGreaterThanOrEqual(map!.y + map!.height - 1);
  });
});
