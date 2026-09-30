import { test, expect, ADMIN_URL, above, box, contained, noHorizontalOverflow } from './fixtures';

test('admin queue separates headers, counts and form controls at each viewport', async ({
  page,
}, testInfo) => {
  await page.goto(ADMIN_URL);
  const table = page.getByRole('table', { name: 'Queue counts' });
  await expect(table.getByRole('cell', { name: '20', exact: true })).toBeVisible();
  await expect(page.getByText(/Workers holding jobs: 3/)).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  const headings = ['Pending', 'Processing', 'Retrying', 'Completed', 'Failed'];
  const values = ['20', '5', '2', '70', '3'];
  for (let i = 0; i < headings.length; i++) {
    const heading = table.getByRole('columnheader', { name: headings[i], exact: true });
    const value = table.getByRole('cell', { name: values[i], exact: true });
    await above(heading, value);
    expect(Math.abs((await box(heading)).x - (await box(value)).x)).toBeLessThanOrEqual(1);
  }
  const main = page.locator('.MuiContainer-root');
  const source = page.getByRole('combobox', { name: 'Source' });
  const series = page.getByRole('textbox', { name: 'Series external IDs' });
  const priority = page.getByRole('spinbutton', { name: 'Priority' });
  await above(source, series, 16);
  await above(series, priority, 16);
  for (const control of [source, series, priority]) await contained(control, main);
  if (testInfo.project.name === 'desktop') await noHorizontalOverflow(page.locator('html'));
});

test('admin desktop tabs and refresh retain visible keyboard focus', async ({ page }, testInfo) => {
  test.skip(
    testInfo.project.name === 'mobile',
    'Desktop shell check; queue/form layout is covered at both widths.'
  );
  await page.goto(ADMIN_URL);
  await expect(page.getByRole('table', { name: 'Queue counts' })).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  const tabs = page.getByRole('tablist', { name: 'Admin navigation' });
  await expect(tabs.getByRole('tab')).toHaveCount(3);
  for (const tab of await tabs.getByRole('tab').all()) await contained(tab, tabs);
  const refresh = page.getByRole('button', { name: 'Refresh', exact: true });
  const title = page.getByRole('heading', { name: 'Crawler queue' });
  const a = await box(title);
  const b = await box(refresh);
  expect(b.x - a.x - a.width).toBeGreaterThanOrEqual(7);
  for (let i = 0; i < 15 && !(await refresh.evaluate(el => el === document.activeElement)); i++)
    await page.keyboard.press('Tab');
  await expect(refresh).toBeFocused();
  await expect(refresh).toHaveClass(/Mui-focusVisible/);
  const ripple = refresh.locator('.MuiTouchRipple-rippleVisible');
  await expect(ripple).toBeVisible();
  await expect(ripple).toHaveCSS('opacity', '0.3');
  await expect(ripple.locator('.MuiTouchRipple-child')).not.toHaveCSS(
    'background-color',
    'rgba(0, 0, 0, 0)'
  );
});
