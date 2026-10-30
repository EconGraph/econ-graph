// REQUIREMENT: The explorer's page and sort are in the URL (ECO-732)
// PURPOSE: Back from a series, forward, a reload and a shared link show the same results page
// in the same order, a source's whole listing can be paged through, and a bad or out-of-range
// `page` or `sort` falls back safely

import React from 'react';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import SeriesExplorer, { parsePageParam, parseSortParam } from '../SeriesExplorer';

// setupTests.vitest.ts replaces react-router-dom with a stub whose search params never change.
// These tests are about the URL, so they use the real router.
vi.mock('react-router-dom', async () => vi.importActual('react-router-dom'));

const { MemoryRouter, Route, Routes, useLocation, useNavigate } =
  await vi.importActual<typeof import('react-router-dom')>('react-router-dom');

const dataSources = [{ id: 'fred', name: 'Federal Reserve Economic Data (FRED)' }];

// 45 rate series: three pages of 20. Search returns them in this (relevance) order, which is
// the reverse of title order; browsing returns them by title.
const rateSeries = Array.from({ length: 45 }, (_, i) => {
  const n = String(45 - i).padStart(2, '0');
  return {
    id: `rate-${n}`,
    title: `Rate ${n}`,
    sourceId: 'fred',
    // A spelling the backend's frequency filter doesn't know, on the second browse page
    frequency: n >= '30' && n <= '33' ? 'Weekly, Ending Friday' : 'Monthly',
    lastUpdated: '2024-01-15T00:00:00Z',
  };
});
const byTitle = [...rateSeries].sort((a, b) => a.title.localeCompare(b.title));

const browseCalls = vi.hoisted(() => [] as Array<{ limit?: number; offset?: number }>);

vi.mock('../../hooks/useSeriesData', () => ({
  useDataSources: () => ({ data: dataSources, isPending: false, error: null }),
  useSeriesSearch: (query: string, _filters?: unknown, enabled = true) => ({
    data: enabled && query.trim().length >= 2 ? rateSeries : undefined,
    isLoading: false,
    isFetching: false,
    error: null,
  }),
  // Like the backend's seriesList: a page of the source's series by title, and the total
  useSeriesBrowse: (
    filters: { sourceId?: string; limit?: number; offset?: number },
    enabled = true
  ) => {
    if (!enabled || !filters.sourceId) return { data: undefined, isLoading: false, error: null };
    browseCalls.push({ limit: filters.limit, offset: filters.offset });
    const offset = filters.offset ?? 0;
    return {
      data: {
        series: byTitle.slice(offset, offset + (filters.limit ?? 50)),
        totalCount: byTitle.length,
      },
      isLoading: false,
      isFetching: false,
      error: null,
    };
  },
}));

// Shows the router's current URL, and stands in for the browser's buttons
const Chrome: React.FC = () => {
  const location = useLocation();
  const navigate = useNavigate();
  return (
    <>
      <output data-testid='url'>{`${location.pathname}${location.search}`}</output>
      <button onClick={() => navigate(-1)}>Back</button>
      <button onClick={() => navigate(1)}>Forward</button>
    </>
  );
};

// The series page, as far as these tests need it: where its Explore link would go
const SeriesStub: React.FC = () => {
  const state = useLocation().state as { explorerUrl?: string } | null;
  return <output data-testid='explorer-url'>{state?.explorerUrl ?? ''}</output>;
};

function renderAt(url: string) {
  return render(
    <ThemeProvider theme={createTheme()}>
      <MemoryRouter initialEntries={[url]}>
        <Chrome />
        <Routes>
          <Route path='/explore' element={<SeriesExplorer />} />
          <Route path='/series/:id' element={<SeriesStub />} />
        </Routes>
      </MemoryRouter>
    </ThemeProvider>
  );
}

const lastBrowse = () => browseCalls[browseCalls.length - 1];
const currentUrl = () => new URL(screen.getByTestId('url').textContent ?? '', 'http://x');
const shownTitles = () =>
  screen.queryAllByText(/^Rate \d\d$/).map(element => element.textContent);
const currentPageButton = () =>
  screen.getByRole('navigation').querySelector('[aria-current="page"]');

describe('the explorer keeps its page and sort in the URL (ECO-732)', () => {
  const scrollTo = vi.fn();
  const originalScrollTo = window.scrollTo;

  beforeEach(() => {
    browseCalls.length = 0;
    scrollTo.mockClear();
    window.scrollTo = scrollTo as unknown as typeof window.scrollTo;
  });

  afterEach(() => {
    window.scrollTo = originalScrollTo;
  });

  test('a page change goes into the URL and scrolls to the top', async () => {
    const user = userEvent.setup();
    renderAt('/explore?q=rate');
    await waitFor(() => expect(shownTitles()[0]).toBe('Rate 45'));

    await user.click(screen.getByRole('button', { name: 'Go to page 3' }));

    await waitFor(() => expect(currentUrl().searchParams.get('page')).toBe('3'));
    expect(currentUrl().searchParams.get('q')).toBe('rate');
    expect(shownTitles()).toEqual(['Rate 05', 'Rate 04', 'Rate 03', 'Rate 02', 'Rate 01']);
    expect(scrollTo).toHaveBeenCalledWith(0, 0);
  });

  test('Back from a series returns to the same page, and its Explore link knows that page', async () => {
    const user = userEvent.setup();
    renderAt('/explore?q=rate');
    await waitFor(() => expect(shownTitles()).toHaveLength(20));
    await user.click(screen.getByRole('button', { name: 'Go to page 2' }));
    await waitFor(() => expect(currentUrl().searchParams.get('page')).toBe('2'));

    const card = screen.getByText('Rate 25').closest('.MuiCard-root') as HTMLElement;
    await user.click(within(card).getByRole('button', { name: 'View Details' }));
    await waitFor(() => expect(currentUrl().pathname).toBe('/series/rate-25'));
    expect(screen.getByTestId('explorer-url')).toHaveTextContent('/explore?q=rate&page=2');

    await user.click(screen.getByRole('button', { name: 'Back' }));
    await waitFor(() => expect(shownTitles()[0]).toBe('Rate 25'));
    expect(currentUrl().search).toBe('?q=rate&page=2');
    expect(currentPageButton()).toHaveTextContent('2');

    await user.click(screen.getByRole('button', { name: 'Forward' }));
    await waitFor(() => expect(currentUrl().pathname).toBe('/series/rate-25'));
  });

  test('a sort goes into the URL and starts again at page 1', async () => {
    const user = userEvent.setup();
    renderAt('/explore?q=rate&page=2');
    await waitFor(() => expect(shownTitles()[0]).toBe('Rate 25'));

    await user.click(screen.getByRole('button', { name: 'Advanced Search' }));
    await user.click(screen.getByRole('combobox', { name: 'Sort By' }));
    await user.click(screen.getByRole('option', { name: 'Title' }));

    await waitFor(() => expect(currentUrl().search).toBe('?q=rate&sort=title'));
    expect(shownTitles()[0]).toBe('Rate 01');
  });

  test('a loaded URL (a reload or a shared link) shows its sort and page', async () => {
    renderAt('/explore?q=rate&sort=title&page=2');

    await waitFor(() => expect(shownTitles()[0]).toBe('Rate 21'));
    expect(shownTitles()).toHaveLength(20);
    expect(currentPageButton()).toHaveTextContent('2');
    expect(currentUrl().search).toBe('?q=rate&sort=title&page=2');
  });

  test.each([
    ['page=abc', 'page'],
    ['page=0', 'page'],
    ['page=-2', 'page'],
    ['page=1.5', 'page'],
    ['page=', 'page'],
    ['sort=bogus', 'sort'],
    ['sort=relevance', 'sort'],
  ])('an invalid or default %s shows the default and is dropped from the URL', async param => {
    renderAt(`/explore?q=rate&${param}`);

    await waitFor(() => expect(currentUrl().search).toBe('?q=rate'));
    expect(shownTitles()[0]).toBe('Rate 45');
  });

  test('a page past the last shows the last page', async () => {
    renderAt('/explore?q=rate&page=99');

    await waitFor(() => expect(currentUrl().searchParams.get('page')).toBe('3'));
    expect(shownTitles()).toEqual(['Rate 05', 'Rate 04', 'Rate 03', 'Rate 02', 'Rate 01']);
  });

  test('browsing a source pages through all of it, a page at a time from the backend', async () => {
    const user = userEvent.setup();
    renderAt('/explore?source=fred');

    expect(await screen.findByText('1–20 of 45 series, by title')).toBeInTheDocument();
    expect(lastBrowse()).toEqual({ limit: 20, offset: 0 });

    await user.click(screen.getByRole('button', { name: 'Go to page 3' }));

    expect(await screen.findByText('41–45 of 45 series, by title')).toBeInTheDocument();
    expect(lastBrowse()).toEqual({ limit: 20, offset: 40 });
    expect(shownTitles()).toEqual(['Rate 41', 'Rate 42', 'Rate 43', 'Rate 44', 'Rate 45']);
    expect(currentUrl().search).toBe('?source=fred&page=3');
  });

  test('a browse past its last page (or one with fewer series for this viewer) moves to the last', async () => {
    renderAt('/explore?source=fred&page=99');

    expect(await screen.findByText('41–45 of 45 series, by title')).toBeInTheDocument();
    expect(lastBrowse()).toEqual({ limit: 20, offset: 40 });
    expect(currentUrl().search).toBe('?source=fred&page=3');
  });

  test('while browsing, the order is the listing\'s, by title, and the sort control says so', async () => {
    const user = userEvent.setup();
    renderAt('/explore?source=fred&sort=lastUpdated');
    expect(await screen.findByText('1–20 of 45 series, by title')).toBeInTheDocument();
    await waitFor(() => expect(currentUrl().search).toBe('?source=fred'));

    await user.click(screen.getByRole('button', { name: 'Advanced Search' }));
    const sort = screen.getByRole('combobox', { name: 'Sort By' });
    expect(sort).toHaveTextContent('Title');
    expect(sort).toHaveAttribute('aria-disabled', 'true');
    expect(shownTitles()[0]).toBe('Rate 01');
  });

  test('typing a new search starts again at page 1, and drops the page from the URL', async () => {
    const user = userEvent.setup();
    renderAt('/explore?q=rate&page=3');
    await waitFor(() => expect(shownTitles()[0]).toBe('Rate 05'));

    await user.type(screen.getByPlaceholderText(/search economic series/i), 's');

    await waitFor(() => expect(currentUrl().search).toBe('?q=rates'));
    await waitFor(() => expect(shownTitles()[0]).toBe('Rate 45'));
  });

  test('browsing with a frequency the backend does not know lists the first 100 and pages them here', async () => {
    renderAt('/explore?source=fred&frequency=Weekly%2C+Ending+Friday');

    expect(await screen.findByText('4 matching series of 45, by title')).toBeInTheDocument();
    expect(lastBrowse()).toEqual({ limit: 100, offset: 0 });
    expect(shownTitles()).toEqual(['Rate 30', 'Rate 31', 'Rate 32', 'Rate 33']);
  });
});

describe('URL parameter parsing', () => {
  test.each([
    [null, 1],
    ['', 1],
    ['1', 1],
    ['7', 7],
    ['007', 7],
    ['0', 1],
    ['-3', 1],
    ['2.5', 1],
    ['1e3', 1],
    [' 4', 1],
    ['abc', 1],
    ['99999999999', 100_000],
  ])('page %j is %d', (value, expected) => {
    expect(parsePageParam(value)).toBe(expected);
  });

  test.each([
    [null, 'relevance'],
    ['relevance', 'relevance'],
    ['title', 'title'],
    ['lastUpdated', 'lastUpdated'],
    ['TITLE', 'relevance'],
    ['__proto__', 'relevance'],
  ])('sort %j is %s', (value, expected) => {
    expect(parseSortParam(value)).toBe(expected);
  });
});
