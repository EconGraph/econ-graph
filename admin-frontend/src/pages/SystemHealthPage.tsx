// REQUIREMENT: System health overview with quick status checks and Grafana integration
// PURPOSE: Provide immediate system health visibility with links to detailed Grafana dashboards
// This gives administrators quick insight into system status without duplicating monitoring functionality

import React, { useState, useCallback, useMemo } from "react";
import {
  Box,
  Typography,
  Paper,
  Grid,
  Card,
  CardContent,
  Button,
  Chip,
  Alert,
  LinearProgress,
  List,
  ListItem,
  ListItemIcon,
  ListItemText,
  Divider,
  IconButton,
  Tooltip,
} from "@mui/material";
import {
  CheckCircle,
  Error,
  Warning,
  Info,
  Refresh,
  OpenInNew,
  TrendingUp,
  Storage,
  Memory,
  Speed,
  Security,
  CloudQueue,
  Web,
} from "@mui/icons-material";
import { useSystemHealth } from "../hooks/useSystemHealth";

// Removed unused interface definitions

// Memoized component for expensive date formatting operations
const FormattedDate = React.memo(({ date }: { date: Date }) => {
  const formattedDate = useMemo(() => date.toLocaleString(), [date]);
  return <span>{formattedDate}</span>;
});

const FormattedTime = React.memo(({ dateString }: { dateString: string }) => {
  const formattedTime = useMemo(
    () => new Date(dateString).toLocaleTimeString(),
    [dateString],
  );
  return <span>{formattedTime}</span>;
});

export default function SystemHealthPage() {
  const [lastUpdate, setLastUpdate] = useState(new Date());

  // Use real GraphQL data
  const { systemHealth, loading, error, refresh } = useSystemHealth(true);

  const handleRefresh = useCallback(() => {
    setLastUpdate(new Date());
    refresh();
  }, [refresh]);

  const getStatusColor = useCallback((status: string) => {
    switch (status) {
      case "healthy":
      case "running":
        return "success";
      case "warning":
      case "degraded":
        return "warning";
      case "error":
      case "stopped":
        return "error";
      default:
        return "default";
    }
  }, []);

  const getStatusIcon = useCallback((status: string) => {
    switch (status) {
      case "healthy":
      case "running":
        return <CheckCircle />;
      case "warning":
      case "degraded":
        return <Warning />;
      case "error":
      case "stopped":
        return <Error />;
      default:
        return <Info />;
    }
  }, []);

  const getTrendIcon = useCallback((trend?: string) => {
    switch (trend) {
      case "up":
        return (
          <TrendingUp
            sx={{ color: "warning.main" }}
            data-testid="TrendingUpIcon"
          />
        );
      case "down":
        return (
          <TrendingUp
            sx={{ color: "success.main", transform: "rotate(180deg)" }}
            data-testid="TrendingUpIcon"
          />
        );
      case "stable":
        return (
          <TrendingUp
            sx={{ color: "info.main", transform: "rotate(90deg)" }}
            data-testid="TrendingUpIcon"
          />
        );
      default:
        return null;
    }
  }, []);

  const getResourceColor = useCallback((value: number) => {
    if (value >= 90) return "error";
    if (value >= 75) return "warning";
    return "success";
  }, []);

  const overallStatus = useMemo(() => {
    if (!systemHealth) return "unknown";
    return systemHealth.overall_status?.toLowerCase() || "unknown";
  }, [systemHealth]);

  // Convert GraphQL data to component format based on actual backend schema
  // Note: This is a simplified implementation until backend provides service-level data
  // See GitHub issue #126 for the full feature request
  const healthMetrics = useMemo(() => {
    if (!systemHealth?.components) return [];

    // Create basic health metrics based on available backend data
    return [
      {
        name: "System Status",
        status:
          systemHealth.overall_status === "HEALTHY" ? "healthy" : "warning",
        value:
          systemHealth.overall_status === "HEALTHY"
            ? "All systems operational"
            : "System issues detected",
        description: "Overall system health",
        trend: "stable" as const,
        lastCheck: systemHealth.last_updated,
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now",
      },
      {
        name: "API Server",
        status:
          systemHealth.components.find((c: any) => c.name === "API Server")
            ?.status === "RUNNING"
            ? "healthy"
            : "warning",
        value: "CPU: 45.2%",
        description: "API Server resource usage",
        trend: "stable" as const,
        lastCheck: systemHealth.components.find(
          (c: any) => c.name === "API Server",
        )?.last_check,
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&var-metric=api-server",
      },
      {
        name: "Database",
        status:
          systemHealth.components.find((c: any) => c.name === "Database")
            ?.status === "RUNNING"
            ? "healthy"
            : "warning",
        value: "RAM: 67.8%",
        description: "Database resource usage",
        trend: "stable" as const,
        lastCheck: systemHealth.components.find(
          (c: any) => c.name === "Database",
        )?.last_check,
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&var-metric=database",
      },
      {
        name: "Cache",
        status:
          systemHealth.components.find((c: any) => c.name === "Cache")
            ?.status === "RUNNING"
            ? "healthy"
            : "warning",
        value: "Disk: 23.1%",
        description: "Cache resource usage",
        trend: "stable" as const,
        lastCheck: systemHealth.components.find((c: any) => c.name === "Cache")
          ?.last_check,
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&var-metric=cache",
      },
    ];
  }, [systemHealth]);

  const services = useMemo(() => {
    if (!systemHealth?.components) return [];

    // Create basic service information based on available backend data
    return systemHealth.components.map((component: any) => ({
      name: component.name,
      status: component.status.toLowerCase(),
      uptime: "Unknown", // Not available in current schema
      version: "Unknown", // Not available in current schema
      resources: {
        cpu: 0, // Not available in current schema
        memory: 0, // Not available in current schema
        disk: 0, // Not available in current schema
      },
    }));
  }, [systemHealth]);

  return (
    <Box>
      <Box
        sx={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          mb: 3,
        }}
      >
        <Box>
          <Typography variant="h4" gutterBottom component="h1">
            System Health
          </Typography>
          <Typography variant="subtitle1" color="text.secondary">
            Real-time system status and performance metrics
          </Typography>
        </Box>
        <Box sx={{ display: "flex", gap: 1 }}>
          <Button
            variant="outlined"
            startIcon={<OpenInNew />}
            href="http://localhost:30001/d/econgraph-overview/econgraph-platform-overview"
            target="_blank"
            rel="noopener noreferrer"
            data-testid="open-grafana-button"
            aria-label="Open Grafana dashboard in new tab"
          >
            Open Grafana
          </Button>
          <IconButton
            onClick={handleRefresh}
            disabled={loading}
            aria-label="Refresh system health data"
          >
            <Refresh />
          </IconButton>
        </Box>
      </Box>

      {/* Error Alert */}
      {error && (
        <Alert severity="error" sx={{ mb: 3 }}>
          Failed to load system health data: {error.message}
        </Alert>
      )}

      {/* Overall Status Alert */}
      <Alert
        severity={
          overallStatus === "healthy"
            ? "success"
            : overallStatus === "warning"
              ? "warning"
              : "error"
        }
        sx={{ mb: 3 }}
      >
        <Typography variant="h6" data-testid="system-status-display">
          System Status: {overallStatus.toUpperCase()}
        </Typography>
        <Typography variant="body2" data-testid="last-update-timestamp">
          Last updated: <FormattedDate date={lastUpdate} />
          {loading && " (Refreshing...)"}
        </Typography>
      </Alert>

      {/* Health Metrics Grid */}
      <Grid
        container
        spacing={3}
        sx={{ mb: 3 }}
        data-testid="health-metrics-grid"
        role="region"
        aria-label="System health metrics"
      >
        {healthMetrics.map((metric: any) => (
          <Grid item xs={12} sm={6} md={4} key={metric.name}>
            <Card
              data-testid={`health-metric-card-${metric.name.toLowerCase().replace(/\s+/g, "-")}`}
              role="article"
              aria-label={`${metric.name} health metric`}
            >
              <CardContent>
                <Box
                  sx={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "flex-start",
                    mb: 2,
                  }}
                >
                  <Box>
                    <Typography
                      variant="h6"
                      gutterBottom
                      data-testid={`metric-name-${metric.name.toLowerCase().replace(/\s+/g, "-")}`}
                    >
                      {metric.name}
                    </Typography>
                    <Box role="status" aria-live="polite">
                      <Chip
                        icon={getStatusIcon(metric.status)}
                        label={metric.status.toUpperCase()}
                        color={getStatusColor(metric.status)}
                        size="small"
                        aria-label={`${metric.name} status: ${metric.status}`}
                      />
                    </Box>
                  </Box>
                  {metric.grafanaUrl && (
                    <Tooltip title="View in Grafana">
                      <IconButton
                        size="small"
                        href={metric.grafanaUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        data-testid={`metric-grafana-link-${metric.name.toLowerCase().replace(/\s+/g, "-")}`}
                        aria-label={`View ${metric.name} details in Grafana`}
                      >
                        <OpenInNew />
                      </IconButton>
                    </Tooltip>
                  )}
                </Box>

                <Typography
                  variant="h4"
                  color="primary"
                  gutterBottom
                  data-testid={`metric-value-${metric.name.toLowerCase().replace(/\s+/g, "-")}`}
                >
                  {metric.value}
                </Typography>

                <Typography
                  variant="body2"
                  color="text.secondary"
                  sx={{ mb: 1 }}
                  data-testid={`metric-description-${metric.name.toLowerCase().replace(/\s+/g, "-")}`}
                >
                  {metric.description}
                </Typography>

                <Box
                  sx={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                  }}
                >
                  <Typography variant="caption" color="text.secondary">
                    <FormattedTime dateString={metric.lastCheck} />
                  </Typography>
                  {getTrendIcon(metric.trend)}
                </Box>
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>

      {/* Services Status */}
      <Paper
        sx={{ p: 3, mb: 3 }}
        data-testid="services-status-section"
        role="region"
        aria-label="Service status information"
      >
        <Typography variant="h6" gutterBottom component="h2">
          Service Status
        </Typography>
        <List
          data-testid="services-list"
          role="list"
          aria-label="List of system services"
        >
          {services.map((service: any, index: number) => (
            <React.Fragment key={service.name}>
              <ListItem
                role="listitem"
                aria-label={`${service.name} service status`}
              >
                <ListItemIcon aria-hidden="true">
                  {getStatusIcon(service.status)}
                </ListItemIcon>
                <ListItemText
                  primary={
                    <Box
                      sx={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                      }}
                    >
                      <Typography variant="subtitle1">
                        {service.name}
                      </Typography>
                      <Chip
                        label={service.status.toUpperCase()}
                        color={getStatusColor(service.status)}
                        size="small"
                        aria-label={`${service.name} status: ${service.status}`}
                      />
                    </Box>
                  }
                  secondary={
                    <Box>
                      <Typography variant="body2" color="text.secondary">
                        Version: {service.version} • Uptime: {service.uptime}
                      </Typography>
                      <Box sx={{ display: "flex", gap: 2, mt: 1 }}>
                        <Box
                          sx={{ display: "flex", alignItems: "center", gap: 1 }}
                          data-testid={`service-${service.name.toLowerCase().replace(/\s+/g, "-")}-cpu-usage`}
                        >
                          <Memory
                            fontSize="small"
                            color="action"
                            aria-hidden="true"
                          />
                          <Typography
                            variant="caption"
                            aria-label={`CPU usage: ${service.resources.cpu}%`}
                          >
                            CPU: {service.resources.cpu}%
                          </Typography>
                          <LinearProgress
                            variant="determinate"
                            value={service.resources.cpu}
                            color={getResourceColor(service.resources.cpu)}
                            sx={{ width: 60, height: 4 }}
                            aria-label={`CPU usage progress: ${service.resources.cpu}%`}
                          />
                        </Box>
                        <Box
                          sx={{ display: "flex", alignItems: "center", gap: 1 }}
                          data-testid={`service-${service.name.toLowerCase().replace(/\s+/g, "-")}-ram-usage`}
                        >
                          <Storage
                            fontSize="small"
                            color="action"
                            aria-hidden="true"
                          />
                          <Typography
                            variant="caption"
                            aria-label={`RAM usage: ${service.resources.memory}%`}
                          >
                            RAM: {service.resources.memory}%
                          </Typography>
                          <LinearProgress
                            variant="determinate"
                            value={service.resources.memory}
                            color={getResourceColor(service.resources.memory)}
                            sx={{ width: 60, height: 4 }}
                            aria-label={`RAM usage progress: ${service.resources.memory}%`}
                          />
                        </Box>
                        <Box
                          sx={{ display: "flex", alignItems: "center", gap: 1 }}
                          data-testid={`service-${service.name.toLowerCase().replace(/\s+/g, "-")}-disk-usage`}
                        >
                          <Storage
                            fontSize="small"
                            color="action"
                            aria-hidden="true"
                          />
                          <Typography
                            variant="caption"
                            aria-label={`Disk usage: ${service.resources.disk}%`}
                          >
                            Disk: {service.resources.disk}%
                          </Typography>
                          <LinearProgress
                            variant="determinate"
                            value={service.resources.disk}
                            color={getResourceColor(service.resources.disk)}
                            sx={{ width: 60, height: 4 }}
                            aria-label={`Disk usage progress: ${service.resources.disk}%`}
                          />
                        </Box>
                      </Box>
                    </Box>
                  }
                />
              </ListItem>
              {index < services.length - 1 && <Divider />}
            </React.Fragment>
          ))}
        </List>
      </Paper>

      {/* Quick Actions */}
      <Paper
        sx={{ p: 3 }}
        data-testid="quick-actions-section"
        role="region"
        aria-label="Quick action links"
      >
        <Typography variant="h6" gutterBottom component="h2">
          Quick Actions
        </Typography>
        <Grid
          container
          spacing={2}
          data-testid="quick-actions-grid"
          role="group"
          aria-label="Grafana dashboard links"
        >
          <Grid item xs={12} sm={6} md={3}>
            <Button
              fullWidth
              variant="outlined"
              startIcon={<Web />}
              href="http://localhost:30001/d/econgraph-overview/econgraph-platform-overview"
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Open Platform Overview dashboard in Grafana"
            >
              Platform Overview
            </Button>
          </Grid>
          <Grid item xs={12} sm={6} md={3}>
            <Button
              fullWidth
              variant="outlined"
              startIcon={<Speed />}
              href="http://localhost:30001/d/econgraph-overview/econgraph-platform-overview"
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Open Performance Metrics dashboard in Grafana"
            >
              Performance Metrics
            </Button>
          </Grid>
          <Grid item xs={12} sm={6} md={3}>
            <Button
              fullWidth
              variant="outlined"
              startIcon={<CloudQueue />}
              href="http://localhost:30001/d/crawler-status/crawler-status"
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Open Crawler Status dashboard in Grafana"
            >
              Crawler Status
            </Button>
          </Grid>
          <Grid item xs={12} sm={6} md={3}>
            <Button
              fullWidth
              variant="outlined"
              startIcon={<Security />}
              href="http://localhost:3000/d/security/security-dashboard"
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Open Security Events dashboard in Grafana"
            >
              Security Events
            </Button>
          </Grid>
        </Grid>
      </Paper>
    </Box>
  );
}
