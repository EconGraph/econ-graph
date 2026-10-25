// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

import { failGraphQL, QUERY_ERROR_TIMEOUT } from './faults';

// ECO-264, sampled once: when the map's values fail to load it says so instead of drawing every
// country as "no data", and Retry draws them. The outline's own failure and Reload are
// world-map/world-atlas.spec.ts's. Live QA: runs unchanged against a deployed build.
test.describe('world map failures', () => {
  test('values that fail to load are an error, not "no data", and Retry recovers', async ({
    page,
  }) => {
    test.slow();
    const fault = await failGraphQL(page, 'GetMapCrossSection', 'http-503');
    await page.goto('/global');

    const alert = page.getByRole('alert').filter({ hasText: "Couldn't load" });
    await expect(alert).toBeVisible({ timeout: QUERY_ERROR_TIMEOUT });
    await expect(page.getByText(/No country has a value for/)).toHaveCount(0);
    await expect(page.locator('svg path.country[data-has-data="true"]')).toHaveCount(0);
    expect(fault.failed()).toBeGreaterThan(0);

    fault.heal();
    await alert.getByRole('button', { name: 'Retry' }).click();
    await expect(page.locator('svg path.country[data-has-data="true"]').first()).toBeAttached();
    await expect(alert).toHaveCount(0);
  });
});
