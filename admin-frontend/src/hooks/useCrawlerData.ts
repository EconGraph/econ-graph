/**
 * Custom hooks for crawler data management
 *
 * Provides:
 * - Real-time crawler status monitoring
 * - Queue statistics with auto-refresh
 * - Performance metrics tracking
 * - Error handling and loading states
 */

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import {
  GET_CRAWLER_STATUS,
  GET_QUEUE_STATISTICS,
  GET_PERFORMANCE_METRICS,
  GET_CRAWLER_LOGS,
} from "../services/graphql/queries";
import {
  TRIGGER_CRAWL,
  START_CRAWLER,
  STOP_CRAWLER,
  PAUSE_CRAWLER,
  RESUME_CRAWLER,
  type TriggerCrawlInput,
} from "../services/graphql/mutations";

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
// CRAWLER STATUS HOOK
// ============================================================================

export const useCrawlerStatus = (
  autoRefresh: boolean = true,
  intervalMs: number = 30000,
) => {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ["crawlerStatus"],
    queryFn: () =>
      graphqlClient(GET_CRAWLER_STATUS.loc?.source?.body || GET_CRAWLER_STATUS),
    refetchInterval: autoRefresh ? intervalMs : false,
    refetchIntervalInBackground: false,
  });

  const refresh = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["crawlerStatus"] });
  }, [queryClient]);

  return {
    status: query.data?.crawlerStatus,
    loading: query.isLoading,
    error: query.error,
    refresh,
  };
};

// ============================================================================
// QUEUE STATISTICS HOOK
// ============================================================================

export const useQueueStatistics = (
  autoRefresh: boolean = true,
  intervalMs: number = 10000,
) => {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ["queueStatistics"],
    queryFn: () =>
      graphqlClient(
        GET_QUEUE_STATISTICS.loc?.source?.body || GET_QUEUE_STATISTICS,
      ),
    refetchInterval: autoRefresh ? intervalMs : false,
    refetchIntervalInBackground: false,
  });

  const refresh = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["queueStatistics"] });
  }, [queryClient]);

  // Calculate derived metrics
  const progressPercentage = query.data?.queueStatistics?.total_items
    ? (query.data.queueStatistics.completed_items /
        query.data.queueStatistics.total_items) *
      100
    : 0;

  const successRate = query.data?.queueStatistics?.total_items
    ? (query.data.queueStatistics.completed_items /
        query.data.queueStatistics.total_items) *
      100
    : 0;

  const errorRate = query.data?.queueStatistics?.total_items
    ? (query.data.queueStatistics.failed_items /
        query.data.queueStatistics.total_items) *
      100
    : 0;

  return {
    statistics: query.data?.queueStatistics,
    loading: query.isLoading,
    error: query.error,
    refresh,
    derived: {
      progressPercentage,
      successRate,
      errorRate,
    },
  };
};

// ============================================================================
// PERFORMANCE METRICS HOOK
// ============================================================================

export const usePerformanceMetrics = (
  timeRange: string = "1h",
  autoRefresh: boolean = true,
) => {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ["performanceMetrics", timeRange],
    queryFn: () =>
      graphqlClient(
        GET_PERFORMANCE_METRICS.loc?.source?.body || GET_PERFORMANCE_METRICS,
        { time_range: timeRange },
      ),
    refetchInterval: autoRefresh ? 30000 : false, // 30 second intervals
    refetchIntervalInBackground: false,
  });

  const refresh = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["performanceMetrics"] });
  }, [queryClient]);

  // Get latest metrics
  const latestMetrics =
    query.data?.performanceMetrics?.[query.data.performanceMetrics.length - 1];

  return {
    metrics: query.data?.performanceMetrics || [],
    latestMetrics,
    loading: query.isLoading,
    error: query.error,
    refresh,
  };
};

// ============================================================================
// CRAWLER LOGS HOOK
// ============================================================================

export const useCrawlerLogs = (
  limit: number = 10,
  autoRefresh: boolean = true,
) => {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ["crawlerLogs", limit],
    queryFn: () =>
      graphqlClient(GET_CRAWLER_LOGS.loc?.source?.body || GET_CRAWLER_LOGS, {
        limit,
      }),
    refetchInterval: autoRefresh ? 15000 : false, // 15 second intervals
    refetchIntervalInBackground: false,
  });

  const refresh = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["crawlerLogs"] });
  }, [queryClient]);

  return {
    logs: query.data?.crawlerLogs || [],
    loading: query.isLoading,
    error: query.error,
    refresh,
  };
};

// ============================================================================
// CRAWLER CONTROL HOOKS
// ============================================================================

export const useCrawlerControl = () => {
  const queryClient = useQueryClient();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const triggerCrawlMutation = useMutation({
    mutationFn: (input: TriggerCrawlInput) =>
      graphqlClient(TRIGGER_CRAWL.loc?.source?.body || TRIGGER_CRAWL, {
        input,
      }),
    onSuccess: () => {
      // Invalidate related queries to refresh data
      queryClient.invalidateQueries({ queryKey: ["crawlerStatus"] });
      queryClient.invalidateQueries({ queryKey: ["queueStatistics"] });
      queryClient.invalidateQueries({ queryKey: ["crawlerLogs"] });
    },
  });

  const startCrawlerMutation = useMutation({
    mutationFn: () =>
      graphqlClient(START_CRAWLER.loc?.source?.body || START_CRAWLER),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["crawlerStatus"] });
    },
  });

  const stopCrawlerMutation = useMutation({
    mutationFn: () =>
      graphqlClient(STOP_CRAWLER.loc?.source?.body || STOP_CRAWLER),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["crawlerStatus"] });
    },
  });

  const pauseCrawlerMutation = useMutation({
    mutationFn: () =>
      graphqlClient(PAUSE_CRAWLER.loc?.source?.body || PAUSE_CRAWLER),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["crawlerStatus"] });
    },
  });

  const resumeCrawlerMutation = useMutation({
    mutationFn: () =>
      graphqlClient(RESUME_CRAWLER.loc?.source?.body || RESUME_CRAWLER),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["crawlerStatus"] });
    },
  });

  const handleTriggerCrawl = useCallback(
    async (input: TriggerCrawlInput) => {
      try {
        setLoading(true);
        setError(null);
        const result = await triggerCrawlMutation.mutateAsync(input);
        return result?.triggerCrawl;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to trigger crawl";
        setError(errorMessage);
        throw err;
      } finally {
        setLoading(false);
      }
    },
    [triggerCrawlMutation],
  );

  const handleStartCrawler = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await startCrawlerMutation.mutateAsync();
      return result?.startCrawler;
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to start crawler";
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [startCrawlerMutation]);

  const handleStopCrawler = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await stopCrawlerMutation.mutateAsync();
      return result?.stopCrawler;
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to stop crawler";
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [stopCrawlerMutation]);

  const handlePauseCrawler = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await pauseCrawlerMutation.mutateAsync();
      return result?.pauseCrawler;
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to pause crawler";
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [pauseCrawlerMutation]);

  const handleResumeCrawler = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await resumeCrawlerMutation.mutateAsync();
      return result?.resumeCrawler;
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to resume crawler";
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [resumeCrawlerMutation]);

  return {
    loading,
    error,
    actions: {
      triggerCrawl: handleTriggerCrawl,
      startCrawler: handleStartCrawler,
      stopCrawler: handleStopCrawler,
      pauseCrawler: handlePauseCrawler,
      resumeCrawler: handleResumeCrawler,
    },
  };
};

// ============================================================================
// COMBINED CRAWLER DATA HOOK
// ============================================================================

export const useCrawlerData = (autoRefresh: boolean = true) => {
  const status = useCrawlerStatus(autoRefresh);
  const queueStats = useQueueStatistics(autoRefresh);
  const performance = usePerformanceMetrics("1h", autoRefresh);
  const logs = useCrawlerLogs(10, autoRefresh);
  const control = useCrawlerControl();

  const refreshAll = useCallback(() => {
    status.refresh();
    queueStats.refresh();
    performance.refresh();
    logs.refresh();
  }, [status, queueStats, performance, logs]);

  return {
    status,
    queueStats,
    performance,
    logs,
    control,
    refreshAll,
    loading:
      status.loading ||
      queueStats.loading ||
      performance.loading ||
      logs.loading,
    error: status.error || queueStats.error || performance.error || logs.error,
  };
};
