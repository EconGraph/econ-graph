// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test, type APIRequestContext } from '@playwright/test';

import { DEPLOYED } from '../env';
import { DASHBOARD_CARDS, type DashboardCard } from '../fixtures';

/**
 * The id of a card's series, looked up anonymously as the card itself looks it up.
 * @param request - The test's API context, with no token.
 * @param card - The dashboard card.
 * @returns The series id.
 */
async function anonymousSeriesId(request: APIRequestContext, card: DashboardCard) {
  const response = await request.post('/graphql', {
    data: {
      query:
        'query($sourceName: String!, $externalId: String!) { seriesByExternalId(sourceName: $sourceName, externalId: $externalId) { id } }',
      variables: { sourceName: card.sourceName, externalId: card.externalId },
    },
  });
  expect(response.ok()).toBe(true);
  const body = await response.json();
  expect(body.errors).toBeUndefined();
  const id = body.data.seriesByExternalId?.id;
  expect(id, `${card.sourceName} ${card.externalId} readable anonymously`).toBeTruthy();
  return id as string;
}

// The dashboard's cards (src/config/dashboard-series.json) are BEA and BLS series, which the
// realm makes public, so a visitor who never signs in sees every card's value.
test.describe('dashboard cards', () => {
  test('a visitor sees the four cards and no others', async ({ page }) => {
    await page.goto('/');
    const cards = page.getByRole('main').getByRole('article');
    await expect(cards).toHaveCount(DASHBOARD_CARDS.length);
    for (const card of DASHBOARD_CARDS) {
      await expect(page.getByRole('article', { name: card.label })).toBeVisible();
    }
  });

  for (const expected of DASHBOARD_CARDS) {
    test(`a visitor's ${expected.label} card shows its latest value, units and date`, async ({
      page,
      request,
    }) => {
      const seriesId = await anonymousSeriesId(request, expected);
      await page.goto('/');

      const card = page.getByRole('article', { name: expected.label });
      // The link appears only once the series has loaded with an observation.
      await expect(card.getByRole('link', { name: expected.label })).toHaveAttribute(
        'href',
        `/series/${seriesId}`
      );
      await expect(card.getByText(expected.units, { exact: true })).toBeVisible();
      await expect(card.getByText(expected.chip, { exact: true })).toBeVisible();
      await expect(card.getByText('No data yet')).toHaveCount(0);
      await expect(card.getByText("Couldn't load this series")).toHaveCount(0);
      await expect(card.getByText('No value reported')).toHaveCount(0);
      await expect(card.locator('time')).toHaveCount(1);
      if (!DEPLOYED) {
        await expect(card.getByText(expected.latestShown.value, { exact: true })).toBeVisible();
        await expect(card.getByText(expected.latestShown.date, { exact: true })).toBeVisible();
      }
      await expect(card.getByTestId('source-notice')).toBeVisible();
    });
  }

  // toBeVisible() ignores clipping by an ancestor's overflow, and MUI's Card hides its overflow,
  // so this compares boxes: a notice pushed past the card's bottom edge is cut off on screen.
  for (const viewport of [
    { width: 1440, height: 900 },
    { width: 375, height: 812 },
  ]) {
    test(`at ${viewport.width} px every card's source notice and terms link lie inside it`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await page.goto('/');
      for (const expected of DASHBOARD_CARDS) {
        const card = page.getByRole('article', { name: expected.label });
        const notice = card.getByTestId('source-notice');
        await expect(notice).toBeVisible();
        const terms = notice.getByRole('link', { name: 'Terms of use' });
        await expect(terms).toBeVisible();
        await card.scrollIntoViewIfNeeded();
        const cardBox = await card.boundingBox();
        expect(cardBox, `${expected.label} card box`).not.toBeNull();
        for (const [what, inner] of [
          ['notice', notice],
          ['terms link', terms],
        ] as const) {
          const box = await inner.boundingBox();
          expect(box, `${expected.label} ${what} box`).not.toBeNull();
          if (!cardBox || !box) continue;
          // Half a pixel of slack for subpixel layout.
          expect(box.x, `${expected.label} ${what} left`).toBeGreaterThanOrEqual(cardBox.x - 0.5);
          expect(box.y, `${expected.label} ${what} top`).toBeGreaterThanOrEqual(cardBox.y - 0.5);
          expect(box.x + box.width, `${expected.label} ${what} right`).toBeLessThanOrEqual(
            cardBox.x + cardBox.width + 0.5
          );
          expect(box.y + box.height, `${expected.label} ${what} bottom`).toBeLessThanOrEqual(
            cardBox.y + cardBox.height + 0.5
          );
        }
      }
    });
  }
});
