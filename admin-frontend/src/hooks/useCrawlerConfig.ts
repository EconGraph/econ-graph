/**
 * React Query hooks for Crawler Configuration
 *
 * Provides data fetching and mutation capabilities for crawler configuration
 * using GraphQL operations with proper caching and error handling.
 */

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

// Types
export interface CrawlerConfigData {
  global_enabled: boolean;
  max_workers: number;
  queue_size_limit: number;
  default_timeout: number;
  default_retry_attempts: number;
  rate_limit_global: number;
  schedule_frequency: string;
  error_threshold: number;
  maintenance_mode: boolean;
  created_at: string;
  updated_at: string;
}

export interface DataSource {
  id: string;
  name: string;
  enabled: boolean;
  priority: number;
  rate_limit: number;
  retry_attempts: number;
  timeout_seconds: number;
  last_success: string | null;
  last_error: string | null;
  health_status: "healthy" | "warning" | "error";
  base_url: string;
  api_key_required: boolean;
  created_at: string;
  updated_at: string;
}

// GraphQL operations
const GET_CRAWLER_CONFIG = `
  query GetCrawlerConfig {
    crawlerConfig {
      global_enabled
      max_workers
      queue_size_limit
      default_timeout
      default_retry_attempts
      rate_limit_global
      schedule_frequency
      error_threshold
      maintenance_mode
      created_at
      updated_at
    }
  }
`;

const GET_DATA_SOURCES = `
  query GetDataSources {
    dataSources {
      id
      name
      enabled
      priority
      rate_limit
      retry_attempts
      timeout_seconds
      last_success
      last_error
      health_status
      base_url
      api_key_required
      created_at
      updated_at
    }
  }
`;

const UPDATE_CRAWLER_CONFIG = `
  mutation UpdateCrawlerConfig($input: CrawlerConfigInput!) {
    updateCrawlerConfig(input: $input) {
      success
      message
      config {
        global_enabled
        max_workers
        queue_size_limit
        default_timeout
        default_retry_attempts
        rate_limit_global
        schedule_frequency
        error_threshold
        maintenance_mode
        created_at
        updated_at
      }
    }
  }
`;

const UPDATE_DATA_SOURCE = `
  mutation UpdateDataSource($id: ID!, $input: DataSourceInput!) {
    updateDataSource(id: $id, input: $input) {
      success
      message
      dataSource {
        id
        name
        enabled
        priority
        rate_limit
        retry_attempts
        timeout_seconds
        last_success
        last_error
        health_status
        base_url
        api_key_required
        created_at
        updated_at
      }
    }
  }
`;

const TEST_DATA_SOURCE_CONNECTION = `
  mutation TestDataSourceConnection($id: ID!) {
    testDataSourceConnection(id: $id) {
      success
      message
      connection_status
      response_time_ms
    }
  }
`;

// Helper function to make GraphQL requests
const graphqlRequest = async (query: string, variables: any = {}) => {
  console.log(
    "[useCrawlerConfig] graphqlRequest called with query:",
    query.substring(0, 50) + "...",
  );
  console.log("[useCrawlerConfig] About to call fetch with URL: /graphql");
  const response = await fetch("/graphql", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${localStorage.getItem("admin_token")}`,
    },
    body: JSON.stringify({
      query,
      variables,
      operationName: query.includes("GetCrawlerConfig")
        ? "GetCrawlerConfig"
        : query.includes("GetDataSources")
          ? "GetDataSources"
          : query.includes("UpdateCrawlerConfig")
            ? "UpdateCrawlerConfig"
            : query.includes("UpdateDataSource")
              ? "UpdateDataSource"
              : query.includes("TestDataSourceConnection")
                ? "TestDataSourceConnection"
                : "Unknown",
    }),
  });

  if (!response.ok) {
    throw new Error(`GraphQL request failed: ${response.statusText}`);
  }

  const result = await response.json();

  if (result.errors) {
    throw new Error(result.errors[0].message);
  }

  return result.data;
};

// React Query hooks
export const useCrawlerConfig = () => {
  return useQuery({
    queryKey: ["crawlerConfig"],
    queryFn: () => graphqlRequest(GET_CRAWLER_CONFIG),
    staleTime: 2 * 60 * 1000, // 2 minutes
    gcTime: 5 * 60 * 1000, // 5 minutes
    retry: (failureCount, error) => {
      // Don't retry on 4xx errors
      if (error instanceof Error && error.message.includes("4")) {
        return false;
      }
      return failureCount < 3;
    },
  });
};

export const useDataSources = () => {
  return useQuery({
    queryKey: ["dataSources"],
    queryFn: () => graphqlRequest(GET_DATA_SOURCES),
    staleTime: 30 * 1000, // 30 seconds (data sources change more frequently)
    gcTime: 2 * 60 * 1000, // 2 minutes
    retry: (failureCount, error) => {
      if (error instanceof Error && error.message.includes("4")) {
        return false;
      }
      return failureCount < 3;
    },
  });
};

export const useUpdateCrawlerConfig = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: Partial<CrawlerConfigData>) =>
      graphqlRequest(UPDATE_CRAWLER_CONFIG, { input }),
    onSuccess: () => {
      // Invalidate and refetch crawler config
      queryClient.invalidateQueries({ queryKey: ["crawlerConfig"] });
    },
    onError: (error) => {
      console.error("Failed to update crawler config:", error);
    },
  });
};

export const useUpdateDataSource = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: Partial<DataSource> }) =>
      graphqlRequest(UPDATE_DATA_SOURCE, { id, input }),
    onSuccess: () => {
      // Invalidate and refetch data sources
      queryClient.invalidateQueries({ queryKey: ["dataSources"] });
    },
    onError: (error) => {
      console.error("Failed to update data source:", error);
    },
  });
};

export const useTestDataSourceConnection = () => {
  return useMutation({
    mutationFn: (id: string) =>
      graphqlRequest(TEST_DATA_SOURCE_CONNECTION, { id }),
    onError: (error) => {
      console.error("Failed to test data source connection:", error);
    },
  });
};
