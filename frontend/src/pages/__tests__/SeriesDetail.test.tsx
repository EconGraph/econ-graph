/**
 * Series page tests against recorded GraphQL responses (fixtures).
 *
 * The real hooks run; only executeGraphQL is replaced, so these tests check the queries
 * and variables the page sends as well as what it renders.
 */

import React from 'react';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import { useParams } from 'react-router-dom';
import { TestProviders } from '../../test-utils/test-providers';
import { executeGraphQL, GraphQLRequest, QUERIES } from '../../utils/graphql';
import SeriesDetail from '../SeriesDetail';
import {
  EMPTY_ID,
  UNKNOWN_ID,
  UNRATE_ID,
  allNullTransformed,
  noObservations,
  seriesResponses,
  unrateLevels,
  unrateYearOverYear,
} from './fixtures/seriesDetail';

// setupTests mocks the hooks globally; this page is tested with the real ones.
vi.mock('../../hooks/useSeriesData', async () => vi.importActual('../../hooks/useSeriesData'));

vi.mock('../../utils/graphql', async () => {
  const actual = await vi.importActual<typeof import('../../utils/graphql')>(
    '../../utils/graphql'
  );
  return { ...actual, executeGraphQL: vi.fn() };
});

vi.mock('react-router-dom', () => ({
  useParams: vi.fn(),
  useNavigate: () => vi.fn(),
}));

// Chart.js needs a canvas; render the points it would plot instead.
vi.mock('react-chartjs-2', () => ({
  Line: ({ data, options }: any) => (
    <div
      data-testid='line-chart'
      data-points={JSON.stringify(data.datasets[0].data.map((p: any) => [p.date, p.y]))}
      data-title={options.plugins.title.text}
    />
  ),
}));

vi.mock('chart.js', () => ({
  Chart: { register: vi.fn() },
  CategoryScale: {},
  LinearScale: {},
  TimeScale: {},
  PointElement: {},
  LineElement: {},
  Title: {},
  Tooltip: {},
  Legend: {},
}));
vi.mock('chartjs-adapter-date-fns', () => ({}));

// Dates must read as calendar dates; west of UTC is where parsing them as UTC goes wrong.
const originalTz = process.env.TZ;
beforeAll(() => {
  process.env.TZ = 'America/Los_Angeles';
});
afterAll(() => {
  if (originalTz === undefined) delete process.env.TZ;
  else process.env.TZ = originalTz;
});

const mockExecute = vi.mocked(executeGraphQL);
const mockUseParams = vi.mocked(useParams);

type DataResponse = { seriesData: { nodes: unknown[]; totalCount: number } };

/**
 * Answer GraphQL requests from fixtures.
 * @param dataByTransformation - The seriesData response for each transformation enum value.
 * @param failOn - Query to fail with a GraphQL error instead.
 */
function serve(
  dataByTransformation: Record<string, DataResponse> = { NONE: unrateLevels },
  failOn?: string
) {
  mockExecute.mockImplementation(async (request: GraphQLRequest) => {
    if (failOn && request.query === failOn) {
      throw new Error('Internal server error');
    }
    if (request.query === QUERIES.GET_SERIES_DETAIL) {
      return { data: seriesResponses[request.variables?.id] ?? { series: null } };
    }
    if (request.query === QUERIES.GET_SERIES_DATA) {
      const response = dataByTransformation[request.variables?.transformation];
      if (!response) throw new Error(`no fixture for ${request.variables?.transformation}`);
      return { data: response };
    }
    throw new Error('unexpected query');
  });
}

function renderPage(id: string | undefined) {
  mockUseParams.mockReturnValue({ id });
  return render(
    <TestProviders>
      <SeriesDetail />
    </TestProviders>
  );
}

const plottedPoints = () => JSON.parse(screen.getByTestId('line-chart').dataset.points ?? '[]');

const dataRequests = () =>
  mockExecute.mock.calls
    .map(([request]) => request)
    .filter(request => request.query === QUERIES.GET_SERIES_DATA);

async function chooseTransformation(label: string) {
  const user = userEvent.setup();
  await user.click(screen.getByRole('combobox'));
  await user.click(await screen.findByRole('option', { name: label }));
}

describe('SeriesDetail', () => {
  beforeEach(() => {
    mockExecute.mockReset();
  });

  test('shows the series and its observations from the API', async () => {
    serve();
    renderPage(UNRATE_ID);

    expect(screen.getByTestId('series-detail-loading')).toBeInTheDocument();
    expect(
      await screen.findByRole('heading', { name: 'Unemployment Rate', level: 1 })
    ).toBeInTheDocument();
    expect(screen.getByText('UNRATE')).toBeInTheDocument();
    expect(screen.getByText('Seasonally Adjusted')).toBeInTheDocument();

    await screen.findByTestId('line-chart');
    expect(plottedPoints()).toEqual([
      ['2024-01-01', 3.7],
      ['2024-02-01', 3.9],
      ['2024-03-01', 3.8],
    ]);

    // Recent data lists newest first, dates read as calendar dates in any timezone.
    const rows = within(screen.getByRole('table', { name: 'Recent observations' })).getAllByRole(
      'row'
    );
    expect(rows[1]).toHaveTextContent('Mar 1, 2024');
    expect(rows[1]).toHaveTextContent('3.80');
    expect(rows[1]).toHaveTextContent('Original');
    expect(rows[3]).toHaveTextContent('Jan 1, 2024');
  });

  test('asks the backend for the latest revision of each date, untransformed', async () => {
    serve();
    renderPage(UNRATE_ID);
    await screen.findByTestId('line-chart');

    expect(dataRequests()).toHaveLength(1);
    expect(dataRequests()[0].variables).toEqual({
      seriesId: UNRATE_ID,
      filter: { startDate: undefined, endDate: undefined, latestRevisionOnly: true },
      transformation: 'NONE',
      first: 10000,
    });
  });

  test.each([
    ['Year-over-Year', 'YEAR_OVER_YEAR'],
    ['Quarter-over-Quarter', 'QUARTER_OVER_QUARTER'],
    ['Month-over-Month', 'MONTH_OVER_MONTH'],
    ['Change since first observation', 'PERCENT_CHANGE'],
  ])('choosing %s sends %s to the backend', async (label, enumValue) => {
    serve({ NONE: unrateLevels, [enumValue]: unrateYearOverYear });
    renderPage(UNRATE_ID);
    await screen.findByTestId('line-chart');

    await chooseTransformation(label);

    await waitFor(() => expect(dataRequests().slice(-1)[0]?.variables?.transformation).toBe(enumValue));
    await waitFor(() => expect(plottedPoints()).toContainEqual(['2024-02-01', 5.4054]));
  });

  test('plots the transformed values the backend returns, unchanged', async () => {
    serve({ NONE: unrateLevels, YEAR_OVER_YEAR: unrateYearOverYear });
    renderPage(UNRATE_ID);
    await screen.findByTestId('line-chart');

    await chooseTransformation('Year-over-Year');

    await waitFor(() =>
      expect(plottedPoints()).toEqual([
        ['2024-01-01', null],
        ['2024-02-01', 5.4054],
        ['2024-03-01', -2.5641],
      ])
    );
    expect(screen.getByTestId('line-chart').dataset.title).toBe(
      'Unemployment Rate (Year-over-Year % Change)'
    );
  });

  test('says when a transformation has no values', async () => {
    serve({ NONE: unrateLevels, MONTH_OVER_MONTH: allNullTransformed });
    renderPage(UNRATE_ID);
    await screen.findByTestId('line-chart');

    await chooseTransformation('Month-over-Month');

    expect(
      await screen.findByText(/Not enough observations to compute month-over-month % change/)
    ).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole('button', { name: 'Show levels' }));
    await screen.findByTestId('line-chart');
  });

  test('shows an empty state for a series with no observations', async () => {
    serve({ NONE: noObservations });
    renderPage(EMPTY_ID);

    expect(await screen.findByText('No observations for this series yet.')).toBeInTheDocument();
    expect(screen.getByText(/after the next successful crawl of FRED/)).toBeInTheDocument();
    expect(screen.queryByTestId('line-chart')).not.toBeInTheDocument();
    expect(screen.queryByRole('table', { name: 'Recent observations' })).not.toBeInTheDocument();
  });

  test('shows not found for an unknown series id', async () => {
    serve();
    renderPage(UNKNOWN_ID);

    expect(await screen.findByText(/Series not found/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Back to Explorer' })).toBeInTheDocument();
    expect(dataRequests()).toHaveLength(0);
  });

  test('shows not found for an id that is not a UUID, without calling the API', async () => {
    serve();
    renderPage('gdp-real');

    expect(await screen.findByText(/Series not found/)).toBeInTheDocument();
    expect(mockExecute).not.toHaveBeenCalled();
  });

  test('shows an error with retry when the series query fails', async () => {
    serve(undefined, QUERIES.GET_SERIES_DETAIL);
    renderPage(UNRATE_ID);

    expect(await screen.findByText(/Could not load this series/)).toBeInTheDocument();

    serve();
    await userEvent.setup().click(screen.getByRole('button', { name: 'Retry' }));
    expect(
      await screen.findByRole('heading', { name: 'Unemployment Rate', level: 1 })
    ).toBeInTheDocument();
  });

  test('shows an error in place of the chart when the data query fails', async () => {
    serve(undefined, QUERIES.GET_SERIES_DATA);
    renderPage(UNRATE_ID);

    expect(
      await screen.findByText(/Could not load observations for this series/)
    ).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Unemployment Rate', level: 1 })).toBeInTheDocument();
  });

  test('a failed transformation can go back to levels', async () => {
    serve({ NONE: unrateLevels });
    renderPage(UNRATE_ID);
    await screen.findByTestId('line-chart');

    await chooseTransformation('Year-over-Year');

    expect(
      await screen.findByText(/Could not load observations for this series/)
    ).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole('button', { name: 'Show levels' }));
    await waitFor(() => expect(plottedPoints()).toContainEqual(['2024-03-01', 3.8]));
  });

  test('the selector shows a new choice while its data loads', async () => {
    let release: () => void = () => {};
    const pending = new Promise<void>(resolve => {
      release = resolve;
    });
    serve({ NONE: unrateLevels, YEAR_OVER_YEAR: unrateYearOverYear });
    const answer = mockExecute.getMockImplementation()!;
    mockExecute.mockImplementation(async request => {
      if (request.variables?.transformation === 'YEAR_OVER_YEAR') await pending;
      return answer(request);
    });
    renderPage(UNRATE_ID);
    await screen.findByTestId('line-chart');

    await chooseTransformation('Year-over-Year');

    expect(await screen.findByLabelText('Loading transformation')).toBeInTheDocument();
    expect(screen.getByRole('combobox')).toHaveTextContent('Year-over-Year');
    expect(screen.getByTestId('line-chart').dataset.title).toBe('Unemployment Rate');

    release();
    await waitFor(() =>
      expect(screen.getByTestId('line-chart').dataset.title).toBe(
        'Unemployment Rate (Year-over-Year % Change)'
      )
    );
  });

  test('shows an error when no series id is in the URL', () => {
    renderPage(undefined);
    expect(screen.getByText('No series ID provided')).toBeInTheDocument();
  });

  test('has no collaboration panel', async () => {
    serve();
    renderPage(UNRATE_ID);
    await screen.findByTestId('line-chart');

    // Public annotations are read-only (SeriesDetailAnnotations.test.tsx); nothing to share.
    expect(screen.queryByText(/collaborat/i)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /share/i })).not.toBeInTheDocument();
  });
});
