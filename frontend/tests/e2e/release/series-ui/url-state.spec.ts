// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test, type Page } from '@playwright/test';

import { DEPLOYED } from '../env';
import { SEEDED_WDI } from '../fixtures';

// ECO-732: the explorer's sort and page and the map's indicator weren't in the URL, so Back
// from a series, or a reload, reset them.

/**
 * The titles of the explorer's result cards, in order.
 * @param page - The page to read.
 * @returns The card titles.
 */
const cardTitles = (page: Page) =>
  page.locator('.MuiCard-root .MuiTypography-h6').allTextContents();

test('the explorer keeps its sort through a series and back, and on reload', async ({ page }) => {
  await page.goto('/explore');
  await page
    .getByPlaceholder('Search economic series (e.g., GDP, unemployment, inflation)')
    .fill('gdp');
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(page.locator('.MuiCard-root').first()).toBeVisible();

  await page.getByRole('button', { name: 'Advanced Search' }).click();
  await page.getByRole('combobox', { name: 'Sort By' }).click();
  await page.getByRole('option', { name: 'Title' }).click();
  await expect(page).toHaveURL(/\/explore\?q=gdp&sort=title$/);
  const sorted = await cardTitles(page);
  expect(sorted.length).toBeGreaterThan(1);
  expect(sorted).toEqual([...sorted].sort((a, b) => a.localeCompare(b)));

  // Out through View Details, back through the series page's Explore breadcrumb
  await page
    .locator('.MuiCard-root')
    .first()
    .getByRole('button', { name: 'View Details' })
    .click();
  await expect(page).toHaveURL(/\/series\/[^/]+$/);
  await page.getByRole('navigation', { name: 'breadcrumb' }).getByRole('link', { name: 'Explore' }).click();
  await expect(page).toHaveURL(/\/explore\?q=gdp&sort=title$/);
  await expect.poll(() => cardTitles(page)).toEqual(sorted);

  // Out again, and back with the browser's Back button
  await page.locator('.MuiCard-root').nth(1).click();
  await expect(page).toHaveURL(/\/series\/[^/]+$/);
  await page.goBack();
  await expect(page).toHaveURL(/\/explore\?q=gdp&sort=title$/);
  await expect.poll(() => cardTitles(page)).toEqual(sorted);

  await page.reload();
  await expect.poll(() => cardTitles(page)).toEqual(sorted);
});

test('the map keeps its indicator through a country and back, and on reload', async ({ page }) => {
  test.skip(DEPLOYED, "uses the seeded indicators and countries, not a deployed build's");
  const population = SEEDED_WDI.indicators.find(i => i.code === 'SP.POP.TOTL')!;
  const indicator = page.getByRole('combobox', { name: 'Indicator' });
  const legend = page.getByTestId('map-legend');

  await page.goto('/global');
  await expect(legend).toContainText('GDP per capita (current US$)');
  await indicator.click();
  await page.getByRole('option', { name: population.name }).click();
  await expect(page).toHaveURL(/\/global\?indicator=SP\.POP\.TOTL$/);
  await expect(legend).toContainText(`Unit: ${population.unit}`);

  // A country's series from the values table, then Back
  const countries = Object.keys(SEEDED_WDI.countries).length;
  await page.getByRole('button', { name: `Values as a table (${countries} countries)` }).click();
  await page
    .getByRole('table', { name: 'Values by country' })
    .getByRole('link', { name: SEEDED_WDI.countries.USA })
    .click();
  await expect(page).toHaveURL(/\/series\/[^/]+$/);
  await page.goBack();

  await expect(page).toHaveURL(/\/global\?indicator=SP\.POP\.TOTL$/);
  await expect(indicator).toHaveText(population.name);
  await expect(legend).toContainText(`Unit: ${population.unit}`);

  await page.reload();
  await expect(indicator).toHaveText(population.name);
  await expect(legend).toContainText(`Unit: ${population.unit}`);
});
