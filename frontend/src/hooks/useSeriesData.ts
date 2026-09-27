/**
 * Custom hooks for series data management.
 *
 * This module provides React hooks for fetching and managing economic time series data
 * from various data sources including FRED, BLS, and other economic data providers.
 */

import { useQuery } from '@tanstack/react-query';
import { executeGraphQL, QUERIES } from '../utils/graphql';
import type { DataTransformation } from '../utils/transformations';

// Types for series data

/** One observation as the page uses it (latest revision of each date). */
export interface SeriesDataPoint {
  /** Observation date, ISO `YYYY-MM-DD`. */
  date: string;
  /** Value after the requested transformation; null where it can't be computed. */
  value: number | null;
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

/**
 * The `first` sent to `seriesData`, which is also the backend's cap. The backend applies it
 * to raw rows (all revisions, oldest first) before keeping the latest revision of each
 * date, so a series with more rows than this loses its newest observations. Fixing that
 * is backend work (paging or a latest-revision query); see the series UI PR map.
 */
export const MAX_SERIES_POINTS = 10000;

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
  return {
    date: raw.date,
    value: value === null || Number.isFinite(value) ? value : null,
    revisionDate: raw.revisionDate,
    isOriginalRelease: raw.isOriginalRelease,
  };
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
  base_url: string;
  api_key_required: boolean;
  rate_limit_per_minute: number;
  series_count: number;
  created_at: string;
  updated_at: string;
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
  return useQuery(
    ['seriesDetail', seriesId],
    async (): Promise<SeriesDetail | null> => {
      if (!seriesId || !isSeriesId(seriesId)) return null;

      const result = await executeGraphQL<{ series: SeriesDetail | null }>({
        query: QUERIES.GET_SERIES_DETAIL,
        variables: { id: seriesId },
      });
      return result.data?.series ?? null;
    },
    {
      enabled: enabled && !!seriesId,
      staleTime: 5 * 60 * 1000, // 5 minutes
    }
  );
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

// Hook for fetching a series' observations (latest revision of each date), transformed
// on the backend. `points` is empty when the series has no observations. While another
// transformation loads, the previous result stays (isPreviousData), labeled with its own
// transformation.
export const useSeriesData = (seriesId: string | null, options: SeriesDataOptions = {}) => {
  const { transformation = 'NONE', startDate, endDate, enabled = true } = options;
  return useQuery(
    ['seriesData', seriesId, transformation, startDate, endDate],
    async (): Promise<SeriesObservations> => {
      if (!seriesId || !isSeriesId(seriesId)) return { transformation, points: [] };

      const result = await executeGraphQL<{ seriesData: { nodes: RawDataPoint[] } | null }>({
        query: QUERIES.GET_SERIES_DATA,
        variables: {
          seriesId,
          filter: { startDate, endDate, latestRevisionOnly: true },
          transformation,
          first: MAX_SERIES_POINTS,
        },
      });
      return {
        transformation,
        points: (result.data?.seriesData?.nodes ?? []).map(toDataPoint),
      };
    },
    {
      enabled: enabled && !!seriesId,
      keepPreviousData: true,
      staleTime: 5 * 60 * 1000, // 5 minutes
    }
  );
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
  return useQuery(
    ['seriesSearch', query, filters],
    async () => {
      if (!query || query.length < 2) return [];

      const result = await executeGraphQL({
        query: QUERIES.SEARCH_SERIES,
        variables: { query, filters },
      });
      return result.data?.searchSeries || [];
    },
    {
      enabled: enabled && query.length >= 2,
      staleTime: 2 * 60 * 1000, // 2 minutes
    }
  );
};

// Hook for search suggestions
export const useSearchSuggestions = (partialQuery: string, enabled = true) => {
  return useQuery(
    ['searchSuggestions', partialQuery],
    async () => {
      if (!partialQuery || partialQuery.trim().length < 2) return [];

      const result = await executeGraphQL({
        query: QUERIES.GET_SEARCH_SUGGESTIONS,
        variables: { partialQuery },
      });
      return result.data?.searchSuggestions || [];
    },
    {
      enabled: enabled && partialQuery.trim().length >= 2,
      staleTime: 10 * 60 * 1000, // 10 minutes
    }
  );
};

// Hook for fetching data sources
export const useDataSources = () => {
  return useQuery({
    queryKey: ['dataSources'],
    queryFn: async () => {
      const result = await executeGraphQL({
        query: QUERIES.GET_DATA_SOURCES,
      });
      return result.data?.dataSources || [];
    },
    suspense: true,
    staleTime: 30 * 60 * 1000, // 30 minutes
  });
};

// Hook for crawler status
export const useCrawlerStatus = (enabled = true) => {
  return useQuery(
    ['crawlerStatus'],
    async () => {
      const result = await executeGraphQL({
        query: QUERIES.GET_CRAWLER_STATUS,
      });
      return result.data?.crawlerStatus || null;
    },
    {
      enabled,
      staleTime: 1 * 60 * 1000, // 1 minute
      refetchInterval: 30 * 1000, // Refetch every 30 seconds
    }
  );
};
