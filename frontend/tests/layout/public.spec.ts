import { test, expect, PUBLIC_URL, above, box, contained, noHorizontalOverflow } from './fixtures';

test('public navigation separates labels, contains targets and shows keyboard focus', async ({
  page,
}) => {
  await page.goto(PUBLIC_URL);
  await page.evaluate(() => document.fonts.ready);
  const nav = page.getByRole('navigation', { name: 'Main navigation' });
  const items = [
    'dashboard',
    'explore-series',
    'fred-data',
    'global-analysis',
    'data-sources',
    'about',
  ];
  for (const item of items) {
    const target = page.getByTestId(`sidebar-nav-${item}`);
    await contained(target, nav);
    await contained(page.getByTestId(`sidebar-nav-${item}-title`), target);
    await contained(page.getByTestId(`sidebar-nav-${item}-description`), target);
    await above(
      page.getByTestId(`sidebar-nav-${item}-title`),
      page.getByTestId(`sidebar-nav-${item}-description`)
    );
    expect((await box(target)).height).toBeGreaterThanOrEqual(44);
  }
  const target = page.getByTestId('sidebar-nav-explore-series');
  // Actual keyboard traversal; DOM focus alone does not establish :focus-visible.
  for (let i = 0; i < 15 && !(await target.evaluate(el => el === document.activeElement)); i++) {
    await page.keyboard.press('Tab');
  }
  await expect(target).toBeFocused();
  const focus = await target.evaluate(el => {
    const s = getComputedStyle(el);
    return {
      visible: el.matches(':focus-visible'),
      style: s.outlineStyle,
      width: parseFloat(s.outlineWidth),
      color: s.outlineColor,
    };
  });
  expect(focus.visible).toBe(true);
  expect(focus.style).not.toBe('none');
  expect(focus.width).toBeGreaterThan(0);
  expect(focus.color).not.toBe('rgba(0, 0, 0, 0)');
});

test('public dashboard cards align and keep text and actions inside their panels', async ({
  page,
}, testInfo) => {
  await page.goto(PUBLIC_URL);
  if (testInfo.project.name === 'mobile') await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'Economic Dashboard' })).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  const cards = page.locator('.MuiCard-root');
  await expect(cards).toHaveCount(4);
  const first = await box(cards.nth(0));
  const second = await box(cards.nth(1));
  if (testInfo.project.name === 'desktop') {
    await noHorizontalOverflow(page.locator('html'));
    expect(Math.abs(first.y - second.y)).toBeLessThanOrEqual(1);
    expect(Math.abs(first.height - second.height)).toBeLessThanOrEqual(1);
    expect(second.x).toBeGreaterThan(first.x + first.width);
  } else {
    await above(cards.nth(0), cards.nth(1), 16);
    expect(Math.abs(first.x - second.x)).toBeLessThanOrEqual(1);
  }
  for (const card of await cards.all()) {
    await noHorizontalOverflow(card);
    for (const text of await card.locator('.MuiTypography-root').all()) await contained(text, card);
  }
  const actions = [
    'Employment Data',
    'Inflation Indicators',
    'GDP & Growth',
    'Browse Data Sources',
  ];
  for (let i = 0; i < actions.length - 1; i++) {
    await above(
      page.getByRole('button', { name: actions[i], exact: true }),
      page.getByRole('button', { name: actions[i + 1], exact: true }),
      12
    );
  }
});
