/**
 * Custom hooks for crawler data management
 *
 * Provides:
 * - Real-time crawler status monitoring
 * - Queue statistics with auto-refresh
 * - Performance metrics tracking
 * - Error handling and loading states
 */

import { useQuery, useMutation, useSubscription } from "@apollo/client/react";
import { useCallback, useEffect, useState } from "react";
import {
  GET_CRAWLER_STATUS,
  GET_QUEUE_STATISTICS,
  GET_PERFORMANCE_METRICS,
  type CrawlerStatus,
  type QueueStatistics,
  type PerformanceMetrics,
} from "../services/graphql/queries";
import {
  TRIGGER_CRAWL,
  START_CRAWLER,
  STOP_CRAWLER,
  PAUSE_CRAWLER,
  RESUME_CRAWLER,
  type TriggerCrawlInput,
} from "../services/graphql/mutations";

// ============================================================================
// CRAWLER STATUS HOOK
// ============================================================================

export const useCrawlerStatus = (
  autoRefresh: boolean = true,
  intervalMs: number = 30000,
) => {
  const { data, loading, error, refetch, startPolling, stopPolling } =
    useQuery<{
      crawlerStatus: CrawlerStatus;
    }>(GET_CRAWLER_STATUS, {
      errorPolicy: "all",
      notifyOnNetworkStatusChange: true,
    });

  useEffect(() => {
    if (autoRefresh) {
      startPolling(intervalMs);
      return () => stopPolling();
    }
  }, [autoRefresh, intervalMs, startPolling, stopPolling]);

  const refresh = useCallback(() => {
    refetch();
  }, [refetch]);

  return {
    status: data?.crawlerStatus,
    loading,
    error,
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
  const { data, loading, error, refetch, startPolling, stopPolling } =
    useQuery<{
      queueStatistics: QueueStatistics;
    }>(GET_QUEUE_STATISTICS, {
      errorPolicy: "all",
      notifyOnNetworkStatusChange: true,
    });

  useEffect(() => {
    if (autoRefresh) {
      startPolling(intervalMs);
      return () => stopPolling();
    }
  }, [autoRefresh, intervalMs, startPolling, stopPolling]);

  const refresh = useCallback(() => {
    refetch();
  }, [refetch]);

  // Calculate derived metrics
  const progressPercentage = data?.queueStatistics.total_items
    ? (data.queueStatistics.completed_items /
        data.queueStatistics.total_items) *
      100
    : 0;

  const successRate = data?.queueStatistics.total_items
    ? (data.queueStatistics.completed_items /
        data.queueStatistics.total_items) *
      100
    : 0;

  const errorRate = data?.queueStatistics.total_items
    ? (data.queueStatistics.failed_items / data.queueStatistics.total_items) *
      100
    : 0;

  return {
    statistics: data?.queueStatistics,
    loading,
    error,
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
  const { data, loading, error, refetch, startPolling, stopPolling } =
    useQuery<{
      performanceMetrics: PerformanceMetrics[];
    }>(GET_PERFORMANCE_METRICS, {
      variables: { time_range: timeRange },
      errorPolicy: "all",
      notifyOnNetworkStatusChange: true,
    });

  useEffect(() => {
    if (autoRefresh) {
      startPolling(30000); // 30 second intervals
      return () => stopPolling();
    }
  }, [autoRefresh, startPolling, stopPolling]);

  const refresh = useCallback(() => {
    refetch();
  }, [refetch]);

  // Get latest metrics
  const latestMetrics =
    data?.performanceMetrics?.[data.performanceMetrics.length - 1];

  return {
    metrics: data?.performanceMetrics || [],
    latestMetrics,
    loading,
    error,
    refresh,
  };
};

// ============================================================================
// CRAWLER CONTROL HOOKS
// ============================================================================

export const useCrawlerControl = () => {
  const [triggerCrawl] = useMutation(TRIGGER_CRAWL);
  const [startCrawler] = useMutation(START_CRAWLER);
  const [stopCrawler] = useMutation(STOP_CRAWLER);
  const [pauseCrawler] = useMutation(PAUSE_CRAWLER);
  const [resumeCrawler] = useMutation(RESUME_CRAWLER);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleTriggerCrawl = useCallback(
    async (input: TriggerCrawlInput) => {
      try {
        setLoading(true);
        setError(null);
        const result = await triggerCrawl({ variables: { input } });
        return (result.data as any)?.triggerCrawl;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to trigger crawl";
        setError(errorMessage);
        throw err;
      } finally {
        setLoading(false);
      }
    },
    [triggerCrawl],
  );

  const handleStartCrawler = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await startCrawler();
      return (result.data as any)?.startCrawler;
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to start crawler";
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [startCrawler]);

  const handleStopCrawler = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await stopCrawler();
      return (result.data as any)?.stopCrawler;
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to stop crawler";
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [stopCrawler]);

  const handlePauseCrawler = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await pauseCrawler();
      return (result.data as any)?.pauseCrawler;
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to pause crawler";
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [pauseCrawler]);

  const handleResumeCrawler = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await resumeCrawler();
      return (result.data as any)?.resumeCrawler;
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to resume crawler";
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [resumeCrawler]);

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
  const control = useCrawlerControl();

  const refreshAll = useCallback(() => {
    status.refresh();
    queueStats.refresh();
    performance.refresh();
  }, [status.refresh, queueStats.refresh, performance.refresh]);

  return {
    status,
    queueStats,
    performance,
    control,
    refreshAll,
    loading: status.loading || queueStats.loading || performance.loading,
    error: status.error || queueStats.error || performance.error,
  };
};
