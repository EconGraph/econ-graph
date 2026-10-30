/**
 * The world map on real data: the indicator picker comes from the WDI dataset's codes, the map
 * colors countries by the indicator's latest value from `crossSection`, and the tooltip shows
 * the value with its unit and date. GraphQL is mocked.
 */

import React from 'react';
import { geoEqualEarth, geoPath } from 'd3-geo';
import { feature } from 'topojson-client';
import { loadWorldAtlas } from '../worldAtlas';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import WorldMapTab from '../WorldMapTab';
import { NO_DATA_FILL } from '../mapFormat';
import { executeGraphQL } from '../../../utils/graphql';

// The shared setup file stubs d3-geo and d3-zoom; the map needs the real ones.
vi.unmock('d3-geo');
vi.unmock('d3-zoom');
// Clicking a country navigates, so this needs the real router.
vi.unmock('react-router-dom');

vi.mock('../../../utils/graphql', () => ({ executeGraphQL: vi.fn() }));

const mockGraphQL = vi.mocked(executeGraphQL);

// The first dynamic import of the atlas chunk can be slow on a cold CI runner.
const WAIT = { timeout: 5000 };

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
        { code: 'SP.POP.TOTL', label: 'Population, total', unit: null },
        { code: 'NY.GDP.PCAP.CD', label: 'GDP per capita (current US$)', unit: null },
        { code: 'FP.CPI.TOTL.ZG', label: 'Inflation, consumer prices (annual %)', unit: '%' },
      ],
    },
    { name: 'area', label: 'Area', codelist: 'countries', codes: [] },
  ],
};

const area = (name: string, iso3: string | null, isoNumeric: number | null, kind = 'COUNTRY') => ({
  name,
  iso3,
  isoNumeric,
  kind,
});

const CROSS_SECTIONS: Record<string, unknown[]> = {
  'NY.GDP.PCAP.CD': [
    {
      key: 'CHN',
      seriesId: 's-chn',
      date: '2023-01-01',
      value: '12614.1',
      area: area('China', 'CHN', 156),
    },
    {
      key: 'DEU',
      seriesId: 's-deu',
      date: '2022-01-01',
      value: '54343.2',
      area: area('Germany', 'DEU', 276),
    },
    {
      key: 'USA',
      seriesId: 's-usa',
      date: '2023-01-01',
      value: '82769.4',
      area: area('United States', 'USA', 840),
    },
    // A microstate the 1:50m outline has no shape for: in the table, not on the map.
    {
      key: 'TUV',
      seriesId: 's-tuv',
      date: '2023-01-01',
      value: '5000',
      area: area('Tuvalu', 'TUV', 798),
    },
    // No value: shown as no data.
    { key: 'AUS', seriesId: 's-aus', date: null, value: null, area: area('Australia', 'AUS', 36) },
    // No ISO numeric code, so it can't be joined to the outline.
    {
      key: 'XKX',
      seriesId: 's-xkx',
      date: '2023-01-01',
      value: '5943.1',
      area: area('Kosovo', null, null),
    },
    // Aggregates are left out.
    {
      key: 'WLD',
      seriesId: 's-wld',
      date: '2023-01-01',
      value: '13138.3',
      area: area('World', null, null, 'AGGREGATE'),
    },
  ],
  'FP.CPI.TOTL.ZG': [
    {
      key: 'CHN',
      seriesId: 's-chn-cpi',
      date: '2023-01-01',
      value: '0.2',
      area: area('China', 'CHN', 156),
    },
    {
      key: 'USA',
      seriesId: 's-usa-cpi',
      date: '2023-01-01',
      value: '4.1',
      area: area('United States', 'USA', 840),
    },
    {
      key: 'AUS',
      seriesId: 's-aus-cpi',
      date: '2023-01-01',
      value: '5.6',
      area: area('Australia', 'AUS', 36),
    },
  ],
};

beforeEach(() => {
  mockGraphQL.mockReset();
  mockGraphQL.mockImplementation(async request => {
    switch (request.operationName) {
      case 'GetMapDatasets':
        return { data: { datasets: [DATASET] } };
      case 'GetMapCrossSection': {
        const indicator = request.variables?.filter[0].value as string;
        return { data: { crossSection: CROSS_SECTIONS[indicator] ?? [] } };
      }
      case 'GetMapSeriesMeta':
        return {
          data: {
            series: { id: request.variables?.id, units: 'current US$', frequency: 'Annual' },
          },
        };
      default:
        throw new Error(`unexpected operation ${request.operationName}`);
    }
  });
});

const renderTab = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={['/global']}>
        <Routes>
          <Route path='/global' element={<WorldMapTab />} />
          <Route path='/series/:id' element={<div>series page</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  );
};

const countryPath = (isoNumeric: string) =>
  document.querySelector(`path.country[data-iso-numeric="${isoNumeric}"]`) as HTMLElement;

const kosovoPath = () => document.querySelector('path.country[data-name="Kosovo"]') as HTMLElement;

/** Wait until the map is drawn and colored for the current indicator. */
const waitForColors = () =>
  waitFor(() => {
    expect(countryPath('840')?.getAttribute('data-has-data')).toBe('true');
  }, WAIT);

describe('WorldMapTab', () => {
  it('starts with Equal Earth geometry and retains explicit projection choices', async () => {
    renderTab();
    await waitForColors();
    const atlas = await loadWorldAtlas();
    const germany = feature(atlas, atlas.objects.countries).features.find(
      country => Number(country.id) === 276
    )!;
    const expectedPath = geoPath(geoEqualEarth().fitSize([960, 500], { type: 'Sphere' }))(germany);
    expect(screen.getByRole('combobox', { name: 'Projection' })).toHaveTextContent('Equal Earth');
    expect(countryPath('276')).toHaveAttribute('d', expectedPath);
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Projection' }));
    fireEvent.click(screen.getByRole('option', { name: 'Natural Earth' }));
    await waitFor(() => expect(countryPath('276')).not.toHaveAttribute('d', expectedPath));
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Indicator' }));
    fireEvent.click(screen.getByRole('option', { name: 'Inflation, consumer prices (annual %)' }));
    expect(screen.getByRole('combobox', { name: 'Projection' })).toHaveTextContent('Natural Earth');
  });

  it('offers the dataset indicators and starts on GDP per capita', async () => {
    renderTab();
    await waitForColors();

    const crossSectionCall = mockGraphQL.mock.calls.find(
      ([request]) => request.operationName === 'GetMapCrossSection'
    );
    expect(crossSectionCall?.[0].variables).toEqual({
      datasetId: 'ds-wdi',
      filter: [{ dimension: 'indicator', value: 'NY.GDP.PCAP.CD' }],
      across: 'area',
    });

    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Indicator' }));
    const options = within(screen.getByRole('listbox')).getAllByRole('option');
    expect(options.map(option => option.textContent)).toEqual([
      'Population, total',
      'GDP per capita (current US$)',
      'Inflation, consumer prices (annual %)',
    ]);
  });

  it('colors countries by value and shows countries without a value as no data', async () => {
    renderTab();
    await waitForColors();

    const fill = (path: HTMLElement) => path.style.fill;
    const usa = fill(countryPath('840'));
    const china = fill(countryPath('156'));
    expect(usa).not.toBe('');
    expect(usa).not.toBe(fill(countryPath('276')));
    expect(usa).not.toBe(china);

    // A null value, Kosovo (no ISO numeric code) and a country missing from the response.
    const noData = new Set([
      fill(countryPath('36')),
      fill(kosovoPath()),
      fill(countryPath('250')), // France
    ]);
    expect(noData.size).toBe(1);
    expect(countryPath('36').getAttribute('data-has-data')).toBe('false');
    expect(kosovoPath().getAttribute('data-has-data')).toBe('false');
    expect([...noData][0]).not.toBe(usa);

    const legend = screen.getByTestId('map-legend');
    // China, Germany, the United States; not Kosovo or Tuvalu (no shape), Australia (no value)
    // or the World aggregate. Tuvalu's lower value stays out of the range too.
    expect(legend).toHaveTextContent('3 countries with data.');
    expect(legend).toHaveTextContent('2 more have no shape on the map; see the table.');
    expect(legend).toHaveTextContent('Unit: current US$');
    expect(legend).toHaveTextContent("Each country's latest value, so dates differ: 2022 to 2023.");
    expect(legend).toHaveTextContent('12,614.1');
    expect(legend).toHaveTextContent('82,769.4');
    expect(NO_DATA_FILL).toBe('#d0d0d0');
  });

  it('shows value, unit and date on hover without rebuilding the map', async () => {
    renderTab();
    await waitForColors();

    const germany = countryPath('276');
    fireEvent.mouseOver(germany);

    const tooltip = await screen.findByTestId('country-tooltip');
    expect(tooltip).toHaveTextContent('Germany');
    expect(tooltip).toHaveTextContent('54,343.2 current US$');
    expect(tooltip).toHaveTextContent('2022');

    // The hover re-rendered the component, but the paths are the same nodes.
    expect(countryPath('276')).toBe(germany);

    fireEvent.mouseOut(germany);
    await waitFor(() => expect(screen.queryByTestId('country-tooltip')).toBeNull());

    fireEvent.mouseOver(kosovoPath());
    expect(await screen.findByTestId('country-tooltip')).toHaveTextContent('KosovoNo data');
  });

  it('refetches and recolors on an indicator switch, keeping the map', async () => {
    renderTab();
    await waitForColors();
    const usa = countryPath('840');
    const gdpFill = usa.style.fill;

    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Indicator' }));
    fireEvent.click(screen.getByRole('option', { name: 'Inflation, consumer prices (annual %)' }));

    // The code's own unit wins over the series' units.
    await waitFor(() => {
      expect(screen.getByTestId('map-legend')).toHaveTextContent('Unit: %');
    }, WAIT);
    expect(
      mockGraphQL.mock.calls.some(
        ([request]) =>
          request.operationName === 'GetMapCrossSection' &&
          request.variables?.filter[0].value === 'FP.CPI.TOTL.ZG'
      )
    ).toBe(true);

    expect(screen.getByTestId('map-legend')).toHaveTextContent('3 countries with data.');
    expect(screen.getByTestId('map-legend')).toHaveTextContent('Latest values, all for 2023.');
    expect(countryPath('840')).toBe(usa);
    expect(usa.style.fill).not.toBe(gdpFill);
    expect(countryPath('276').getAttribute('data-has-data')).toBe('false');
    expect(countryPath('36').getAttribute('data-has-data')).toBe('true');
  });

  it('defaults GDP to log and synchronizes the ramp and midpoint without refetching', async () => {
    renderTab();
    await waitForColors();
    expect(screen.getByRole('combobox', { name: 'Color scale' })).toHaveTextContent('Logarithmic');
    const midpoint = () =>
      Number(screen.getAllByTestId('map-scale-tick')[1].getAttribute('data-value'));
    expect(midpoint()).toBeCloseTo(Math.sqrt(12614.1 * 82769.4));
    const germany = countryPath('276');
    const logFill = germany.style.fill;
    const calls = mockGraphQL.mock.calls.filter(
      ([r]) => r.operationName === 'GetMapCrossSection'
    ).length;
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Color scale' }));
    fireEvent.click(screen.getByRole('option', { name: /^Linear$/ }));
    expect(screen.getByTestId('map-legend')).toHaveTextContent('Scale: Linear');
    expect(midpoint()).toBeCloseTo((12614.1 + 82769.4) / 2);
    expect(germany.style.fill).not.toBe(logFill);
    expect(countryPath('276')).toBe(germany);
    expect(
      mockGraphQL.mock.calls.filter(([r]) => r.operationName === 'GetMapCrossSection')
    ).toHaveLength(calls);
  });

  it('remembers a separately chosen scale for each indicator', async () => {
    renderTab();
    await waitForColors();
    const chooseScale = (name: string) => {
      fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Color scale' }));
      fireEvent.click(screen.getByRole('option', { name }));
    };
    const chooseIndicator = (name: string) => {
      fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Indicator' }));
      fireEvent.click(screen.getByRole('option', { name }));
    };
    chooseScale('Linear');
    chooseIndicator('Inflation, consumer prices (annual %)');
    await waitFor(() => expect(screen.getByTestId('map-legend')).toHaveTextContent('Inflation'));
    chooseScale('Logarithmic');
    chooseIndicator('GDP per capita (current US$)');
    await waitFor(() =>
      expect(screen.getByTestId('map-legend')).toHaveTextContent('GDP per capita')
    );
    expect(screen.getByRole('combobox', { name: 'Color scale' })).toHaveTextContent('Linear');
    expect(screen.getByTestId('map-legend')).toHaveTextContent('Scale: Linear');
    chooseIndicator('Inflation, consumer prices (annual %)');
    await waitFor(() => expect(screen.getByTestId('map-legend')).toHaveTextContent('Inflation'));
    expect(screen.getByRole('combobox', { name: 'Color scale' })).toHaveTextContent('Logarithmic');
  });

  it('keeps signed observations and explains why Log is disabled', async () => {
    const signed = CROSS_SECTIONS['FP.CPI.TOTL.ZG'].map((row, i) => ({
      ...(row as object),
      ...(i === 0 ? { value: '-2' } : {}),
    }));
    mockGraphQL.mockImplementation(async request => {
      if (request.operationName === 'GetMapDatasets') return { data: { datasets: [DATASET] } };
      if (request.operationName === 'GetMapCrossSection')
        return {
          data: {
            crossSection:
              request.variables?.filter[0].value === 'FP.CPI.TOTL.ZG'
                ? signed
                : CROSS_SECTIONS['NY.GDP.PCAP.CD'],
          },
        };
      return { data: { series: { units: 'current US$', frequency: 'Annual' } } };
    });
    renderTab();
    await waitForColors();
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Indicator' }));
    fireEvent.click(screen.getByRole('option', { name: 'Inflation, consumer prices (annual %)' }));
    await screen.findByText(/zero or negative country values/);
    expect(screen.getByRole('combobox', { name: 'Color scale' })).toHaveTextContent('Linear');
    expect(countryPath('156').getAttribute('data-has-data')).toBe('true');
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Color scale' }));
    expect(screen.getByRole('option', { name: 'Logarithmic' })).toHaveAttribute(
      'aria-disabled',
      'true'
    );
  });

  it('opens the series of a clicked country', async () => {
    renderTab();
    await waitForColors();

    fireEvent.click(countryPath('840'));

    expect(await screen.findByText('series page')).toBeInTheDocument();
  });

  it('ignores clicks on countries without data', async () => {
    renderTab();
    await waitForColors();

    fireEvent.click(countryPath('36'));

    expect(screen.queryByText('series page')).toBeNull();
  });

  it('opens a country on the second tap of a touch screen, the first showing its value', async () => {
    renderTab();
    await waitForColors();
    const usa = countryPath('840');

    fireEvent.pointerDown(usa, { pointerType: 'touch' });
    fireEvent.mouseOver(usa);
    fireEvent.click(usa);
    expect(await screen.findByTestId('country-tooltip')).toHaveTextContent('82,769.4');
    expect(screen.queryByText('series page')).toBeNull();

    // A tap elsewhere, even on a country without data, moves the tooltip away.
    const france = countryPath('250');
    fireEvent.pointerDown(france, { pointerType: 'touch' });
    fireEvent.mouseOver(france);
    fireEvent.click(france);
    fireEvent.pointerDown(usa, { pointerType: 'touch' });
    fireEvent.mouseOver(usa);
    fireEvent.click(usa);
    expect(screen.queryByText('series page')).toBeNull();

    fireEvent.pointerDown(usa, { pointerType: 'touch' });
    fireEvent.click(usa);
    expect(await screen.findByText('series page')).toBeInTheDocument();
  });

  it('lists every value in a table, including countries the map cannot draw', async () => {
    renderTab();
    await waitForColors();

    fireEvent.click(screen.getByRole('button', { name: 'Values as a table (5 countries)' }));

    const table = await screen.findByRole('table', { name: 'Values by country' });
    const rows = within(table).getAllByRole('row').slice(1);
    expect(rows.map(row => row.textContent)).toEqual([
      'United States82,769.42023',
      'Germany54,343.22022',
      'China12,614.12023',
      'Kosovo5,943.12023',
      'Tuvalu5,0002023',
    ]);
    expect(within(table).getByRole('link', { name: 'Kosovo' })).toHaveAttribute(
      'href',
      '/series/s-xkx'
    );
    expect(within(table).getByRole('columnheader', { name: 'Value (current US$)' })).toBeTruthy();
  });

  it('shows the legend only once values have loaded', async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>(resolve => {
      release = resolve;
    });
    const working = mockGraphQL.getMockImplementation()!;
    mockGraphQL.mockImplementation(async request => {
      if (request.operationName === 'GetMapCrossSection') await gate;
      return working(request);
    });
    renderTab();

    expect(await screen.findByText('Loading GDP per capita (current US$)…')).toBeInTheDocument();
    expect(screen.queryByTestId('map-legend')).toBeNull();

    release();
    await waitForColors();
    expect(screen.getByTestId('map-legend')).toHaveTextContent('3 countries with data.');
  });

  it('shows values without a unit when the series metadata fails', async () => {
    const working = mockGraphQL.getMockImplementation()!;
    mockGraphQL.mockImplementation(async request => {
      if (request.operationName === 'GetMapSeriesMeta') throw new Error('meta down');
      return working(request);
    });
    renderTab();
    await waitForColors();

    const legend = screen.getByTestId('map-legend');
    expect(legend).toHaveTextContent('3 countries with data.');
    expect(legend).not.toHaveTextContent('Unit:');
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull();
  });

  it("shows the World Bank's attribution, notice and terms link", async () => {
    mockGraphQL.mockImplementation(async request => {
      if (request.operationName === 'GetMapDatasets') {
        return {
          data: {
            datasets: [
              {
                ...DATASET,
                source: {
                  name: 'World Bank Open Data',
                  notice: {
                    citation: 'The World Bank: World Development Indicators',
                    notice: 'World Bank data are licensed under CC BY 4.0.',
                    termsUrl: 'https://www.worldbank.org/ext/en/legal/terms-conditions/datasets',
                    retrievalDateRequired: false,
                  },
                },
              },
            ],
          },
        };
      }
      return { data: { crossSection: [] } };
    });
    renderTab();

    const notice = await screen.findByTestId('source-notice', {}, WAIT);
    expect(notice).toHaveTextContent('The World Bank: World Development Indicators');
    expect(notice).toHaveTextContent('World Bank data are licensed under CC BY 4.0.');
    expect(within(notice).getByRole('link', { name: 'Terms of use' })).toHaveAttribute(
      'href',
      'https://www.worldbank.org/ext/en/legal/terms-conditions/datasets'
    );
    // The caption doesn't repeat the citation the notice carries.
    expect(screen.getAllByText(/The World Bank: World Development Indicators/)).toHaveLength(1);
  });

  it('falls back to the World Bank citation in the caption without a source notice', async () => {
    renderTab();

    expect(
      await screen.findByText(/The World Bank: World Development Indicators\./, {}, WAIT)
    ).toBeInTheDocument();
    expect(screen.queryByTestId('source-notice')).not.toBeInTheDocument();
  });

  it('says so when the WDI dataset is not loaded', async () => {
    mockGraphQL.mockImplementation(async () => ({ data: { datasets: [] } }));
    renderTab();

    expect(
      await screen.findByText(
        "World Development Indicators haven't been loaded yet, so the map has no data."
      )
    ).toBeInTheDocument();
    expect(mockGraphQL).toHaveBeenCalledTimes(1);
  });

  it('offers a retry when the values fail to load', async () => {
    let fail = true;
    const working = mockGraphQL.getMockImplementation()!;
    mockGraphQL.mockImplementation(async request => {
      if (request.operationName === 'GetMapCrossSection' && fail) {
        throw new Error('backend down');
      }
      return working(request);
    });
    renderTab();

    const retry = await screen.findByRole('button', { name: 'Retry' }, WAIT);
    expect(screen.getByText("Couldn't load GDP per capita (current US$).")).toBeInTheDocument();
    fail = false;
    fireEvent.click(retry);

    await waitForColors();
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull();
  });

  it('zooms with the buttons', async () => {
    renderTab();
    await waitForColors();

    expect(screen.getByText('100%')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Zoom out' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Reset view' })).toBeDisabled();

    fireEvent.click(screen.getByRole('button', { name: 'Zoom in' }));
    expect(await screen.findByText('150%')).toBeInTheDocument();
    expect(document.querySelector('.map-container')?.getAttribute('transform')).toContain(
      'scale(1.5)'
    );

    fireEvent.click(screen.getByRole('button', { name: 'Reset view' }));
    expect(await screen.findByText('100%')).toBeInTheDocument();
  });
});
