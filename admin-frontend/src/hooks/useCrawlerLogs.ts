/**
 * Custom hooks for crawler logs and monitoring
 *
 * Provides:
 * - Real-time log streaming and filtering
 * - Performance metrics monitoring
 * - Log export and management
 * - System health monitoring
 */

import { useQuery, useMutation } from "@apollo/client/react";
import { useCallback, useState, useEffect } from "react";
import {
  GET_CRAWLER_LOGS,
  GET_PERFORMANCE_METRICS,
  GET_SYSTEM_HEALTH,
  GET_LOG_STATISTICS,
  type LogEntry,
  type PerformanceMetrics,
  type SystemHealth,
} from "../services/graphql/queries";
import {
  SEARCH_LOGS,
  CLEAR_LOGS,
  EXPORT_LOGS,
  type LogFilters,
} from "../services/graphql/mutations";

// ============================================================================
// LOGS HOOK
// ============================================================================

export const useCrawlerLogs = (
  filters: LogFilters = {},
  limit: number = 100,
  offset: number = 0,
  autoRefresh: boolean = true,
) => {
  const { data, loading, error, refetch, startPolling, stopPolling } =
    useQuery<{
      crawlerLogs: LogEntry[];
    }>(GET_CRAWLER_LOGS, {
      variables: {
        limit,
        offset,
        ...filters,
      },
      errorPolicy: "all",
      notifyOnNetworkStatusChange: true,
    });

  useEffect(() => {
    if (autoRefresh) {
      startPolling(5000); // 5 second intervals for logs
      return () => stopPolling();
    }
  }, [autoRefresh, startPolling, stopPolling]);

  const refresh = useCallback(() => {
    refetch();
  }, [refetch]);

  const logs = data?.crawlerLogs || [];

  // Group logs by level for statistics
  const logsByLevel = logs.reduce(
    (acc, log) => {
      acc[log.level] = (acc[log.level] || 0) + 1;
      return acc;
    },
    {} as Record<string, number>,
  );

  // Group logs by source for statistics
  const logsBySource = logs.reduce(
    (acc, log) => {
      acc[log.source] = (acc[log.source] || 0) + 1;
      return acc;
    },
    {} as Record<string, number>,
  );

  return {
    logs,
    loading,
    error,
    refresh,
    statistics: {
      total: logs.length,
      byLevel: logsByLevel,
      bySource: logsBySource,
    },
  };
};

// ============================================================================
// LOG SEARCH HOOK
// ============================================================================

export const useLogSearch = () => {
  const [searchLogs] = useMutation(SEARCH_LOGS);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<LogEntry[]>([]);

  const search = useCallback(
    async (query: string, filters?: LogFilters) => {
      try {
        setLoading(true);
        setError(null);
        const result = await searchLogs({
          variables: { query, filters },
        });
        const searchResults = (result.data as any)?.searchLogs || [];
        setResults(searchResults);
        return searchResults;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Search failed";
        setError(errorMessage);
        throw err;
      } finally {
        setLoading(false);
      }
    },
    [searchLogs],
  );

  const clearResults = useCallback(() => {
    setResults([]);
    setError(null);
  }, []);

  return {
    results,
    loading,
    error,
    search,
    clearResults,
  };
};

// ============================================================================
// LOG MANAGEMENT HOOK
// ============================================================================

export const useLogManagement = () => {
  const [clearLogs] = useMutation(CLEAR_LOGS);
  const [exportLogs] = useMutation(EXPORT_LOGS);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const clearLogEntries = useCallback(
    async (filters?: LogFilters) => {
      try {
        setLoading(true);
        setError(null);
        const result = await clearLogs({ variables: { filters } });
        return (result.data as any)?.clearLogs;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to clear logs";
        setError(errorMessage);
        throw err;
      } finally {
        setLoading(false);
      }
    },
    [clearLogs],
  );

  const exportLogEntries = useCallback(
    async (filters?: LogFilters, format: string = "json") => {
      try {
        setLoading(true);
        setError(null);
        const result = await exportLogs({ variables: { filters, format } });
        return (result.data as any)?.exportLogs;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to export logs";
        setError(errorMessage);
        throw err;
      } finally {
        setLoading(false);
      }
    },
    [exportLogs],
  );

  return {
    loading,
    error,
    actions: {
      clear: clearLogEntries,
      export: exportLogEntries,
    },
  };
};

// ============================================================================
// PERFORMANCE METRICS HOOK
// ============================================================================

export const usePerformanceMonitoring = (
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
      startPolling(10000); // 10 second intervals for performance metrics
      return () => stopPolling();
    }
  }, [autoRefresh, startPolling, stopPolling]);

  const refresh = useCallback(() => {
    refetch();
  }, [refetch]);

  const metrics = data?.performanceMetrics || [];
  const latestMetrics = metrics[metrics.length - 1];

  // Calculate trends
  const cpuTrend =
    metrics.length > 1
      ? metrics[metrics.length - 1].cpu_usage_percent -
        metrics[0].cpu_usage_percent
      : 0;

  const memoryTrend =
    metrics.length > 1
      ? metrics[metrics.length - 1].memory_usage_percent -
        metrics[0].memory_usage_percent
      : 0;

  const errorRateTrend =
    metrics.length > 1
      ? metrics[metrics.length - 1].error_rate_percent -
        metrics[0].error_rate_percent
      : 0;

  return {
    metrics,
    latestMetrics,
    loading,
    error,
    refresh,
    trends: {
      cpu: cpuTrend,
      memory: memoryTrend,
      errorRate: errorRateTrend,
    },
  };
};

// ============================================================================
// SYSTEM HEALTH HOOK
// ============================================================================

export const useSystemHealth = (autoRefresh: boolean = true) => {
  const { data, loading, error, refetch, startPolling, stopPolling } =
    useQuery<{
      systemHealth: SystemHealth;
    }>(GET_SYSTEM_HEALTH, {
      errorPolicy: "all",
      notifyOnNetworkStatusChange: true,
    });

  useEffect(() => {
    if (autoRefresh) {
      startPolling(30000); // 30 second intervals for system health
      return () => stopPolling();
    }
  }, [autoRefresh, startPolling, stopPolling]);

  const refresh = useCallback(() => {
    refetch();
  }, [refetch]);

  const health = data?.systemHealth;

  // Calculate overall health score
  const healthScore = health
    ? {
        overall:
          health.overall_status === "healthy"
            ? 100
            : health.overall_status === "warning"
              ? 75
              : health.overall_status === "error"
                ? 25
                : 0,
        components: health.components.map((comp) => ({
          ...comp,
          score:
            comp.status === "healthy"
              ? 100
              : comp.status === "warning"
                ? 75
                : comp.status === "error"
                  ? 25
                  : 0,
        })),
      }
    : null;

  return {
    health,
    healthScore,
    loading,
    error,
    refresh,
  };
};

// ============================================================================
// LOG STATISTICS HOOK
// ============================================================================

export const useLogStatistics = (timeRange: string = "1h") => {
  const { data, loading, error, refetch } = useQuery<{
    logStatistics: {
      total_logs: number;
      logs_by_level: Array<{ level: string; count: number }>;
      logs_by_source: Array<{ source: string; count: number }>;
      error_rate_percent: number;
      average_duration_ms: number;
    };
  }>(GET_LOG_STATISTICS, {
    variables: { time_range: timeRange },
    errorPolicy: "all",
  });

  const refresh = useCallback(() => {
    refetch();
  }, [refetch]);

  const statistics = data?.logStatistics;

  return {
    statistics,
    loading,
    error,
    refresh,
  };
};

// ============================================================================
// COMBINED MONITORING HOOK
// ============================================================================

export const useCrawlerMonitoring = (
  logFilters: LogFilters = {},
  autoRefresh: boolean = true,
) => {
  const logs = useCrawlerLogs(logFilters, 100, 0, autoRefresh);
  const performance = usePerformanceMonitoring("1h", autoRefresh);
  const systemHealth = useSystemHealth(autoRefresh);
  const logStats = useLogStatistics("1h");
  const logManagement = useLogManagement();

  const refreshAll = useCallback(() => {
    logs.refresh();
    performance.refresh();
    systemHealth.refresh();
    logStats.refresh();
  }, [logs, performance, systemHealth, logStats]);

  return {
    logs,
    performance,
    systemHealth,
    logStats,
    logManagement,
    refreshAll,
    loading:
      logs.loading ||
      performance.loading ||
      systemHealth.loading ||
      logStats.loading,
    error:
      logs.error || performance.error || systemHealth.error || logStats.error,
  };
};
