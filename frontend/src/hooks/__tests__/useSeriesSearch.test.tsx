// Tests that useSeriesSearch sends the variables the backend's searchSeries query declares
// and returns the series list from its SearchResult.

import React from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { vi } from 'vitest';

// The global test setup replaces this module with a mock; these tests need the real hook.
vi.unmock('../useSeriesData');

const executeGraphQL = vi.fn();

vi.mock('../../utils/graphql', () => ({
  executeGraphQL: (request: unknown) => executeGraphQL(request),
  QUERIES: { SEARCH_SERIES: 'query SearchSeries' },
}));

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
    {children}
  </QueryClientProvider>
);

describe('useSeriesSearch', () => {
  beforeEach(() => {
    executeGraphQL.mockReset();
  });

  test('sends query, source and first, and returns the series list', async () => {
    const series = [{ id: 'series-1', title: 'Unemployment Rate' }];
    executeGraphQL.mockResolvedValue({
      data: { searchSeries: { series, totalCount: 1, query: 'unemployment', tookMs: 3 } },
    });
    const { useSeriesSearch } = await import('../useSeriesData');

    const { result } = renderHook(
      () => useSeriesSearch('unemployment', { sourceId: 'source-uuid', limit: 5 }),
      { wrapper }
    );

    await waitFor(() => expect(result.current.data).toEqual(series));
    expect(executeGraphQL).toHaveBeenCalledWith({
      query: 'query SearchSeries',
      variables: { query: 'unemployment', source: 'source-uuid', first: 5 },
    });
  });

  test('trims the query and stays idle for fewer than two characters', async () => {
    const { useSeriesSearch } = await import('../useSeriesData');

    const { result } = renderHook(() => useSeriesSearch('  u  '), { wrapper });

    expect(result.current.fetchStatus).toBe('idle');
    await new Promise(resolve => setTimeout(resolve, 20));
    expect(executeGraphQL).not.toHaveBeenCalled();
  });

  test('sends the trimmed query', async () => {
    executeGraphQL.mockResolvedValue({ data: { searchSeries: { series: [] } } });
    const { useSeriesSearch } = await import('../useSeriesData');

    renderHook(() => useSeriesSearch('  gdp '), { wrapper });

    await waitFor(() =>
      expect(executeGraphQL).toHaveBeenCalledWith({
        query: 'query SearchSeries',
        variables: { query: 'gdp', source: undefined, frequency: undefined, first: undefined },
      })
    );
  });

  test('sends the frequency filter', async () => {
    executeGraphQL.mockResolvedValue({ data: { searchSeries: { series: [] } } });
    const { useSeriesSearch } = await import('../useSeriesData');

    renderHook(() => useSeriesSearch('gdp', { frequency: 'MONTHLY' }), { wrapper });

    await waitFor(() =>
      expect(executeGraphQL).toHaveBeenCalledWith({
        query: 'query SearchSeries',
        variables: { query: 'gdp', source: undefined, frequency: 'MONTHLY', first: undefined },
      })
    );
  });
});
