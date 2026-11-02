import { expect, test, type Locator, type Page } from '@playwright/test';

const fixture = '/tests/accessibility/fixtures/financial.html';

async function tabTo(page: Page, target: Locator) {
  // Traverse the real tab order instead of assigning focus programmatically.
  for (let count = 0; count < 50; count++) {
    await page.keyboard.press('Tab');
    if (await target.evaluate(element => element === document.activeElement)) break;
  }
  await expect(target).toBeFocused();
  await expect(target).toHaveCSS('outline-style', 'solid');
  await expect(target).toHaveCSS('outline-width', '2px');
  await expect(target).toHaveCSS('outline-offset', '2px');
  await expect(target).toHaveCSS('outline-color', 'rgb(25, 118, 210)');
}

async function expectButtonReset(button: Locator) {
  await expect(button).toHaveAttribute('type', 'button');
  await expect(button).toHaveCSS('appearance', 'none');
  await expect(button).toHaveCSS('text-align', 'left');
  await expect(button).toHaveCSS(
    'font-family',
    await button.evaluate(element => getComputedStyle(element.parentElement!).fontFamily)
  );
}

async function expectStackedText(button: Locator, title: string, description: string) {
  const titleBox = await button.getByText(title, { exact: true }).boundingBox();
  const descriptionBox = await button.getByText(description, { exact: true }).boundingBox();
  expect(titleBox).not.toBeNull();
  expect(descriptionBox).not.toBeNull();
  expect(descriptionBox!.y).toBeGreaterThanOrEqual(titleBox!.y + titleBox!.height - 1);
  expect(Math.abs(descriptionBox!.x - titleBox!.x)).toBeLessThan(1);
}

async function expectReadableText(button: Locator) {
  const contrast = await button.evaluate(element => {
    const rgba = (value: string) => {
      const channels = value.match(/[\d.]+/g)!.map(Number);
      return [channels[0], channels[1], channels[2], channels[3] ?? 1];
    };
    // Composite translucent backgrounds over their real ancestors, outermost first.
    const backgrounds: number[][] = [];
    for (let node: Element | null = element; node; node = node.parentElement) {
      backgrounds.unshift(rgba(getComputedStyle(node).backgroundColor));
    }
    const background = backgrounds.reduce(
      (under, over) => under.map((channel, i) => over[i] * over[3] + channel * (1 - over[3])),
      [255, 255, 255]
    );
    const text = rgba(getComputedStyle(element).color).slice(0, 3);
    const luminance = (color: number[]) =>
      color
        .map(channel => {
          const value = channel / 255;
          return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
        })
        .reduce((sum, channel, i) => sum + channel * [0.2126, 0.7152, 0.0722][i], 0);
    const levels = [luminance(text), luminance(background)].sort((a, b) => b - a);
    return (levels[0] + 0.05) / (levels[1] + 0.05);
  });
  expect(contrast).toBeGreaterThanOrEqual(4.5);
}

async function expectFullWidth(button: Locator) {
  const bounds = await button.evaluate(element => ({
    width: element.getBoundingClientRect().width,
    parentWidth: element.parentElement!.getBoundingClientRect().width,
  }));
  expect(Math.abs(bounds.width - bounds.parentWidth)).toBeLessThan(1);
}

for (const width of [390, 1280]) {
  test(`export choices retain layout, selected styling and keyboard focus at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(fixture);
    const pdf = page.getByRole('button', { name: /^PDF Report/ });
    const excel = page.getByRole('button', { name: /^Excel Workbook/ });
    await expectButtonReset(pdf);
    await expectStackedText(pdf, 'PDF Report', 'Professional PDF report with charts and analysis');
    const pdfBox = (await pdf.boundingBox())!;
    const excelBox = (await excel.boundingBox())!;
    const gridBox = (await pdf.locator('..').boundingBox())!;
    if (width < 768) {
      await expectFullWidth(pdf);
      expect(excelBox.y).toBeGreaterThanOrEqual(pdfBox.y + pdfBox.height);
    } else {
      expect(Math.abs(excelBox.y - pdfBox.y)).toBeLessThan(1);
      expect(excelBox.x).toBeGreaterThan(pdfBox.x + pdfBox.width);
      expect(Math.abs(pdfBox.width * 2 + 12 - gridBox.width)).toBeLessThan(1);
    }
    await expect(pdf).toHaveAttribute('aria-pressed', 'true');
    await expect(pdf).toHaveCSS('background-color', 'rgba(25, 118, 210, 0.08)');
    await tabTo(page, excel);
    await page.keyboard.press('Space');
    await expect(excel).toHaveAttribute('aria-pressed', 'true');
    await expect(excel).toHaveCSS('border-top-color', 'rgb(25, 118, 210)');
    await expect(excel).toHaveCSS('background-color', 'rgba(25, 118, 210, 0.08)');
    await expect(pdf).toHaveAttribute('aria-pressed', 'false');
    await expect(pdf).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
    await page.keyboard.press('Shift+Tab');
    await expect(pdf).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(pdf).toHaveAttribute('aria-pressed', 'true');
    await expect(excel).toHaveAttribute('aria-pressed', 'false');
  });
}

test('mobile filings retain full-width rows, stacked labels and keyboard focus', async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 900 });
  await page.goto(`${fixture}?surface=mobile`);
  const filing = page.getByRole('button', { name: '10-K FY 2023 Q4', exact: true });
  await expectButtonReset(filing);
  await expectFullWidth(filing);
  await expectStackedText(filing, '10-K', 'FY 2023 Q4');
  const icon = await filing.locator('svg').first().boundingBox();
  const label = await filing.getByText('10-K', { exact: true }).boundingBox();
  const status = await filing.locator('svg').last().boundingBox();
  expect(icon!.x + icon!.width).toBeLessThan(label!.x);
  expect(status!.x).toBeGreaterThan(label!.x + label!.width);
  await tabTo(page, filing);
  await page.keyboard.press('Enter');
  await expect(filing).toBeFocused();
});

for (const theme of ['light', 'dark']) {
  test(`dashboard filings retain readable hover, reset rows and keyboard selection in ${theme} mode`, async ({
    page,
  }) => {
    await page.addInitScript(theme => localStorage.setItem('theme', theme), theme);
    await page.goto(`${fixture}?surface=dashboard`);
    const filing = page.getByRole('button', { name: '10-Q - 2024', exact: true });
    await expectButtonReset(filing);
    await expectFullWidth(filing);
    await expect(filing).toHaveCSS('border-top-width', '0px');
    await expect(filing).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
    const label = await filing.getByText('10-Q - 2024', { exact: true }).boundingBox();
    const icon = await filing.locator('svg').boundingBox();
    expect(icon!.x).toBeGreaterThan(label!.x + label!.width);
    expect(Math.abs(icon!.y + icon!.height / 2 - label!.y - label!.height / 2)).toBeLessThan(1);
    await filing.hover();
    await expectReadableText(filing);
    await tabTo(page, filing);
    await page.keyboard.press('Space');
    await page.getByRole('tab', { name: 'Statements', exact: true }).click();
    await expect(page.getByText('Statement ID: statement-2', { exact: true })).toBeVisible();
  });
}
