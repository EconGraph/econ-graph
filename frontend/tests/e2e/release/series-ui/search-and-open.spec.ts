// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test, type Page } from '@playwright/test';

import { SEEDED } from '../fixtures';

// Live QA: search for a real series by name and open its page from the results.
test.describe('search and open a series', () => {
  test('finds a seeded series by title and opens it', async ({ page }) => {
    await page.goto('/explore');

    // Exact match: the header also has a search box, whose placeholder ends in "..." instead.
    await page
      .getByPlaceholder('Search economic series (e.g., GDP, unemployment, inflation)')
      .fill('Gross Domestic Product');
    // Exact match: substring matching on "Search" also picks up the sidebar nav item and the
    // search box's own "Clear search text" button.
    await page.getByRole('button', { name: 'Search', exact: true }).click();

    const resultText = page.getByText(SEEDED.fredGdp.title, { exact: true });
    await expect(resultText).toBeVisible();

    // The result cards are MUI Cards with no other stable role/testid; scope by the
    // library's own class so the click lands on this series' card, not another result's.
    const card = page.locator('.MuiCard-root', { has: resultText });
    await card.getByRole('button', { name: 'View Details' }).click();

    await expect(page).toHaveURL(/\/series\/[^/]+$/);
    await expect(page.getByRole('heading', { name: SEEDED.fredGdp.title })).toBeVisible();
  });
});

// ECO-406: "gdp" listed FRED's GDP sixth, behind World Bank "GDP per capita" series, and missed
// BEA's GDP, whose title only spells it out.
test.describe('search by abbreviation', () => {
  test('"gdp" lists the series whose id is GDP first and finds the spelled-out title', async ({
    page,
  }) => {
    await page.goto('/explore');
    await page
      .getByPlaceholder('Search economic series (e.g., GDP, unemployment, inflation)')
      .fill('gdp');
    await page.getByRole('button', { name: 'Search', exact: true }).click();

    const cards = page.locator('.MuiCard-root');
    // Exact: FRED's title is a case-insensitive substring of BEA's ("Gross domestic product
    // (quarterly)"), so a substring match would pass with BEA's first.
    await expect(cards.first().getByText(SEEDED.fredGdp.title, { exact: true })).toBeVisible();
    await expect(page.getByText(SEEDED.beaGdp.title, { exact: true })).toBeVisible();
  });
});

// ECO-405: Browse Series on /sources used to open an empty explorer asking for a query. It
// lists the source's series instead.
test.describe('browse a source', () => {
  /**
   * Opens FRED's listing the way a visitor does: its Browse Series link on /sources.
   * @param page - The page to drive.
   */
  const browseFredFromSources = async (page: Page) => {
    await page.goto('/sources');
    await page
      .getByRole('region', { name: SEEDED.fredGdp.sourceName })
      .getByRole('link', { name: 'Browse Series' })
      .click();
    await expect(page).toHaveURL(/\/explore\?source=[^&]+$/);
  };

  test("Browse Series lists FRED's series without a query", async ({ page }) => {
    await browseFredFromSources(page);

    await expect(page.getByText(/series, by title/)).toBeVisible();
    await expect(page.getByText(SEEDED.fredGdp.title, { exact: true })).toBeVisible();
    await expect(page.getByText(/Type at least 2 characters/)).toHaveCount(0);
  });

  // ECO-511: a link to another /explore URL changed nothing: the explorer wrote its own form
  // back over the link's URL. Each URL's query and source now reach the form and the results.
  // The header search is the same-route link here: from FRED's listing it opens a GDP search.
  test('the header search from a source listing searches, and back and forward follow', async ({
    page,
  }) => {
    const searchBox = page.getByPlaceholder(
      'Search economic series (e.g., GDP, unemployment, inflation)'
    );
    const headerSearch = page.getByPlaceholder('Search economic series...', { exact: true });
    const sourceSelect = page.getByRole('combobox', { name: 'Source' });
    const cardSources = () =>
      page
        .locator('.MuiCard-root [title^="Data Source: "]')
        .evaluateAll(chips => chips.map(chip => chip.getAttribute('title')));
    // Where FRED's Browse Series link lands, read once the test has followed it
    let fredUrl = '';

    const expectGdpSearch = async () => {
      await expect(page).toHaveURL(/\/explore\?q=GDP$/);
      await expect(searchBox).toHaveValue('GDP');
      await expect(sourceSelect).not.toContainText(SEEDED.fredGdp.sourceName);
      await expect(page.getByText(SEEDED.beaGdp.title, { exact: true })).toBeVisible();
    };
    const expectFredBrowse = async () => {
      await expect(page).toHaveURL(fredUrl);
      await expect(searchBox).toHaveValue('');
      await expect(sourceSelect).toContainText(SEEDED.fredGdp.sourceName);
      await expect(page.getByText(/series, by title/)).toBeVisible();
      await expect(page.getByText(SEEDED.beaGdp.title, { exact: true })).toHaveCount(0);
      const sources = await cardSources();
      expect(sources.length).toBeGreaterThan(0);
      expect(new Set(sources)).toEqual(new Set([`Data Source: ${SEEDED.fredGdp.sourceName}`]));
    };

    await browseFredFromSources(page);
    fredUrl = page.url();
    await expectFredBrowse();
    // Survives only while the page stays loaded: the search must navigate within the app
    await page.evaluate(() => Object.assign(window, { eco511SamePage: true }));

    await headerSearch.fill('GDP');
    await headerSearch.press('Enter');
    await expectGdpSearch();

    await page.goBack();
    await expectFredBrowse();
    await page.goForward();
    await expectGdpSearch();
    expect(await page.evaluate(() => 'eco511SamePage' in window)).toBe(true);
  });
});

// ECO-414: at 1400px the result cards started flush at the sidebar edge, varied from 423 to
// 477px wide, and the last of an odd count spanned the row.
test.describe('results grid', () => {
  test('cards line up in equal columns with a gutter at the sidebar', async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 900 });
    await page.goto('/explore');
    const searchBox = page.getByPlaceholder(
      'Search economic series (e.g., GDP, unemployment, inflation)'
    );
    await searchBox.fill('gdp');
    await page.getByRole('button', { name: 'Search', exact: true }).click();

    const cards = page.locator('.MuiCard-root');
    await expect(cards.first().getByText(SEEDED.fredGdp.title, { exact: true })).toBeVisible();
    const boxes = await cards.evaluateAll(els => els.map(el => el.getBoundingClientRect().toJSON()));
    // FRED's and BEA's GDP at least, so there are widths to compare.
    expect(boxes.length).toBeGreaterThanOrEqual(2);

    const widths = boxes.map(box => Math.round(box.width));
    expect(new Set(widths).size, `card widths ${widths.join(', ')}`).toBe(1);

    // The cards' left edge lines up with the search panel above them, clear of the sidebar.
    const sidebar = await page.getByTestId('sidebar-desktop').boundingBox();
    const searchPanel = await searchBox
      .locator('xpath=ancestor::*[contains(@class, "MuiPaper-root")][1]')
      .boundingBox();
    expect(sidebar).not.toBeNull();
    expect(searchPanel).not.toBeNull();
    expect(boxes[0].x).toBeGreaterThanOrEqual(sidebar!.x + sidebar!.width + 16);
    expect(Math.abs(boxes[0].x - searchPanel!.x)).toBeLessThanOrEqual(1);
  });
});
