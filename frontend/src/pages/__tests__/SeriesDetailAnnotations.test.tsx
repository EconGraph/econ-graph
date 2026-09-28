/**
 * Series page annotation tests against GraphQL response fixtures in the backend's shape.
 *
 * The real hooks run; only executeGraphQL is replaced, so these tests check the query and
 * variables the page sends as well as what the chart and the list show.
 */

import React from 'react';
import { render, screen, within } from '@testing-library/react';
import { createTheme } from '@mui/material/styles';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import { useParams } from 'react-router-dom';
import { TestProviders } from '../../test-utils/test-providers';
import { useAuth } from '../../contexts/AuthContext';
import { executeGraphQL, GraphQLRequest, QUERIES } from '../../utils/graphql';
import { parseIsoDate } from '../../utils/dates';
import SeriesDetail from '../SeriesDetail';
import { GDP_ID, gdpAnnotations, gdpDetail, gdpLevels } from './fixtures/seriesAnnotations';

// setupTests mocks these hooks globally; this page is tested with the real ones.
vi.mock('../../hooks/useSeriesData', async () => vi.importActual('../../hooks/useSeriesData'));
vi.mock('../../hooks/useSeriesAnnotations', async () =>
  vi.importActual('../../hooks/useSeriesAnnotations')
);

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

// Chart.js needs a canvas. Render the annotations the chart would draw instead, each as a
// button that fires the plugin's click handler.
vi.mock('react-chartjs-2', () => ({
  Line: ({ options }: any) => {
    const drawn: Record<string, any> = options.plugins.annotation.annotations;
    return (
      <div data-testid='line-chart'>
        {Object.entries(drawn).map(([id, a]) => (
          <button
            key={id}
            type='button'
            data-testid='chart-annotation'
            data-x={a.xMin}
            data-color={a.borderColor}
            onClick={() => a.click?.()}
          >
            {a.label.content}
          </button>
        ))}
      </div>
    );
  },
}));

const mockExecute = vi.mocked(executeGraphQL);
const mockUseParams = vi.mocked(useParams);

/**
 * Answer GraphQL requests from fixtures.
 * @param annotations - The annotationsForSeries response, or an Error to fail with.
 */
function serve(annotations: typeof gdpAnnotations | Error = gdpAnnotations) {
  mockExecute.mockImplementation(async (request: GraphQLRequest) => {
    if (request.query === QUERIES.GET_SERIES_DETAIL) return { data: gdpDetail };
    if (request.query === QUERIES.GET_SERIES_DATA) return { data: gdpLevels };
    if (request.query === QUERIES.GET_ANNOTATIONS_FOR_SERIES) {
      if (annotations instanceof Error) throw annotations;
      return { data: annotations };
    }
    throw new Error('unexpected query');
  });
}

function renderPage() {
  mockUseParams.mockReturnValue({ id: GDP_ID });
  return render(
    <TestProviders>
      <SeriesDetail />
    </TestProviders>
  );
}

const annotationRequests = () =>
  mockExecute.mock.calls
    .map(([request]) => request)
    .filter(request => request.query === QUERIES.GET_ANNOTATIONS_FOR_SERIES);

// Each annotation's row is the first button in its list item; the rest are its controls.
const listRows = () =>
  within(screen.getByRole('list', { name: 'Annotations' }))
    .getAllByRole('listitem')
    .map(item => within(item).getAllByRole('button')[0]);

describe('SeriesDetail annotations', () => {
  beforeEach(() => {
    mockExecute.mockReset();
    // A build without sign-in: the public, read-only view. Signed-in editing is tested in
    // SeriesDetailAnnotationEditing.test.tsx.
    vi.mocked(useAuth).mockReturnValue({
      user: null,
      isAuthenticated: false,
      isLoading: false,
      isConfigured: false,
      accountUrl: null,
      error: null,
      signIn: vi.fn(),
      signOut: vi.fn(),
      completeSignIn: vi.fn(),
      clearError: vi.fn(),
    });
  });

  test('asks for the series annotations without a user id', async () => {
    serve();
    renderPage();

    await screen.findByRole('list', { name: 'Annotations' });
    expect(annotationRequests()).toHaveLength(1);
    // The viewer comes from the access token, never from a query argument.
    expect(annotationRequests()[0].variables).toEqual({ seriesId: GDP_ID });
    expect(QUERIES.GET_ANNOTATIONS_FOR_SERIES).toMatch(/annotationsForSeries\(seriesId: \$seriesId\)/);
  });

  test('shows no annotate or edit controls when sign-in is not configured', async () => {
    serve();
    renderPage();

    await screen.findByRole('list', { name: 'Annotations' });
    expect(screen.queryByRole('button', { name: /add annotation/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /sign in/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^edit /i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^delete /i })).not.toBeInTheDocument();
  });

  test('draws public annotations on the chart at their dates', async () => {
    serve();
    renderPage();

    await screen.findByRole('list', { name: 'Annotations' });
    const drawn = screen.getAllByTestId('chart-annotation');
    expect(drawn.map(el => [el.textContent, Number(el.dataset.x)])).toEqual([
      ['Reopening rebound', parseIsoDate('2020-07-01').getTime()],
      ['Lockdown trough', parseIsoDate('2020-04-01').getTime()],
      ['Pre-pandemic peak', parseIsoDate('2020-01-01').getTime()],
    ]);
    // The author's colour is kept; a missing or non-hex colour falls back to the theme's.
    const fallback = createTheme().palette.warning.main;
    expect(drawn.map(el => el.dataset.color)).toEqual([fallback, '#d32f2f', fallback]);
  });

  test('lists public annotations beside the chart, newest date first', async () => {
    serve();
    renderPage();

    await screen.findByRole('list', { name: 'Annotations' });
    const rows = listRows();
    expect(rows).toHaveLength(3);
    within(rows[0]).getByText('Reopening rebound');
    within(rows[0]).getByText('Jul 1, 2020');
    within(rows[1]).getByText('Lockdown trough');
    within(rows[1]).getByText('Largest quarterly fall on record.');
    within(rows[2]).getByText('Pre-pandemic peak');
  });

  test('clicking an annotation on the chart highlights it in the list', async () => {
    serve();
    renderPage();
    const user = userEvent.setup();

    await screen.findByRole('list', { name: 'Annotations' });
    await user.click(
      within(screen.getByTestId('line-chart')).getByRole('button', { name: 'Lockdown trough' })
    );

    const selected = () =>
      listRows().filter(row => row.getAttribute('aria-current') === 'true');
    expect(selected()).toHaveLength(1);
    expect(selected()[0]).toHaveTextContent('Lockdown trough');

    // Clicking a row selects it too.
    await user.click(listRows()[2]);
    expect(selected()).toHaveLength(1);
    expect(selected()[0]).toHaveTextContent('Pre-pandemic peak');
  });

  test('says so when the series has no annotations', async () => {
    serve({ annotationsForSeries: [] });
    renderPage();

    expect(await screen.findByText('No annotations on this series yet.')).toBeInTheDocument();
    await screen.findByTestId('line-chart');
    expect(screen.queryAllByTestId('chart-annotation')).toHaveLength(0);
  });

  test('keeps the chart when annotations fail to load, and offers a retry', async () => {
    serve(new Error('Internal server error'));
    renderPage();

    expect(
      await screen.findByText(/Could not load annotations\. Internal server error/)
    ).toBeInTheDocument();
    expect(screen.getByTestId('line-chart')).toBeInTheDocument();

    serve();
    await userEvent.setup().click(screen.getByRole('button', { name: 'Retry' }));
    expect(await screen.findByRole('list', { name: 'Annotations' })).toBeInTheDocument();
  });
});
