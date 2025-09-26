/**
 * Custom hooks for crawler logs management
 *
 * Provides:
 * - Real-time crawler logs monitoring
 * - Log search and filtering
 * - Performance metrics tracking
 * - Error handling and loading states
 */

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";
import {
  GET_CRAWLER_LOGS,
  SEARCH_LOGS,
  type LogEntry,
} from "../services/graphql/queries";

// GraphQL client function - this will be mocked in tests
const graphqlClient = async (query: string | any, variables?: any) => {
  // Convert GraphQL query to string if it's a DocumentNode
  const queryString = typeof query === 'string' ? query : String(query);
  // This will be replaced by actual GraphQL client in production
  // For now, we'll use fetch to make GraphQL requests
  const response = await fetch("/graphql", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      query: queryString,
      variables,
    }),
  });

  if (!response.ok) {
    throw new Error(`GraphQL request failed: ${response.statusText}`);
  }

  const result = await response.json();

  if (result.errors) {
    throw new Error(result.errors[0]?.message || "GraphQL error");
  }

  return result.data;
};

// ============================================================================
// CRAWLER LOGS HOOK
// ============================================================================

export const useCrawlerLogs = (
  limit: number = 50,
  offset: number = 0,
  level?: string,
  source?: string,
  autoRefresh: boolean = true,
  intervalMs: number = 5000,
) => {
  const queryClient = useQueryClient();
  
  const query = useQuery({
    queryKey: ['crawlerLogs', { limit, offset, level, source }],
    queryFn: () => graphqlClient(GET_CRAWLER_LOGS.loc?.source?.body || GET_CRAWLER_LOGS, { 
      limit, 
      offset, 
      level, 
      source 
    }),
    refetchInterval: autoRefresh ? intervalMs : false,
    refetchIntervalInBackground: false,
  });

  const refresh = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ['crawlerLogs'] });
  }, [queryClient]);

  return {
    logs: query.data?.crawlerLogs || [],
    loading: query.isLoading,
    error: query.error,
    refresh,
  };
};

// ============================================================================
// LOG SEARCH HOOK
// ============================================================================

export const useLogSearch = (
  searchQuery: string,
  filters?: any,
  enabled: boolean = true,
) => {
  const queryClient = useQueryClient();
  
  const query = useQuery({
    queryKey: ['logSearch', searchQuery, filters],
    queryFn: () => graphqlClient(SEARCH_LOGS.loc?.source?.body || SEARCH_LOGS, { 
      query: searchQuery, 
      filters 
    }),
    enabled: enabled && searchQuery.length > 0,
    refetchInterval: false,
    refetchIntervalInBackground: false,
  });

  const refresh = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ['logSearch'] });
  }, [queryClient]);

  return {
    searchResults: query.data?.searchLogs || [],
    loading: query.isLoading,
    error: query.error,
    refresh,
  };
};