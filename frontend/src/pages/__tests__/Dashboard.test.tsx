// Dashboard page tests: the indicator cards render the latest observation from GraphQL
// (mocked with the fixtures in test-utils/mocks/graphql-responses/get_series_latest_observation).

import React from 'react';
import { render, screen, within } from '@testing-library/react';
import { vi } from 'vitest';
import { TestProviders } from '../../test-utils/test-providers';
import Dashboard from '../Dashboard';
import dashboardSource from '../Dashboard.tsx?raw';
import cardSource from '../../components/dashboard/LatestValueCard.tsx?raw';
import { DASHBOARD_SERIES } from '../../config/dashboardSeries';
import { executeGraphQL } from '../../utils/graphql';
import gdp from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/gdp.json';
import unrate from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/unrate.json';
import cpiaucsl from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/cpiaucsl.json';
import fedfunds from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/fedfunds.json';
import notFound from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/not_found.json';
import emptySeries from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/empty_series.json';
import nullValue from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/null_value.json';

vi.mock('../../utils/graphql', async () => {
  const actual = await vi.importActual<typeof import('../../utils/graphql')>('../../utils/graphql');
  return { ...actual, executeGraphQL: vi.fn() };
});

const mockedExecuteGraphQL = vi.mocked(executeGraphQL);

type Fixture = { data: unknown };
const FIXTURES: Record<string, Fixture> = {
  GDP: gdp,
  UNRATE: unrate,
  CPIAUCSL: cpiaucsl,
  FEDFUNDS: fedfunds,
};

/**
 * Answer each series lookup from a fixture chosen by external id.
 * @param overrides - Fixtures (or errors) to use instead of the defaults, by external id.
 */
function mockResponses(overrides: Record<string, Fixture | Error> = {}) {
  mockedExecuteGraphQL.mockImplementation(async ({ variables }) => {
    const externalId = variables?.externalId as string;
    const response = overrides[externalId] ?? FIXTURES[externalId] ?? notFound;
    if (response instanceof Error) throw response;
    return response as Awaited<ReturnType<typeof executeGraphQL>>;
  });
}

function renderDashboard() {
  return render(
    <TestProviders>
      <Dashboard />
    </TestProviders>
  );
}

/**
 * The card for a dashboard entry, found by its heading.
 * @param label - The card heading.
 * @returns The card element.
 */
function card(label: string): HTMLElement {
  return screen.getByRole('article', { name: label });
}

describe('Dashboard', () => {
  beforeEach(() => {
    mockedExecuteGraphQL.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  test('lists the four FRED series from the config file', () => {
    expect(DASHBOARD_SERIES.map(entry => entry.externalId)).toEqual([
      'GDP',
      'UNRATE',
      'CPIAUCSL',
      'FEDFUNDS',
    ]);
    for (const entry of DASHBOARD_SERIES) {
      expect(entry.sourceName).toBe('Federal Reserve Economic Data (FRED)');
    }
  });

  test('looks each series up by source name and external id', async () => {
    mockResponses();
    renderDashboard();

    await screen.findByText('29,723.9');
    expect(mockedExecuteGraphQL).toHaveBeenCalledTimes(4);
    for (const entry of DASHBOARD_SERIES) {
      expect(mockedExecuteGraphQL).toHaveBeenCalledWith(
        expect.objectContaining({
          query: expect.stringContaining('seriesByExternalId'),
          variables: { sourceName: entry.sourceName, externalId: entry.externalId },
        })
      );
    }
  });

  test('shows each value, its units and its observation date from the response', async () => {
    mockResponses();
    renderDashboard();

    const gdpCard = card('Gross Domestic Product');
    expect(await within(gdpCard).findByText('29,723.9')).toBeInTheDocument();
    expect(within(gdpCard).getByText('Billions of Dollars')).toBeInTheDocument();
    expect(within(gdpCard).getByText('Oct 1, 2024')).toHaveAttribute('datetime', '2024-10-01');
    expect(within(gdpCard).getByText('FRED GDP')).toBeInTheDocument();

    const unrateCard = card('Unemployment Rate');
    expect(await within(unrateCard).findByText('4.1')).toBeInTheDocument();
    expect(within(unrateCard).getByText('Percent')).toBeInTheDocument();
    expect(within(unrateCard).getByText('Feb 1, 2025')).toBeInTheDocument();

    // 319.0825 rounds up to 319.083 on the digits; float rounding would give 319.082.
    const cpiCard = card('Consumer Price Index');
    expect(await within(cpiCard).findByText('319.083')).toBeInTheDocument();
    expect(within(cpiCard).getByText('Index 1982-1984=100')).toBeInTheDocument();

    const fedFundsCard = card('Federal Funds Rate');
    expect(await within(fedFundsCard).findByText('4.33')).toBeInTheDocument();
  });

  test('links each card to its series page', async () => {
    mockResponses();
    renderDashboard();

    await screen.findByText('29,723.9');
    expect(within(card('Gross Domestic Product')).getByRole('link')).toHaveAttribute(
      'href',
      `/series/${gdp.data.seriesByExternalId.id}`
    );
    expect(within(card('Federal Funds Rate')).getByRole('link')).toHaveAttribute(
      'href',
      `/series/${fedfunds.data.seriesByExternalId.id}`
    );
  });

  test('shows "No data yet" without a link when the series does not exist', async () => {
    mockResponses({ GDP: notFound });
    renderDashboard();

    const gdpCard = card('Gross Domestic Product');
    expect(await within(gdpCard).findByText('No data yet')).toBeInTheDocument();
    expect(within(gdpCard).queryByRole('link')).not.toBeInTheDocument();
    // The other cards still load.
    expect(await within(card('Unemployment Rate')).findByText('4.1')).toBeInTheDocument();
  });

  test('shows "No data yet" when the series has no observations', async () => {
    mockResponses({ FEDFUNDS: emptySeries });
    renderDashboard();

    const fedFundsCard = card('Federal Funds Rate');
    expect(await within(fedFundsCard).findByText('No data yet')).toBeInTheDocument();
    expect(within(fedFundsCard).getByRole('link')).toHaveAttribute(
      'href',
      `/series/${emptySeries.data.seriesByExternalId.id}`
    );
  });

  test('says no value was reported when the latest observation has a null value', async () => {
    mockResponses({ UNRATE: nullValue });
    renderDashboard();

    const unrateCard = card('Unemployment Rate');
    expect(await within(unrateCard).findByText('No value reported')).toBeInTheDocument();
    expect(within(unrateCard).getByText('Feb 1, 2025')).toBeInTheDocument();
  });

  test('shows an error on the card when the lookup fails', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    mockResponses({ CPIAUCSL: new Error('HTTP error! status: 500') });
    renderDashboard();

    const cpiCard = card('Consumer Price Index');
    expect(await within(cpiCard).findByText("Couldn't load this series")).toBeInTheDocument();
    expect(within(cpiCard).queryByText('No data yet')).not.toBeInTheDocument();
  });

  test.each([
    ['Dashboard.tsx', dashboardSource],
    ['LatestValueCard.tsx', cardSource],
  ])('keeps no hard-coded indicator values in %s', (_file, source) => {
    // Currency amounts, fractional percentages, ISO dates, and digits in JSX text.
    expect(source).not.toMatch(/\$\d/);
    expect(source).not.toMatch(/\d\.\d+%/);
    expect(source).not.toMatch(/\b(19|20)\d\d-\d\d-\d\d\b/);
    expect(source).not.toMatch(/>[^<>{}]*\d[^<>{}]*</);
  });

  test('shows a loading placeholder without a link until the lookup returns', () => {
    mockedExecuteGraphQL.mockImplementation(() => new Promise(() => {}));
    renderDashboard();

    const gdpCard = card('Gross Domestic Product');
    expect(within(gdpCard).getByRole('progressbar', { name: 'Loading' })).toBeInTheDocument();
    expect(within(gdpCard).queryByRole('link')).not.toBeInTheDocument();
  });

  test('names each card link by the card heading', async () => {
    mockResponses();
    renderDashboard();

    expect(
      await screen.findByRole('link', { name: 'Gross Domestic Product' })
    ).toHaveAttribute('href', `/series/${gdp.data.seriesByExternalId.id}`);
  });

  test('shows the raw value when it is not a decimal the formatter understands', async () => {
    mockResponses({
      GDP: {
        data: {
          seriesByExternalId: {
            ...gdp.data.seriesByExternalId,
            latestObservation: { ...gdp.data.seriesByExternalId.latestObservation, value: 'n/a' },
          },
        },
      },
    });
    renderDashboard();

    expect(await within(card('Gross Domestic Product')).findByText('n/a')).toBeInTheDocument();
  });

  test('keeps the quick links to the explorer and data sources', () => {
    mockResponses();
    renderDashboard();

    expect(screen.getByRole('button', { name: /explore all series/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /browse data sources/i })).toBeInTheDocument();
  });

  // ECO-246 (every-control crawl): these controls were removed along with the fake panels
  // they belonged to, rather than wired up, so the crawl's dashboard allowlist entries for
  // them are gone. Each one is checked here so a regression fails a unit test, not the crawl.
  test.each([
    'Employment Data',
    'Inflation Indicators',
    'GDP & Growth',
    'refresh data',
    'view details',
  ])('does not render the removed "%s" control', name => {
    mockResponses();
    renderDashboard();

    expect(screen.queryByRole('button', { name })).not.toBeInTheDocument();
  });

  test('does not render the collaboration badge button', () => {
    mockResponses();
    renderDashboard();

    expect(screen.queryByRole('button', { name: /collaboration/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/collaborator/i)).not.toBeInTheDocument();
  });

  test('never links a card to a hard-coded id like /series/gdp', async () => {
    mockResponses();
    renderDashboard();

    await screen.findByText('29,723.9');
    for (const link of screen.getAllByRole('link')) {
      expect(link.getAttribute('href')).not.toMatch(/^\/series\/[a-z-]+$/);
    }
  });
});
