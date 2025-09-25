/**
 * Custom hooks for system health monitoring
 *
 * Provides:
 * - Real-time system health metrics
 * - Service status monitoring
 * - Error handling and loading states
 */

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";
import {
  GET_SYSTEM_HEALTH,
  type SystemHealth,
} from "../services/graphql/queries";

// GraphQL client function - this will be mocked in tests
const graphqlClient = async (query: string | any, variables?: any) => {
  // Convert GraphQL query to string if it's a DocumentNode
  const queryString = typeof query === "string" ? query : String(query);
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
// SYSTEM HEALTH HOOK
// ============================================================================

export const useSystemHealth = (
  autoRefresh: boolean = true,
  intervalMs: number = 30000,
) => {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ["systemHealth"],
    queryFn: () =>
      graphqlClient(GET_SYSTEM_HEALTH.loc?.source?.body || GET_SYSTEM_HEALTH),
    refetchInterval: autoRefresh ? intervalMs : false,
    refetchIntervalInBackground: false,
  });

  const refresh = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["systemHealth"] });
  }, [queryClient]);

  return {
    systemHealth: query.data?.systemHealth,
    loading: query.isLoading,
    error: query.error,
    refresh,
  };
};
