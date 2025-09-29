/**
 * Custom hooks for series data management
 *
 * This module provides React hooks for fetching and managing economic time series data
 * from various data sources including FRED, BLS, and other economic data providers.
 */

import { useQuery } from 'react-query';
import { executeGraphQL, QUERIES } from '../utils/graphql';

// Types for series data
export interface SeriesDataPoint {
  date: string;
  value: number;
}

export interface SeriesDetail {
  id: string;
  title: string;
  description: string;
  sourceId: string;
  frequency: string;
  units: string;
  lastUpdated: string;
  startDate: string;
  endDate: string;
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

// Hook for fetching series detail
export const useSeriesDetail = (seriesId: string | null, enabled: boolean = true) => {
  return useQuery(
    ['seriesDetail', seriesId],
    async () => {
      if (!seriesId) return null;

      const result = await executeGraphQL({
        query: QUERIES.GET_SERIES_DETAIL,
        variables: { id: seriesId },
      });
      return result.data?.seriesDetail || null;
    },
    {
      enabled: enabled && !!seriesId,
      staleTime: 5 * 60 * 1000, // 5 minutes
    }
  );
};

// Hook for fetching series data points
export const useSeriesData = (
  seriesId: string | null,
  startDate?: string,
  endDate?: string,
  transformation?: string,
  originalOnly: boolean = false,
  enabled: boolean = true
) => {
  return useQuery(
    ['seriesData', seriesId, startDate, endDate, transformation, originalOnly],
    async () => {
      if (!seriesId) return null;

      const result = await executeGraphQL({
        query: QUERIES.GET_SERIES_DATA,
        variables: { id: seriesId, startDate, endDate, transformation, originalOnly },
      });
      return result.data?.seriesData || null;
    },
    {
      enabled: enabled && !!seriesId,
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
  enabled: boolean = true
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
export const useSearchSuggestions = (partialQuery: string, enabled: boolean = true) => {
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
export const useDataSources = (enabled: boolean = true) => {
  return useQuery(
    ['dataSources'],
    async () => {
      const result = await executeGraphQL({
        query: QUERIES.GET_DATA_SOURCES,
      });
      return result.data?.dataSources || [];
    },
    {
      enabled,
      staleTime: 30 * 60 * 1000, // 30 minutes
    }
  );
};

// Hook for crawler status
export const useCrawlerStatus = (enabled: boolean = true) => {
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
