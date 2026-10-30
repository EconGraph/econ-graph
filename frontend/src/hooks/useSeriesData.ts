/**
 * Custom hooks for series data management.
 *
 * This module provides React hooks for fetching and managing economic time series data
 * from various data sources including FRED, BLS, and other economic data providers.
 */

import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { executeGraphQL, QUERIES, type SeriesListNode } from '../utils/graphql';
import type { DataTransformation } from '../utils/transformations';

// Types for series data

/** One observation as the page uses it (latest revision of each date). */
export interface SeriesDataPoint {
  /** Observation date, ISO `YYYY-MM-DD`. */
  date: string;
  /** Value after the requested transformation; null where it can't be computed. */
  value: number | null;
  /**
   * The value exactly as the API sent it (a decimal string), for output that must keep every
   * digit, such as the CSV download. `value` is the nearest double, for charting.
   */
  exactValue?: string;
  revisionDate: string;
  isOriginalRelease: boolean;
}

/** A series' observations and the transformation they carry. */
export interface SeriesObservations {
  transformation: DataTransformation;
  points: SeriesDataPoint[];
}

/** Series metadata as returned by the `series(id)` query. */
export interface SeriesDetail {
  id: string;
  externalId: string;
  title: string;
  description: string | null;
  source: { name: string } | null;
  frequency: string;
  units: string | null;
  seasonalAdjustment: string | null;
  startDate: string | null;
  endDate: string | null;
  lastUpdated: string | null;
  isActive: boolean;
}

/** Points requested per `seriesData` page; the backend caps a page at this size. */
export const SERIES_PAGE_SIZE = 10000;

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Whether a string can be a series id. The backend rejects anything that isn't a UUID
 * with an error, so the page treats such ids as unknown without asking.
 * @param id - Candidate series id.
 * @returns True when the id is a UUID.
 */
export function isSeriesId(id: string): boolean {
  return UUID_PATTERN.test(id);
}

interface RawDataPoint {
  date: string;
  // async-graphql serializes BigDecimal as a string
  value: string | number | null;
  revisionDate: string;
  isOriginalRelease: boolean;
}

function toDataPoint(raw: RawDataPoint): SeriesDataPoint {
  const value = raw.value === null ? null : Number(raw.value);
  const finite = value === null || Number.isFinite(value) ? value : null;
  const point: SeriesDataPoint = {
    date: raw.date,
    value: finite,
    revisionDate: raw.revisionDate,
    isOriginalRelease: raw.isOriginalRelease,
  };
  if (finite !== null && typeof raw.value === 'string') point.exactValue = raw.value.trim();
  return point;
}

export interface SeriesSearchResult {
  id: string;
  title: string;
  description: string;
  externalId?: string;
  sourceId: string;
  frequency: string;
  units: string;
  lastUpdated: string;
  startDate: string;
  endDate: string;
  isActive?: boolean;
  rank?: number;
  similarityScore: number;
}

export interface DataSource {
  id: string;
  name: string;
  description: string;
  baseUrl: string;
  apiKeyRequired: boolean;
  rateLimitPerMinute: number;
  seriesCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface CrawlerStatus {
  isRunning: boolean;
  lastRun: string;
  nextRun: string;
  processedSeries: number;
  totalSeries: number;
}

// Hook for fetching series detail. Resolves to null when the series doesn't exist.
export const useSeriesDetail = (seriesId: string | null, enabled = true) => {
  return useQuery({
    queryKey: ['seriesDetail', seriesId],
    queryFn: async (): Promise<SeriesDetail | null> => {
      if (!seriesId || !isSeriesId(seriesId)) return null;

      const result = await executeGraphQL<{ series: SeriesDetail | null }>({
        query: QUERIES.GET_SERIES_DETAIL,
        variables: { id: seriesId },
      });
      return result.data?.series ?? null;
    },
    enabled: enabled && !!seriesId,
    staleTime: 5 * 60 * 1000, // 5 minutes
  });
};

export interface SeriesDataOptions {
  /** Backend transformation to apply; NONE returns levels. */
  transformation?: DataTransformation;
  /** ISO start date; omit for the start of the series. */
  startDate?: string;
  /** ISO end date; omit for the end of the series. */
  endDate?: string;
  enabled?: boolean;
}

interface RawSeriesDataPage {
  nodes: RawDataPoint[];
  totalCount: number;
  pageInfo: { hasNextPage: boolean; endCursor: string | null };
}

/** The `seriesData` variables shared by every page of one read. */
export interface SeriesDataVariables {
  seriesId: string;
  filter: { startDate?: string; endDate?: string; latestRevisionOnly: boolean };
  transformation: DataTransformation;
}

/**
 * Fetch every observation of a series by following `seriesData` pages to the end.
 *
 * The backend returns at most SERIES_PAGE_SIZE points per call, ordered by date, revision
 * date and id, with a cursor that is the count of points read so far. A transformed page
 * carries the values it has in the whole series, so pages concatenate as they are. A
 * missing or stalled page is an error: a partial series must never look complete.
 * @param variables - The query variables of the first page (no `after`).
 * @param signal - Aborts the remaining page requests.
 * @returns Every point, oldest first.
 */
export async function fetchAllSeriesData(
  variables: SeriesDataVariables,
  signal?: AbortSignal
): Promise<SeriesDataPoint[]> {
  const points: SeriesDataPoint[] = [];
  let after: string | undefined;
  for (;;) {
    // fetch rejects an already-aborted signal too; this keeps a cancelled read from starting
    // another page when the transport doesn't honor the signal.
    if (signal?.aborted) throw new DOMException('seriesData read was cancelled', 'AbortError');
    const result = await executeGraphQL<{ seriesData: RawSeriesDataPage | null }>(
      {
        query: QUERIES.GET_SERIES_DATA,
        variables:
          after === undefined
            ? { ...variables, first: SERIES_PAGE_SIZE }
            : { ...variables, first: SERIES_PAGE_SIZE, after },
      },
      signal
    );
    const page = result.data?.seriesData;
    if (!page) throw new Error(`seriesData returned no page after ${points.length} points`);
    for (const raw of page.nodes) points.push(toDataPoint(raw));
    if (!page.pageInfo.hasNextPage) break;
    // Guard against a server that keeps promising a next page without advancing, or past
    // the count it reported.
    const next = page.pageInfo.endCursor;
    if (page.nodes.length === 0 || !next || next === after || points.length >= page.totalCount) {
      throw new Error(`seriesData paging stalled after ${points.length} points`);
    }
    after = next;
  }
  return points;
}

/**
 * Fetch all of a series' observations (latest revision of each date), transformed on the
 * backend and read page by page. `points` is empty when the series has no observations.
 * While another transformation loads, the previous result stays (isPlaceholderData), labeled
 * with its own transformation.
 * @param seriesId - The series id, or null while it isn't known yet.
 * @param options - Transformation, date range and enabled flag.
 * @returns The React Query result of `{ transformation, points }`.
 */
export const useSeriesData = (seriesId: string | null, options: SeriesDataOptions = {}) => {
  const { transformation = 'NONE', startDate, endDate, enabled = true } = options;
  return useQuery({
    queryKey: ['seriesData', seriesId, transformation, startDate, endDate],
    queryFn: async ({ signal }): Promise<SeriesObservations> => {
      if (!seriesId || !isSeriesId(seriesId)) return { transformation, points: [] };

      const points = await fetchAllSeriesData(
        { seriesId, filter: { startDate, endDate, latestRevisionOnly: true }, transformation },
        signal
      );
      return { transformation, points };
    },
    enabled: enabled && !!seriesId,
    staleTime: 5 * 60 * 1000, // 5 minutes
    placeholderData: keepPreviousData,
  });
};

// Hook for searching series
export const useSeriesSearch = (
  query: string,
  filters?: {
    sourceId?: string;
    frequency?: string;
    limit?: number;
  },
  enabled = true
) => {
  const trimmedQuery = query.trim();
  return useQuery({
    queryKey: ['seriesSearch', trimmedQuery, filters],
    queryFn: async () => {
      if (trimmedQuery.length < 2) return [];

      const result = await executeGraphQL({
        query: QUERIES.SEARCH_SERIES,
        variables: {
          query: trimmedQuery,
          source: filters?.sourceId,
          frequency: filters?.frequency,
          first: filters?.limit,
        },
      });
      return result.data?.searchSeries?.series || [];
    },
    enabled: enabled && trimmedQuery.length >= 2,
    staleTime: 2 * 60 * 1000, // 2 minutes
  });
};

/** One page of a source's (or a frequency's) series, and how many there are in all. */
export interface SeriesBrowsePage {
  series: SeriesListNode[];
  totalCount: number;
}

/**
 * Lists series by source and/or frequency without a search query, in title order, so a
 * researcher can see what a source has without guessing a word to search for (ECO-405).
 * Runs only when a source or frequency is given.
 * @param filters - The source id and/or backend frequency to list, how many to fetch, and how
 * many series in title order to skip first (the page's offset), for paging through a source.
 * @param enabled - False to hold the request (e.g. while the source list loads).
 * @returns The query; `data` is that page and the total count.
 */
export const useSeriesBrowse = (
  filters: { sourceId?: string; frequency?: string; limit?: number; offset?: number },
  enabled = true
) => {
  const { sourceId, frequency, limit, offset = 0 } = filters;
  return useQuery({
    queryKey: ['seriesBrowse', sourceId, frequency, limit, offset],
    queryFn: async (): Promise<SeriesBrowsePage> => {
      const result = await executeGraphQL({
        query: QUERIES.GET_SERIES_LIST,
        variables: {
          filter: { sourceId, frequency },
          // seriesList's cursor is the number of series already read
          pagination: { first: limit, ...(offset > 0 && { after: String(offset) }) },
        },
      });
      const list = result.data?.seriesList;
      return { series: list?.nodes ?? [], totalCount: list?.totalCount ?? 0 };
    },
    enabled: enabled && Boolean(sourceId || frequency),
    staleTime: 2 * 60 * 1000, // 2 minutes
  });
};

// Hook for search suggestions
export const useSearchSuggestions = (partialQuery: string, enabled = true) => {
  return useQuery({
    queryKey: ['searchSuggestions', partialQuery],
    queryFn: async () => {
      if (!partialQuery || partialQuery.trim().length < 2) return [];

      const result = await executeGraphQL({
        query: QUERIES.GET_SEARCH_SUGGESTIONS,
        variables: { partialQuery },
      });
      return result.data?.searchSuggestions || [];
    },
    enabled: enabled && partialQuery.trim().length >= 2,
    staleTime: 10 * 60 * 1000, // 10 minutes
  });
};

// Hook for fetching data sources
export const useDataSources = () => {
  return useQuery({
    queryKey: ['dataSources'],
    queryFn: async (): Promise<DataSource[]> => {
      const result = await executeGraphQL({
        query: QUERIES.GET_DATA_SOURCES,
      });
      return result.data?.dataSources || [];
    },
    staleTime: 30 * 60 * 1000, // 30 minutes
    retry: 1, // the source list gates the Explore search, so don't hold it through long retries
  });
};

// Hook for crawler status
export const useCrawlerStatus = (enabled = true) => {
  return useQuery({
    queryKey: ['crawlerStatus'],
    queryFn: async () => {
      const result = await executeGraphQL({
        query: QUERIES.GET_CRAWLER_STATUS,
      });
      return result.data?.crawlerStatus || null;
    },
    enabled,
    staleTime: 1 * 60 * 1000, // 1 minute
    refetchInterval: 30 * 1000, // Refetch every 30 seconds
  });
};
