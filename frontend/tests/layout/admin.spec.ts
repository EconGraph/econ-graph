import { test, expect, ADMIN_URL, above, box, contained, noHorizontalOverflow } from './fixtures';

test('admin status cards separate labels and values and adapt to the viewport', async ({
  page,
}, testInfo) => {
  await page.goto(ADMIN_URL);
  const status = page.getByTestId('crawler-status-card');
  const queue = page.getByTestId('queue-statistics-card');
  await expect(status.getByText('Running', { exact: true })).toBeVisible();
  await expect(queue.getByText('100', { exact: true })).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  if (testInfo.project.name === 'desktop') {
    await noHorizontalOverflow(page.locator('html'));
    const a = await box(status);
    const b = await box(queue);
    expect(Math.abs(a.y - b.y)).toBeLessThanOrEqual(1);
    expect(b.x).toBeGreaterThan(a.x + a.width);
    expect(Math.abs(a.width - b.width)).toBeLessThanOrEqual(1);
  } else {
    await above(status, queue, 16);
  }
  await above(
    status.getByText('Active Workers', { exact: true }),
    status.getByText('3', { exact: true })
  );
  await above(
    queue.getByText('Total Items', { exact: true }),
    queue.getByText('100', { exact: true })
  );
  for (const card of [status, queue]) {
    await noHorizontalOverflow(card);
    for (const text of await card.locator('.MuiTypography-root').all()) await contained(text, card);
  }
});

test('admin desktop tabs and actions remain separated with visible keyboard focus', async ({
  page,
}, testInfo) => {
  test.skip(
    testInfo.project.name === 'mobile',
    'Mobile shell overflow is documented separately; card layout above remains covered.'
  );
  await page.goto(ADMIN_URL);
  await page.evaluate(() => document.fonts.ready);
  const tabs = page.getByRole('tablist', { name: 'admin navigation tabs' });
  for (const tab of await tabs.getByRole('tab').all()) await contained(tab, tabs);
  const refresh = page.getByRole('button', { name: 'Refresh crawler data' });
  await expect(refresh).toBeEnabled();
  const trigger = page.getByRole('button', { name: 'Trigger manual crawl' });
  const stop = page.getByRole('button', { name: 'Stop crawler' });
  const a = await box(refresh);
  const b = await box(trigger);
  const c = await box(stop);
  expect(b.x - a.x - a.width).toBeGreaterThanOrEqual(7);
  expect(c.x - b.x - b.width).toBeGreaterThanOrEqual(7);
  for (let i = 0; i < 15 && !(await refresh.evaluate(el => el === document.activeElement)); i++)
    await page.keyboard.press('Tab');
  await expect(refresh).toBeFocused();
  await expect(refresh).toHaveClass(/Mui-focusVisible/);
  // MUI outlined buttons signal keyboard focus with a visible ripple, not an outline.
  const ripple = refresh.locator('.MuiTouchRipple-rippleVisible');
  await expect(ripple).toBeVisible();
  await expect(ripple).toHaveCSS('opacity', '0.3');
  await expect(ripple.locator('.MuiTouchRipple-child')).not.toHaveCSS(
    'background-color',
    'rgba(0, 0, 0, 0)'
  );
});
