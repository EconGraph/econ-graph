// Copyright (c) 2026 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';
import { SEEDED_WDI } from '../fixtures';
import { expectEqualEarthGeometry, expectEqualLandAreas } from './equal-earth';

// No GraphQL routes: this spec exercises the seeded real backend in the release suite.
test('Equal Earth is the default, preserves land areas and retains seeded country interactions', async ({ page }) => {
  await page.goto('/global');
  const germany = page.locator('path.country[data-iso-numeric="276"]');
  await expect(germany).toHaveAttribute('data-has-data', 'true');
  await expectEqualEarthGeometry(page);
  await expectEqualLandAreas(page);

  const originalPath = await germany.getAttribute('d');
  const originalFill = await germany.evaluate(el => (el as SVGElement).style.fill);
  await page.getByRole('combobox', { name: 'Projection' }).click();
  await page.getByRole('option', { name: 'Natural Earth', exact: true }).click();
  await expect(germany).not.toHaveAttribute('d', originalPath!);
  await expect(germany).toHaveAttribute('data-has-data', 'true');
  await page.getByRole('combobox', { name: 'Projection' }).click();
  await page.getByRole('option', { name: 'Equal Earth', exact: true }).click();
  await expectEqualEarthGeometry(page);
  expect(await germany.evaluate(el => (el as SVGElement).style.fill)).toBe(originalFill);

  await germany.hover();
  const tooltip = page.getByTestId('country-tooltip');
  const gdp = SEEDED_WDI.indicators.find(indicator => indicator.code === 'NY.GDP.PCAP.CD')!;
  await expect(tooltip).toContainText('Germany');
  await expect(tooltip).toContainText(
    gdp.latest.DEU.value.toLocaleString('en-US', { maximumFractionDigits: 2 })
  );
  await expect(tooltip).toContainText(gdp.latest.DEU.date.slice(0, 4));

  // The values table provides the independently returned series URL for this seeded country.
  await page.getByRole('button', { name: /Values as a table/ }).click();
  const seriesUrl = await page.getByRole('link', { name: 'Germany', exact: true }).getAttribute('href');
  await page.getByRole('button', { name: /Values as a table/ }).click();
  await germany.click();
  await expect(page).toHaveURL(new RegExp(`${seriesUrl}$`));
});
