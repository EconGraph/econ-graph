// REQUIREMENT: The explorer's URL and its search form stay in step on same-route navigation
// PURPOSE: A link to /explore while already on /explore (the sidebar's FRED Data), browser
// back/forward and direct loads all show the URL's query and source, not the previous state

import React from 'react';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import SeriesExplorer from '../SeriesExplorer';

// setupTests.vitest.ts replaces react-router-dom with a stub whose search params never change.
// These tests are about navigation, so they use the real router.
vi.mock('react-router-dom', async () => vi.importActual('react-router-dom'));

const { Link, MemoryRouter, Route, Routes, useLocation, useNavigate } =
  await vi.importActual<typeof import('react-router-dom')>('react-router-dom');

const dataSources = [
  { id: 'fred', name: 'Federal Reserve Economic Data (FRED)' },
  { id: 'bea', name: 'Bureau of Economic Analysis' },
];

const allSeries = [
  { id: 'fred-gdp', title: 'Gross Domestic Product', sourceId: 'fred', frequency: 'Quarterly' },
  { id: 'fred-unrate', title: 'Unemployment Rate', sourceId: 'fred', frequency: 'Monthly' },
  { id: 'bea-gdp', title: 'GDP by Industry', sourceId: 'bea', frequency: 'Annual' },
];

const matches = (query: string, sourceId?: string) =>
  allSeries.filter(
    series =>
      (!sourceId || series.sourceId === sourceId) &&
      (query.toLowerCase() === 'gdp'
        ? /gdp|gross domestic/i.test(series.title)
        : series.title.toLowerCase().includes(query.toLowerCase()))
  );

vi.mock('../../hooks/useSeriesData', () => ({
  useDataSources: () => ({ data: dataSources, isPending: false, error: null }),
  useSeriesSearch: (query: string, filters?: { sourceId?: string }, enabled = true) => ({
    data: enabled && query.trim().length >= 2 ? matches(query.trim(), filters?.sourceId) : [],
    isLoading: false,
    isFetching: false,
    error: null,
  }),
  useSeriesBrowse: (filters: { sourceId?: string }, enabled = true) => {
    const series = enabled && filters.sourceId ? matches('', filters.sourceId) : undefined;
    return {
      data: series && { series, totalCount: series.length },
      isLoading: false,
      isFetching: false,
      error: null,
    };
  },
}));

// Shows the router's current URL, and stands in for the sidebar and the browser's buttons
const Chrome: React.FC = () => {
  const location = useLocation();
  const navigate = useNavigate();
  return (
    <>
      <output data-testid='url'>{`${location.pathname}${location.search}`}</output>
      <Link to='/explore?source=Federal Reserve Economic Data'>FRED Data</Link>
      <button onClick={() => navigate(-1)}>Back</button>
      <button onClick={() => navigate(1)}>Forward</button>
    </>
  );
};

function renderExplorerAt(url: string) {
  return render(
    <ThemeProvider theme={createTheme()}>
      <MemoryRouter initialEntries={[url]}>
        <Chrome />
        <Routes>
          <Route path='/explore' element={<SeriesExplorer />} />
        </Routes>
      </MemoryRouter>
    </ThemeProvider>
  );
}

const currentUrl = () => new URL(screen.getByTestId('url').textContent ?? '', 'http://x');
const searchBox = () => screen.getByPlaceholderText(/search economic series/i);
const sourceSelect = () => screen.getByRole('combobox', { name: /source/i });
const resultTitles = () =>
  allSeries.map(series => series.title).filter(title => screen.queryByText(title));

async function expectFredBrowse() {
  await waitFor(() => {
    expect(currentUrl().searchParams.get('source')).toBe('Federal Reserve Economic Data');
    expect(currentUrl().searchParams.get('q')).toBeNull();
    expect(searchBox()).toHaveValue('');
    expect(sourceSelect()).toHaveTextContent('Federal Reserve Economic Data (FRED)');
    // Every result is a FRED series
    expect(resultTitles()).toEqual(['Gross Domestic Product', 'Unemployment Rate']);
  });
}

async function expectAllSourceGdpSearch() {
  await waitFor(() => {
    expect(currentUrl().searchParams.get('q')).toBe('GDP');
    expect(currentUrl().searchParams.get('source')).toBeNull();
    expect(searchBox()).toHaveValue('GDP');
    expect(sourceSelect()).not.toHaveTextContent('Federal Reserve');
    expect(resultTitles()).toEqual(['Gross Domestic Product', 'GDP by Industry']);
  });
}

describe('the explorer follows navigation to another /explore URL (ECO-511)', () => {
  test('a GDP search, then the sidebar FRED Data link, browses FRED without remounting', async () => {
    const user = userEvent.setup();
    renderExplorerAt('/explore?q=GDP');
    await expectAllSourceGdpSearch();
    const searchInput = searchBox();

    await user.click(screen.getByRole('link', { name: 'FRED Data' }));

    await expectFredBrowse();
    // Same page instance: the link navigated within the explorer rather than reloading it
    expect(searchBox()).toBe(searchInput);
  });

  test('back and forward restore each URL\'s query and source', async () => {
    const user = userEvent.setup();
    renderExplorerAt('/explore?q=GDP');
    await expectAllSourceGdpSearch();
    await user.click(screen.getByRole('link', { name: 'FRED Data' }));
    await expectFredBrowse();

    await user.click(screen.getByRole('button', { name: 'Back' }));
    await expectAllSourceGdpSearch();

    await user.click(screen.getByRole('button', { name: 'Forward' }));
    await expectFredBrowse();
  });

  test('typing after a navigation writes the new query into the URL with the source kept', async () => {
    const user = userEvent.setup();
    renderExplorerAt('/explore?q=GDP');
    await expectAllSourceGdpSearch();
    await user.click(screen.getByRole('link', { name: 'FRED Data' }));
    await expectFredBrowse();

    await user.type(searchBox(), 'unemployment');

    await waitFor(() => {
      expect(currentUrl().searchParams.get('q')).toBe('unemployment');
      expect(currentUrl().searchParams.get('source')).toBe('Federal Reserve Economic Data');
      expect(resultTitles()).toEqual(['Unemployment Rate']);
    });
    expect(searchBox()).toHaveValue('unemployment');
  });

  test('a direct load of the FRED Data URL browses FRED', async () => {
    renderExplorerAt('/explore?source=Federal%20Reserve%20Economic%20Data');
    await expectFredBrowse();
  });

  test('clearing the search in the page updates the URL and stays cleared', async () => {
    const user = userEvent.setup();
    renderExplorerAt('/explore?q=GDP&source=fred');
    await waitFor(() => expect(searchBox()).toHaveValue('GDP'));

    await user.clear(searchBox());
    await act(async () => {
      await new Promise(resolve => setTimeout(resolve, 400));
    });

    expect(currentUrl().searchParams.get('q')).toBeNull();
    expect(currentUrl().searchParams.get('source')).toBe('fred');
    expect(searchBox()).toHaveValue('');
  });
});
