// Copyright (c) 2026 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).

import { expect, test, type Page } from '@playwright/test';

const country = (page: Page, code: number) =>
  page.locator(`path.country[data-iso-numeric="${code}"]`);
const map = (page: Page) => page.getByRole('img', { name: 'World map', exact: true });

async function projection(page: Page, name: string) {
  await page.getByRole('combobox', { name: 'Projection', exact: true }).click();
  await page.getByRole('option', { name, exact: true }).click();
  await expect(page.locator('.MuiPopover-root')).toHaveCount(0);
}

/** Real pointer movement: moving the projection must change country paths, not just a group. */
async function drag(page: Page, from = 0.8, to = 0.2, yFrom = 0.55, yTo = yFrom) {
  await map(page).scrollIntoViewIfNeeded();
  const bounds = await map(page).boundingBox();
  if (!bounds) throw new Error('Map has no screen bounds');
  await page.mouse.move(bounds.x + bounds.width * from, bounds.y + bounds.height * yFrom);
  await page.mouse.down();
  await page.mouse.move(bounds.x + bounds.width * to, bounds.y + bounds.height * yTo, {
    steps: 20,
  });
  await page.mouse.up();
}

async function midpoint(page: Page, code: number) {
  return country(page, code).evaluate(element => {
    const box = (element as SVGGraphicsElement).getBBox();
    return box.x + box.width / 2;
  });
}

/** Find a visible interior point rather than a multipart country's bounding-box center. */
async function interior(page: Page, code: number) {
  return country(page, code).evaluate(element => {
    const path = element as SVGGeometryElement;
    const box = path.getBBox();
    const matrix = path.getScreenCTM();
    if (!matrix || !box.width || !box.height) throw new Error('Country is not visible');
    for (let y = 0.2; y < 0.9; y += 0.1) {
      for (let x = 0.2; x < 0.9; x += 0.1) {
        const point = new DOMPoint(box.x + box.width * x, box.y + box.height * y);
        if (path.isPointInFill(point)) {
          const screen = point.matrixTransform(matrix);
          if (document.elementFromPoint(screen.x, screen.y) === path)
            return { x: screen.x, y: screen.y };
        }
      }
    }
    throw new Error('No unobscured country interior found');
  });
}

test.describe('Geographic map dragging on the seeded backend', () => {
  test('flat maps wrap through the Pacific and repeated turns without moving the drawing offscreen', async ({
    page,
  }) => {
    let crossSections = 0;
    page.on('request', request => {
      if (request.url().endsWith('/graphql') && request.postData()?.includes('GetMapCrossSection'))
        crossSections++;
    });
    await page.goto('/global');
    await expect(country(page, 156)).toHaveAttribute('data-has-data', 'true');
    await projection(page, 'Natural Earth');
    const initial = await country(page, 156).getAttribute('d');
    const requests = crossSections;
    await drag(page);
    await expect(country(page, 156)).not.toHaveAttribute('d', initial!);
    expect(await midpoint(page, 156)).toBeLessThan(480);
    expect(await midpoint(page, 76)).toBeGreaterThan(480);
    await expect(page.locator('.map-container')).toHaveAttribute(
      'transform',
      'translate(0,0) scale(1)'
    );
    await expect(page).toHaveURL(/\/global/);
    await expect(page.getByRole('button', { name: 'Reset view' })).toBeEnabled();
    for (const [from, to] of [
      [0.8, 0.2],
      [0.8, 0.2],
      [0.2, 0.8],
      [0.2, 0.8],
    ]) {
      await drag(page, from, to);
      const bounds = await country(page, 156).evaluate(element => {
        const path = element as SVGGraphicsElement;
        const box = path.getBBox();
        return {
          x: box.x,
          y: box.y,
          width: box.width,
          height: box.height,
          d: path.getAttribute('d'),
        };
      });
      expect(bounds.d).not.toMatch(/NaN|Infinity/);
      expect(bounds.width).toBeGreaterThan(1);
      expect(bounds.x).toBeGreaterThanOrEqual(-1);
      expect(bounds.x + bounds.width).toBeLessThanOrEqual(961);
    }
    await page.getByRole('button', { name: 'Reset view' }).click();
    await expect(country(page, 156)).toHaveAttribute('d', initial!);
    expect(crossSections).toBe(requests);

    // All flat projections use the same direct movement at whole-world zoom.
    for (const name of ['Mercator', 'Equal Earth']) {
      await projection(page, name);
      const before = await country(page, 156).getAttribute('d');
      await drag(page);
      await expect(country(page, 156)).not.toHaveAttribute('d', before!);
      await expect(page.locator('.map-container')).toHaveAttribute(
        'transform',
        'translate(0,0) scale(1)'
      );
    }
  });

  test('spins the hidden hemisphere into view and preserves country interactions', async ({
    page,
  }) => {
    await page.goto('/global');
    await expect(country(page, 392)).toHaveAttribute('data-has-data', 'true');
    await projection(page, 'Orthographic');
    // Japan lies beyond the initial horizon; hidden country DOM nodes must have no geometry.
    await expect(country(page, 392)).not.toHaveAttribute('d');
    await expect(country(page, 276)).toHaveAttribute('d', /M/);
    await drag(page, 0.8, 0.2);
    await expect(country(page, 392)).toHaveAttribute('d', /M/);
    await expect(page).toHaveURL(/\/global/);
    const point = await interior(page, 392);
    await page.mouse.move(point.x, point.y);
    await expect(page.getByTestId('country-tooltip')).toContainText('Japan');
    await expect(page.getByTestId('country-tooltip')).toContainText('2023');

    // Latitude rotation and zoom do not prevent reset from restoring the fitted globe.
    await drag(page, 0.5, 0.5, 0.65, 0.35);
    await page.getByRole('button', { name: 'Zoom in' }).click();
    await drag(page, 0.65, 0.35);
    await page.getByRole('button', { name: 'Reset view' }).click();
    await expect(country(page, 392)).not.toHaveAttribute('d');
    await expect(page.getByRole('button', { name: 'Reset view' })).toBeDisabled();
    await drag(page);
    const clickPoint = await interior(page, 392);
    await page.mouse.click(clickPoint.x, clickPoint.y);
    await expect(page).toHaveURL(/\/series\//);
  });

  test('keyboard rotation and Home reset change the geographic view', async ({ page }) => {
    await page.goto('/global');
    await expect(country(page, 156)).toHaveAttribute('data-has-data', 'true');
    const before = await country(page, 156).getAttribute('d');
    await map(page).focus();
    await page.keyboard.press('ArrowLeft');
    await expect(country(page, 156)).not.toHaveAttribute('d', before!);
    await page.keyboard.press('+');
    await expect(page.getByTestId('map-zoom-controls')).toContainText('150%');
    await page.keyboard.press('Home');
    await expect(country(page, 156)).toHaveAttribute('d', before!);
    await expect(page.getByRole('button', { name: 'Reset view' })).toBeDisabled();
  });

  test('touch selection highlights only the current country and clears on rotation and reset', async ({
    page,
    isMobile,
  }) => {
    test.skip(!isMobile, 'Uses genuine touchscreen taps in the mobile Chromium project');
    await page.goto('/global');
    await expect(country(page, 156)).toHaveAttribute('data-has-data', 'true');
    await map(page).scrollIntoViewIfNeeded();
    const highlighted = () =>
      country(page, 156).evaluate(() =>
        [...document.querySelectorAll<SVGElement>('path.country')]
          .filter(path => getComputedStyle(path).stroke === 'rgb(25, 118, 210)')
          .map(path => Number(path.dataset.isoNumeric))
      );
    const tapCountry = async (code: number, name: string) => {
      const point = await interior(page, code);
      await page.touchscreen.tap(point.x, point.y);
      await expect(page.getByTestId('country-tooltip')).toContainText(name);
      await expect.poll(highlighted).toEqual([code]);
    };
    for (const [code, name] of [
      [156, 'China'],
      [76, 'Brazil'],
      [840, 'United States'],
    ] as const) {
      await tapCountry(code, name);
    }
    await map(page).focus();
    await page.keyboard.press('ArrowRight');
    await expect.poll(highlighted).toEqual([]);
    await expect(page.getByTestId('country-tooltip')).toHaveCount(0);

    await tapCountry(156, 'China');
    await page.getByRole('button', { name: 'Reset view' }).tap();
    await expect.poll(highlighted).toEqual([]);
    await expect(page.getByTestId('country-tooltip')).toHaveCount(0);
    await tapCountry(840, 'United States');
    const point = await interior(page, 840);
    await page.touchscreen.tap(point.x, point.y);
    await expect(page).toHaveURL(/\/series\//);
  });

  test('touch dragging, pinch zoom, and cancellation leave a usable globe', async ({
    page,
    isMobile,
  }) => {
    test.skip(!isMobile, 'Actual touch input uses the mobile Chromium project');
    await page.goto('/global');
    await expect(country(page, 392)).toHaveAttribute('data-has-data', 'true');
    await projection(page, 'Orthographic');
    await map(page).scrollIntoViewIfNeeded();
    const box = await map(page).boundingBox();
    if (!box) throw new Error('Map missing');
    const session = await page.context().newCDPSession(page);
    const touch = (type: string, points: Array<{ id: number; x: number; y: number }>) =>
      session.send('Input.dispatchTouchEvent', { type, touchPoints: points });
    const y = box.y + box.height * 0.55;
    await touch('touchStart', [{ id: 1, x: box.x + box.width * 0.8, y }]);
    for (let step = 1; step <= 20; step++) {
      await touch('touchMove', [{ id: 1, x: box.x + box.width * (0.8 - step * 0.03), y }]);
    }
    await touch('touchEnd', []);
    await expect(country(page, 392)).toHaveAttribute('d', /M/);
    await expect(page).toHaveURL(/\/global/);
    // Two real touch contacts spread apart: changing touch distance must zoom, not spin.
    const center = box.x + box.width / 2;
    await touch('touchStart', [
      { id: 1, x: center - 20, y },
      { id: 2, x: center + 20, y },
    ]);
    await touch('touchMove', [
      { id: 1, x: center - 35, y },
      { id: 2, x: center + 35, y },
    ]);
    await touch('touchEnd', []);
    await expect(page.getByTestId('map-zoom-controls')).not.toContainText('100%');
    await touch('touchStart', [{ id: 1, x: center, y }]);
    await touch('touchMove', [{ id: 1, x: center - 15, y }]);
    await touch('touchCancel', []);
    await page.getByRole('button', { name: 'Reset view' }).click();
    await expect(country(page, 392)).not.toHaveAttribute('d');
    await expect(page.getByRole('button', { name: 'Reset view' })).toBeDisabled();
    await session.detach();
  });
});
