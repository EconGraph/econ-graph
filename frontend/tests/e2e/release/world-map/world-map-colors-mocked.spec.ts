import { expect, test, type Page } from '@playwright/test';
import { landColorDistribution } from './land-color-distribution';

const GDP = 'NY.GDP.PCAP.CD';
const entry = (key: string, name: string, isoNumeric: number, value: number) => ({
  key,
  seriesId: `colors-${key}`,
  date: '2023-01-01',
  value: String(value),
  area: { name, iso3: key, isoNumeric, kind: 'COUNTRY' },
});
const countries = [
  entry('IND', 'India', 356, 1000),
  entry('BRA', 'Brazil', 76, 5000),
  entry('CHN', 'China', 156, 10000),
  entry('USA', 'United States', 840, 50000),
  entry('AUS', 'Australia', 36, 50000),
];

async function mockColors(page: Page, variant: 'baseline' | 'tiny' | 'large') {
  const values = [...countries];
  if (variant === 'tiny') values.push(entry('QAT', 'Qatar', 634, 100000));
  if (variant === 'large') values[3] = entry('USA', 'United States', 840, 100000);
  let crossSections = 0;
  await page.route('**/graphql', async route => {
    const { operationName } = route.request().postDataJSON();
    let data = {};
    if (operationName === 'GetMapDatasets')
      data = {
        datasets: [
          {
            id: 'color-wdi',
            code: 'wdi',
            name: 'World Development Indicators',
            dimensions: [
              {
                name: 'indicator',
                label: 'Indicator',
                codelist: null,
                codes: [{ code: GDP, label: 'GDP per capita (current US$)', unit: 'current US$' }],
              },
              { name: 'area', label: 'Area', codelist: 'countries', codes: [] },
            ],
          },
        ],
      };
    if (operationName === 'GetMapCrossSection') {
      crossSections++;
      data = { crossSection: values };
    }
    if (operationName === 'GetMapSeriesMeta')
      data = { series: { units: 'current US$', frequency: 'Annual' } };
    await route.fulfill({ json: { data } });
  });
  await page.goto('/global');
  await expect(page.locator('path.country[data-iso-numeric="156"]')).toHaveAttribute(
    'data-has-data',
    'true'
  );
  await expect(page.getByRole('combobox', { name: 'Color scale' })).toHaveText('Logarithmic');
  // Fix the projection explicitly so this regression is independent of default changes.
  await page.getByRole('combobox', { name: 'Projection' }).click();
  await page.getByRole('option', { name: 'Natural Earth', exact: true }).click();
  return () => crossSections;
}

test.describe('World map land color distribution (mocked)', () => {
  test.use({ viewport: { width: 1440, height: 1000 } });

  test('a tiny wealthy outlier does not wash out the visible low and middle GDP land', async ({
    page,
  }) => {
    const requests = await mockColors(page, 'baseline');
    const baseline = await landColorDistribution(page);
    expect(requests()).toBe(1);
    await page.unroute('**/graphql');
    const outlierRequests = await mockColors(page, 'tiny');
    await expect(page.locator('path.country[data-iso-numeric="634"]')).toHaveAttribute(
      'data-has-data',
      'true'
    );
    const tiny = await landColorDistribution(page);
    // These broad fixture-derived ranges describe the rendered areas of the five
    // large countries, not a target of equal-sized color bins or country counts.
    // Natural Earth baseline: data covers 26.5% of land; Log with Qatar gives
    // mean L*=64.5 and low/middle/high area shares 6.6%/46.7%/46.7%.
    expect(tiny.dataFraction).toBeGreaterThan(0.15);
    expect(tiny.dataFraction).toBeLessThan(0.5);
    expect(tiny.nearWhiteFraction).toBeLessThan(0.01);
    expect(tiny.meanLightness).toBeGreaterThan(40);
    expect(tiny.meanLightness).toBeLessThan(80);
    expect(tiny.lightnessBands[0]).toBeGreaterThan(0.05);
    expect(tiny.lightnessBands[1]).toBeGreaterThan(0.1);
    expect(tiny.lightnessBands[2]).toBeGreaterThan(0.1);
    expect(baseline.meanLightness - tiny.meanLightness).toBeLessThan(15);
    expect(tiny.averageRgb.every(value => value < 230)).toBe(true);
    const chinaLogFill = await page
      .locator('path.country[data-iso-numeric="156"]')
      .evaluate(node => getComputedStyle(node).fill);
    await page.getByRole('combobox', { name: 'Color scale' }).click();
    await page.getByRole('option', { name: 'Linear', exact: true }).click();
    const linear = await landColorDistribution(page);
    expect(tiny.meanLightness - linear.meanLightness).toBeGreaterThan(10);
    expect(linear.lightnessBands[0]).toBeGreaterThan(tiny.lightnessBands[0]);
    expect(
      await page
        .locator('path.country[data-iso-numeric="156"]')
        .evaluate(node => getComputedStyle(node).fill)
    ).not.toBe(chinaLogFill);
    expect(outlierRequests()).toBe(1);
  });

  test('weights a large wealthy country more heavily and keeps white land in the mask', async ({
    page,
  }) => {
    await mockColors(page, 'tiny');
    const tiny = await landColorDistribution(page);
    await page.unroute('**/graphql');
    await mockColors(page, 'large');
    const large = await landColorDistribution(page);
    // Equal maxima and almost identical land masks; increasing the United States'
    // value changes a substantial area, whereas adding Qatar changes very little.
    expect(large.meanLightness - tiny.meanLightness).toBeGreaterThan(2);
    expect(large.countryPixels[840] / large.dataPixels).toBeGreaterThan(0.15);
    // A stylesheet may override the map's inline fill; measure what is rendered.
    await page.addStyleTag({
      content: 'path.country[data-iso-numeric="156"] { fill: white !important; }',
    });
    const white = await landColorDistribution(page);
    expect(white.dataPixels).toBe(large.dataPixels);
    expect(white.nearWhiteFraction).toBeGreaterThan(0.15);
    await page.addStyleTag({ content: '.countries { opacity: 0 !important; }' });
    const invisible = await landColorDistribution(page);
    expect(invisible.dataPixels).toBe(large.dataPixels);
    expect(invisible.nearWhiteFraction).toBeGreaterThan(0.99);
  });
});
