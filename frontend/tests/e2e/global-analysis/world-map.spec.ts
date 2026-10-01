import { test, expect, type Page } from '@playwright/test';

/**
 * The world map on mocked GraphQL responses: an indicator picker from the WDI dataset's codes,
 * countries colored by their latest value, a tooltip with value, unit and date, zoom, and an
 * indicator switch that refetches. MAP-7's release spec checks the same against a seeded
 * backend.
 */

const DATASET = {
  id: 'ds-wdi',
  code: 'wdi',
  name: 'World Development Indicators',
  dimensions: [
    {
      name: 'indicator',
      label: 'Indicator',
      codelist: null,
      codes: [
        { code: 'NY.GDP.PCAP.CD', label: 'GDP per capita (current US$)', unit: null },
        { code: 'FP.CPI.TOTL.ZG', label: 'Inflation, consumer prices (annual %)', unit: null },
      ],
    },
    { name: 'area', label: 'Area', codelist: 'countries', codes: [] },
  ],
};

const entry = (key: string, name: string, isoNumeric: number, date: string, value: string) => ({
  key,
  seriesId: `series-${key}-${value}`,
  date,
  value,
  area: { name, iso3: key, isoNumeric, kind: 'COUNTRY' },
});

const CROSS_SECTIONS: Record<string, unknown[]> = {
  'NY.GDP.PCAP.CD': [
    entry('USA', 'United States', 840, '2023-01-01', '82769.4'),
    entry('DEU', 'Germany', 276, '2022-01-01', '54343.2'),
    entry('CHN', 'China', 156, '2023-01-01', '12614.1'),
    {
      key: 'WLD',
      seriesId: 'series-WLD',
      date: '2023-01-01',
      value: '13138.3',
      area: { name: 'World', iso3: null, isoNumeric: null, kind: 'AGGREGATE' },
    },
  ],
  'FP.CPI.TOTL.ZG': [
    entry('USA', 'United States', 840, '2023-01-01', '4.1'),
    entry('CHN', 'China', 156, '2023-01-01', '0.2'),
  ],
};

const UNITS: Record<string, string> = {
  'NY.GDP.PCAP.CD': 'current US$',
  'FP.CPI.TOTL.ZG': '% change',
};

/**
 * Answer the map's GraphQL operations.
 * @param page - The page.
 * @returns The indicators the page asked for, in order, filled in as it asks.
 */
async function mockGraphQL(page: Page): Promise<string[]> {
  const requested: string[] = [];
  await page.route('**/graphql', async route => {
    const body = route.request().postDataJSON() as {
      operationName?: string;
      variables?: Record<string, any>;
    };
    let data: unknown;
    switch (body.operationName) {
      case 'GetMapDatasets':
        data = { datasets: [DATASET] };
        break;
      case 'GetMapCrossSection': {
        const indicator = body.variables?.filter[0].value as string;
        requested.push(indicator);
        data = { crossSection: CROSS_SECTIONS[indicator] ?? [] };
        break;
      }
      case 'GetMapSeriesMeta': {
        // Asked right after the cross-section, for one of its series.
        const indicator = requested[requested.length - 1];
        data = {
          series: { id: body.variables?.id, units: UNITS[indicator], frequency: 'Annual' },
        };
        break;
      }
      default:
        data = {};
    }
    await route.fulfill({ json: { data } });
  });
  return requested;
}

const country = (page: Page, isoNumeric: number) =>
  page.locator(`svg path.country[data-iso-numeric="${isoNumeric}"]`);

test.describe('World map', () => {
  let requested: string[];

  test.beforeEach(async ({ page }) => {
    requested = await mockGraphQL(page);
    await page.goto('/global');
    await expect(country(page, 840)).toHaveAttribute('data-has-data', 'true');
  });

  test('colors countries with data and shows the rest as no data', async ({ page }) => {
    expect(requested).toEqual(['NY.GDP.PCAP.CD']);

    const fill = (isoNumeric: number) =>
      country(page, isoNumeric).evaluate(el => (el as HTMLElement).style.fill);
    const [usa, china, france] = await Promise.all([fill(840), fill(156), fill(250)]);
    expect(usa).not.toBe(china);
    expect(france).toBe('rgb(208, 208, 208)');
    await expect(country(page, 250)).toHaveAttribute('data-has-data', 'false');
    await expect(page.locator('svg path.country[data-name="Kosovo"]')).toHaveAttribute(
      'data-has-data',
      'false'
    );

    const legend = page.getByTestId('map-legend');
    await expect(legend).toContainText('GDP per capita (current US$)');
    await expect(legend).toContainText('Unit: current US$');
    // The World aggregate is left out.
    await expect(legend).toContainText('3 countries with data.');
    await expect(legend).toContainText("Each country's latest value, so dates differ: 2022 to 2023.");
  });

  test('shows value, unit and date on hover', async ({ page }) => {
    await country(page, 276).hover();

    const tooltip = page.getByTestId('country-tooltip');
    await expect(tooltip).toContainText('Germany');
    await expect(tooltip).toContainText('54,343.2 current US$');
    await expect(tooltip).toContainText('2022');

    await country(page, 76).hover();
    await expect(tooltip).toContainText('Brazil');
    await expect(tooltip).toContainText('No data');
  });

  test('keeps the tooltip inside the map near its edges', async ({ page }) => {
    const box = await page.getByTestId('world-map').boundingBox();
    // Japan, near the right edge, and Australia, near the right and bottom edges. (Australia's
    // code also marks the Ashmore and Cartier Islands, so it is found by name.)
    for (const name of ['Japan', 'Australia']) {
      await page.locator(`svg path.country[data-name="${name}"]`).hover();
      const tip = await page.getByTestId('country-tooltip').boundingBox();
      expect(tip && box).toBeTruthy();
      expect(tip!.x + tip!.width).toBeLessThanOrEqual(box!.x + box!.width);
      expect(tip!.y + tip!.height).toBeLessThanOrEqual(box!.y + box!.height);
      expect(tip!.x).toBeGreaterThanOrEqual(box!.x);
    }
  });

  test('lists the values in a table', async ({ page }) => {
    await page.getByRole('button', { name: 'Values as a table (3 countries)' }).click();

    const table = page.getByRole('table', { name: 'Values by country' });
    await expect(table.getByRole('row')).toHaveCount(4);
    await expect(table.getByRole('link', { name: 'Germany' })).toHaveAttribute(
      'href',
      '/series/series-DEU-54343.2'
    );
  });

  test('refetches when the indicator changes', async ({ page }) => {
    await page.getByRole('combobox', { name: 'Indicator' }).click();
    await page.getByRole('option', { name: 'Inflation, consumer prices (annual %)' }).click();

    const legend = page.getByTestId('map-legend');
    await expect(legend).toContainText('Unit: % change');
    await expect(legend).toContainText('2 countries with data.');
    await expect(country(page, 276)).toHaveAttribute('data-has-data', 'false');
    expect(requested).toEqual(['NY.GDP.PCAP.CD', 'FP.CPI.TOTL.ZG']);
  });

  test('zooms with the buttons', async ({ page }) => {
    const map = page.getByTestId('world-map');
    await expect(map).toContainText('100%');

    await page.getByRole('button', { name: 'Zoom in' }).click();
    await expect(map).toContainText('150%');
    await expect(page.locator('.map-container')).toHaveAttribute('transform', /scale\(1\.5\)/);

    await page.getByRole('button', { name: 'Reset zoom' }).click();
    await expect(map).toContainText('100%');
  });

  test('changes projection', async ({ page }) => {
    const before = await country(page, 840).getAttribute('d');

    await page.getByRole('combobox', { name: 'Projection' }).click();
    await page.getByRole('option', { name: 'Mercator' }).click();

    await expect(country(page, 840)).not.toHaveAttribute('d', before ?? '');
    await expect(country(page, 840)).toHaveAttribute('data-has-data', 'true');
  });

  test('opens the series of a clicked country', async ({ page }) => {
    // The middle of the United States' outline, Alaska included, isn't in the United States.
    await country(page, 840).dispatchEvent('click');

    await expect(page).toHaveURL(/\/series\/series-USA-82769\.4$/);
  });
});
