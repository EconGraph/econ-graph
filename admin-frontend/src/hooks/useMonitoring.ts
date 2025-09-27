/**
 * React Query hooks for Monitoring Dashboard
 *
 * Provides data fetching, caching, and real-time updates for monitoring dashboards.
 * Optimized for admin UI with focus on real-time data and background updates.
 */

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

export interface DashboardInfo {
  id: string;
  title: string;
  description: string;
  url: string;
  embedUrl: string;
  status: "healthy" | "warning" | "error";
  lastUpdate: string;
  metrics: {
    totalSeries: number;
    activeCrawlers: number;
    dataPoints: number;
    uptime: string;
  };
}

export interface SystemStatus {
  overall: "healthy" | "warning" | "error";
  services: {
    backend: string;
    database: string;
    crawler: string;
    grafana: string;
  };
  alerts: number;
}

// Mock data - in real implementation, these would make actual API calls
const mockDashboards: DashboardInfo[] = [
  {
    id: "econgraph-overview",
    title: "EconGraph Platform Overview",
    description:
      "High-level system monitoring and health overview with API metrics, resource utilization, and service availability",
    url: "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview",
    embedUrl:
      "http://localhost:30001/d-solo/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&panelId=1",
    status: "healthy",
    lastUpdate: new Date().toISOString(),
    metrics: {
      totalSeries: 1250,
      activeCrawlers: 3,
      dataPoints: 45000,
      uptime: "99.9%",
    },
  },
  {
    id: "database-statistics",
    title: "Database Statistics",
    description:
      "Comprehensive PostgreSQL monitoring for time series data with performance metrics and growth trends",
    url: "http://localhost:30001/d/database-statistics/database-statistics",
    embedUrl:
      "http://localhost:30001/d-solo/database-statistics/database-statistics?orgId=1&from=now-6h&to=now&panelId=1",
    status: "healthy",
    lastUpdate: new Date().toISOString(),
    metrics: {
      totalSeries: 890,
      activeCrawlers: 0,
      dataPoints: 32000,
      uptime: "99.9%",
    },
  },
  {
    id: "crawler-status",
    title: "Crawler Status",
    description:
      "Data crawler monitoring and queue processing analysis with error rates and performance metrics",
    url: "http://localhost:30001/d/crawler-status/crawler-status",
    embedUrl:
      "http://localhost:30001/d-solo/crawler-status/crawler-status?orgId=1&from=now-2h&to=now&panelId=1",
    status: "healthy",
    lastUpdate: new Date().toISOString(),
    metrics: {
      totalSeries: 0,
      activeCrawlers: 2,
      dataPoints: 0,
      uptime: "100%",
    },
  },
];

// Simulate API delay for realistic testing
const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Fetch all monitoring dashboards
 */
export const useDashboards = () => {
  return useQuery({
    queryKey: ["monitoring", "dashboards"],
    queryFn: async (): Promise<DashboardInfo[]> => {
      // Simulate API delay
      await delay(300);

      // In real implementation, this would fetch from Grafana API
      return mockDashboards.map((dashboard) => ({
        ...dashboard,
        lastUpdate: new Date().toISOString(),
        metrics: {
          ...dashboard.metrics,
          // Simulate real-time data updates
          totalSeries:
            dashboard.metrics.totalSeries + Math.floor(Math.random() * 10),
          activeCrawlers: Math.max(
            0,
            dashboard.metrics.activeCrawlers +
              Math.floor(Math.random() * 3) -
              1,
          ),
          dataPoints:
            dashboard.metrics.dataPoints + Math.floor(Math.random() * 100),
        },
      }));
    },
    staleTime: 30 * 1000, // 30 seconds for real-time monitoring data
    gcTime: 2 * 60 * 1000, // 2 minutes cache
    refetchInterval: 30 * 1000, // Refetch every 30 seconds for real-time updates
    refetchIntervalInBackground: true, // Continue refetching even when tab is not active
  });
};

/**
 * Fetch system status and health metrics
 */
export const useSystemStatus = () => {
  return useQuery({
    queryKey: ["monitoring", "system-status"],
    queryFn: async (): Promise<SystemStatus> => {
      const response = await fetch("/api/monitoring/system-status");
      if (!response.ok) {
        throw new Error(
          `Failed to fetch system status: ${response.statusText}`,
        );
      }
      return response.json();
    },
    staleTime: 15 * 1000, // 15 seconds for system status (changes frequently)
    gcTime: 1 * 60 * 1000, // 1 minute cache
    refetchInterval: 15 * 1000, // Refetch every 15 seconds
    refetchIntervalInBackground: true,
  });
};

/**
 * Fetch a specific dashboard by ID
 */
export const useDashboard = (dashboardId: string) => {
  return useQuery({
    queryKey: ["monitoring", "dashboard", dashboardId],
    queryFn: async (): Promise<DashboardInfo> => {
      // Simulate API delay
      await delay(150);

      const dashboard = mockDashboards.find((d) => d.id === dashboardId);
      if (!dashboard) {
        throw new Error(`Dashboard with ID ${dashboardId} not found`);
      }

      return {
        ...dashboard,
        lastUpdate: new Date().toISOString(),
        metrics: {
          ...dashboard.metrics,
          totalSeries:
            dashboard.metrics.totalSeries + Math.floor(Math.random() * 5),
          activeCrawlers: Math.max(
            0,
            dashboard.metrics.activeCrawlers +
              Math.floor(Math.random() * 2) -
              1,
          ),
          dataPoints:
            dashboard.metrics.dataPoints + Math.floor(Math.random() * 50),
        },
      };
    },
    enabled: !!dashboardId,
    staleTime: 30 * 1000, // 30 seconds for individual dashboard
    gcTime: 2 * 60 * 1000, // 2 minutes cache
  });
};

/**
 * Refresh all monitoring data
 */
export const useRefreshMonitoring = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (): Promise<void> => {
      // Simulate API delay
      await delay(500);

      // In real implementation, this would trigger a background sync
      // For now, just invalidate the cache to force refetch
    },
    onSuccess: () => {
      // Invalidate all monitoring queries
      queryClient.invalidateQueries({ queryKey: ["monitoring"] });
    },
  });
};

/**
 * Get aggregated metrics across all dashboards
 */
export const useMonitoringMetrics = () => {
  const { data: dashboards = [] } = useDashboards();

  return {
    totalSeries: dashboards.reduce((sum, d) => sum + d.metrics.totalSeries, 0),
    activeCrawlers: dashboards.reduce(
      (sum, d) => sum + d.metrics.activeCrawlers,
      0,
    ),
    totalDataPoints: dashboards.reduce(
      (sum, d) => sum + d.metrics.dataPoints,
      0,
    ),
    averageUptime:
      dashboards.length > 0
        ? dashboards.reduce(
            (sum, d) => sum + parseFloat(d.metrics.uptime.replace("%", "")),
            0,
          ) / dashboards.length
        : 0,
    healthyDashboards: dashboards.filter((d) => d.status === "healthy").length,
    totalDashboards: dashboards.length,
  };
};
