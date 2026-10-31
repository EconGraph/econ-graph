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
import gdp from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/bea_gdp.json';
import unemployment from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/unemployment_rate.json';
import cpi from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/cpi.json';
import payrolls from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/nonfarm_payrolls.json';
import notFound from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/not_found.json';
import emptySeries from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/empty_series.json';
import nullValue from '../../test-utils/mocks/graphql-responses/get_series_latest_observation/null_value.json';

vi.mock('../../utils/graphql', async () => {
  const actual = await vi.importActual<typeof import('../../utils/graphql')>('../../utils/graphql');
  return { ...actual, executeGraphQL: vi.fn() };
});

const mockedExecuteGraphQL = vi.mocked(executeGraphQL);

type Fixture = { data: unknown };
const GDP_ID = 'bea_nipa/T10105.1.Q';
const FIXTURES: Record<string, Fixture> = {
  [GDP_ID]: gdp,
  LNS14000000: unemployment,
  CUSR0000SA0: cpi,
  CES0000000001: payrolls,
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

  test('lists the four BEA and BLS series from the config file', () => {
    expect(
      DASHBOARD_SERIES.map(({ label, sourceName, externalId }) => ({
        label,
        sourceName,
        externalId,
      }))
    ).toEqual([
      {
        label: 'Gross Domestic Product',
        sourceName: 'Bureau of Economic Analysis (BEA)',
        externalId: GDP_ID,
      },
      {
        label: 'Unemployment Rate',
        sourceName: 'Bureau of Labor Statistics (BLS)',
        externalId: 'LNS14000000',
      },
      {
        label: 'Consumer Price Index',
        sourceName: 'Bureau of Labor Statistics (BLS)',
        externalId: 'CUSR0000SA0',
      },
      {
        label: 'Nonfarm Payrolls',
        sourceName: 'Bureau of Labor Statistics (BLS)',
        externalId: 'CES0000000001',
      },
    ]);
  });

  test('looks each series up by source name and external id', async () => {
    mockResponses();
    renderDashboard();

    await screen.findByText('29.72');
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

    // BEA stores GDP in dollars (it applies UNIT_MULT): 29,723,864.9 million dollars is shown
    // as 29.72 trillion, labelled as such rather than with the stored "Current Dollars".
    const gdpCard = card('Gross Domestic Product');
    expect(await within(gdpCard).findByText('29.72')).toBeInTheDocument();
    expect(
      within(gdpCard).getByText('Trillions of dollars, seasonally adjusted annual rate')
    ).toBeInTheDocument();
    expect(within(gdpCard).queryByText('Current Dollars')).not.toBeInTheDocument();
    expect(within(gdpCard).getByText('Oct 1, 2024')).toHaveAttribute('datetime', '2024-10-01');
    expect(within(gdpCard).getByText(`BEA ${GDP_ID}`)).toBeInTheDocument();

    const unemploymentCard = card('Unemployment Rate');
    expect(await within(unemploymentCard).findByText('4.1')).toBeInTheDocument();
    expect(within(unemploymentCard).getByText('Percent, seasonally adjusted')).toBeInTheDocument();
    expect(within(unemploymentCard).getByText('Feb 1, 2025')).toBeInTheDocument();
    expect(within(unemploymentCard).getByText('BLS LNS14000000')).toBeInTheDocument();

    // 319.0825 rounds up to 319.083 on the digits; float rounding would give 319.082.
    const cpiCard = card('Consumer Price Index');
    expect(await within(cpiCard).findByText('319.083')).toBeInTheDocument();
    expect(within(cpiCard).getByText('Index 1982-84=100, seasonally adjusted')).toBeInTheDocument();
    expect(within(cpiCard).getByText('BLS CUSR0000SA0')).toBeInTheDocument();

    // BLS publishes payrolls in thousands; the value is shown as stored.
    const payrollsCard = card('Nonfarm Payrolls');
    expect(await within(payrollsCard).findByText('159,275')).toBeInTheDocument();
    expect(
      within(payrollsCard).getByText('Thousands of employees, seasonally adjusted')
    ).toBeInTheDocument();
    expect(within(payrollsCard).getByText('BLS CES0000000001')).toBeInTheDocument();

    expect(screen.queryByText(/FRED/)).not.toBeInTheDocument();
    expect(screen.queryByRole('article', { name: 'Federal Funds Rate' })).not.toBeInTheDocument();
  });

  test("shows each value's source notice with a terms link, outside the card's link", async () => {
    mockResponses();
    renderDashboard();

    const gdpCard = card('Gross Domestic Product');
    await within(gdpCard).findByText('29.72');
    const notice = within(gdpCard).getByTestId('source-notice');
    expect(notice).toHaveTextContent(
      'This product uses the Bureau of Economic Analysis (BEA) Data API but is not endorsed or certified by BEA.'
    );
    expect(within(notice).getByRole('link', { name: 'Terms of use' })).toHaveAttribute(
      'href',
      'https://apps.bea.gov/API/_pdf/bea_api_tos.pdf'
    );
    expect(
      within(gdpCard).getByRole('link', { name: 'Gross Domestic Product' })
    ).not.toContainElement(notice);
  });

  test('links each card to its series page', async () => {
    mockResponses();
    renderDashboard();

    await screen.findByText('29.72');
    expect(
      within(card('Gross Domestic Product')).getByRole('link', { name: 'Gross Domestic Product' })
    ).toHaveAttribute('href', `/series/${gdp.data.seriesByExternalId.id}`);
    expect(await within(card('Nonfarm Payrolls')).findByRole('link')).toHaveAttribute(
      'href',
      `/series/${payrolls.data.seriesByExternalId.id}`
    );
  });

  test('shows "No data yet" without a link when the series does not exist', async () => {
    mockResponses({ [GDP_ID]: notFound });
    renderDashboard();

    const gdpCard = card('Gross Domestic Product');
    expect(await within(gdpCard).findByText('No data yet')).toBeInTheDocument();
    expect(within(gdpCard).queryByRole('link')).not.toBeInTheDocument();
    // The other cards still load.
    expect(await within(card('Unemployment Rate')).findByText('4.1')).toBeInTheDocument();
  });

  test('shows "No data yet" without a link when the series has no observations', async () => {
    // series(id) treats a series with no observations as not found, so linking to it would
    // send the visitor to a "Series not found" page.
    mockResponses({ CES0000000001: emptySeries });
    renderDashboard();

    const payrollsCard = card('Nonfarm Payrolls');
    expect(await within(payrollsCard).findByText('No data yet')).toBeInTheDocument();
    expect(within(payrollsCard).queryByRole('link')).not.toBeInTheDocument();
  });

  test('says no value was reported when the latest observation has a null value', async () => {
    mockResponses({ LNS14000000: nullValue });
    renderDashboard();

    const unrateCard = card('Unemployment Rate');
    expect(await within(unrateCard).findByText('No value reported')).toBeInTheDocument();
    expect(within(unrateCard).getByText('Feb 1, 2025')).toBeInTheDocument();
  });

  test('shows an error on the card when the lookup fails', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    mockResponses({ CUSR0000SA0: new Error('HTTP error! status: 500') });
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

  test('shows the raw value, with the series units, when the formatter cannot read it', async () => {
    mockResponses({
      [GDP_ID]: {
        data: {
          seriesByExternalId: {
            ...gdp.data.seriesByExternalId,
            latestObservation: { ...gdp.data.seriesByExternalId.latestObservation, value: 'n/a' },
          },
        },
      },
    });
    renderDashboard();

    const gdpCard = card('Gross Domestic Product');
    expect(await within(gdpCard).findByText('n/a')).toBeInTheDocument();
    // The trillions label describes a scaled value; an unscaled one keeps the stored units.
    expect(within(gdpCard).getByText('Current Dollars')).toBeInTheDocument();
    expect(
      within(gdpCard).queryByText('Trillions of dollars, seasonally adjusted annual rate')
    ).not.toBeInTheDocument();
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

    await screen.findByText('29.72');
    for (const link of screen.getAllByRole('link')) {
      expect(link.getAttribute('href')).not.toMatch(/^\/series\/[a-z-]+$/);
    }
  });
});
