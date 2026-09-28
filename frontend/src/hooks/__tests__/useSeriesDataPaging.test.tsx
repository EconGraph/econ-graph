/**
 * The real `useSeriesData` (unmocked here) follows `seriesData` pages to the end and
 * concatenates them.
 */

import React from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { vi } from 'vitest';
import { executeGraphQL, GraphQLRequest, QUERIES } from '../../utils/graphql';
import { fetchAllSeriesData, useSeriesData, SERIES_PAGE_SIZE } from '../useSeriesData';
import {
  DAILY_PAGE_SIZE,
  UNRATE_ID,
  dailyPages,
  dailyPoints,
  page,
} from '../../pages/__tests__/fixtures/seriesDetail';

// setupTests mocks the hooks globally; this file tests the real ones.
vi.mock('../useSeriesData', async () => vi.importActual('../useSeriesData'));

vi.mock('../../utils/graphql', async () => {
  const actual = await vi.importActual<typeof import('../../utils/graphql')>(
    '../../utils/graphql'
  );
  return { ...actual, executeGraphQL: vi.fn() };
});

const mockExecute = vi.mocked(executeGraphQL);

/** Serve `pages` keyed by the `after` cursor ('first' for the first page). */
function servePages(pages: Record<string, ReturnType<typeof page>>) {
  mockExecute.mockImplementation(async (request: GraphQLRequest) => {
    expect(request.query).toBe(QUERIES.GET_SERIES_DATA);
    const response = pages[request.variables?.after ?? 'first'];
    if (!response) throw new Error(`no page after ${request.variables?.after}`);
    return { data: response };
  });
}

const dataRequests = () => mockExecute.mock.calls.map(([request]) => request);

const DATE_RANGE = { startDate: '2024-01-01', endDate: '2024-01-31' };

function renderUseSeriesData(transformation: 'NONE' | 'LOG_DIFFERENCE' = 'NONE') {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, cacheTime: 0 } },
  });
  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const view = renderHook(() => useSeriesData(UNRATE_ID, { transformation, ...DATE_RANGE }), {
    wrapper,
  });
  return { ...view, queryClient };
}

describe('useSeriesData paging', () => {
  beforeEach(() => {
    mockExecute.mockReset();
  });

  test('follows endCursor through three pages and concatenates them in order', async () => {
    expect(dailyPoints.length).toBeGreaterThan(2 * DAILY_PAGE_SIZE);
    const signals: (AbortSignal | undefined)[] = [];
    mockExecute.mockImplementation(async (request: GraphQLRequest, signal?: AbortSignal) => {
      signals.push(signal);
      return { data: dailyPages[request.variables?.after ?? 'first'] };
    });

    const { result } = renderUseSeriesData();
    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    // The same live signal (React Query's own, not a fresh one per page) goes on every page.
    expect(signals).toHaveLength(3);
    signals.forEach(signal => {
      expect(signal).toBeInstanceOf(AbortSignal);
      expect(signal).toBe(signals[0]);
      expect(signal?.aborted).toBe(false);
    });

    expect(result.current.data?.points.map(p => p.date)).toEqual(dailyPoints.map(p => p.date));
    expect(result.current.data?.points.map(p => p.value)).toEqual(
      dailyPoints.map(p => Number(p.value))
    );
    expect(result.current.data?.points[6].isOriginalRelease).toBe(true);
    expect(result.current.data?.transformation).toBe('NONE');
  });

  test('sends the same filter and transformation on every page, with the last endCursor', async () => {
    servePages(dailyPages);

    const { result } = renderUseSeriesData('LOG_DIFFERENCE');
    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    const requests = dataRequests();
    const expectedAfter = [undefined, '3', '6'];
    expect(requests.map(r => r.variables?.after)).toEqual(expectedAfter);
    expect(requests[0].variables).not.toHaveProperty('after');
    requests.forEach((request, i) => {
      expect(request.variables).toEqual({
        seriesId: UNRATE_ID,
        filter: { ...DATE_RANGE, latestRevisionOnly: true },
        transformation: 'LOG_DIFFERENCE',
        first: SERIES_PAGE_SIZE,
        ...(expectedAfter[i] === undefined ? {} : { after: expectedAfter[i] }),
      });
    });
  });

  test('passes the query signal to page requests and stops it from starting more once cancelled', async () => {
    const signals: (AbortSignal | undefined)[] = [];
    let releaseFirstPage: () => void = () => {};
    mockExecute.mockImplementation(async (request: GraphQLRequest, signal?: AbortSignal) => {
      signals.push(signal);
      if (request.variables?.after === undefined) {
        await new Promise<void>(resolve => {
          releaseFirstPage = resolve;
        });
      }
      return { data: dailyPages[request.variables?.after ?? 'first'] };
    });

    const { queryClient, unmount } = renderUseSeriesData();
    await waitFor(() => expect(signals).toHaveLength(1));
    expect(signals[0]).toBeInstanceOf(AbortSignal);

    unmount();
    await queryClient.cancelQueries();
    expect(signals[0]?.aborted).toBe(true);
    releaseFirstPage();
    await new Promise(resolve => setTimeout(resolve, 20));
    // No later page is requested once the query is cancelled.
    expect(signals).toHaveLength(1);
  });

  test('a missing page after the first is an error, not a short series', async () => {
    servePages({
      first: page(dailyPoints.slice(0, 3), 7, { hasNextPage: true, endCursor: '3' }),
      '3': { seriesData: null as unknown as ReturnType<typeof page>['seriesData'] },
    });

    await expect(
      fetchAllSeriesData({
        seriesId: UNRATE_ID,
        filter: { latestRevisionOnly: true },
        transformation: 'NONE',
      })
    ).rejects.toThrow(/no page after 3 points/);
  });

  test('a server that pages past its own totalCount is an error', async () => {
    servePages({
      first: page(dailyPoints.slice(0, 3), 3, { hasNextPage: true, endCursor: '3' }),
    });

    await expect(
      fetchAllSeriesData({
        seriesId: UNRATE_ID,
        filter: { latestRevisionOnly: true },
        transformation: 'NONE',
      })
    ).rejects.toThrow(/paging stalled after 3 points/);
    expect(dataRequests()).toHaveLength(1);
  });

  test('a single page needs one request', async () => {
    servePages({ first: page(dailyPoints.slice(0, 2), 2) });

    const { result } = renderUseSeriesData();
    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(dataRequests()).toHaveLength(1);
    expect(result.current.data?.points).toHaveLength(2);
  });

  test('an empty series is one request and no points', async () => {
    servePages({ first: page([], 0) });

    const { result } = renderUseSeriesData();
    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(dataRequests()).toHaveLength(1);
    expect(result.current.data?.points).toEqual([]);
  });

  test('a page that promises more without a new cursor is an error, not a loop', async () => {
    servePages({
      first: page(dailyPoints.slice(0, 3), 7, { hasNextPage: true, endCursor: '3' }),
      '3': page([], 7, { hasNextPage: true, endCursor: '3' }),
    });

    await expect(
      fetchAllSeriesData({
        seriesId: UNRATE_ID,
        filter: { latestRevisionOnly: true },
        transformation: 'NONE',
      })
    ).rejects.toThrow(/paging stalled after 3 points/);
    expect(dataRequests()).toHaveLength(2);
  });

  test('a failing later page fails the whole query', async () => {
    servePages({ first: dailyPages.first });

    const { result } = renderUseSeriesData();
    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(dataRequests()).toHaveLength(2);
  });
});
