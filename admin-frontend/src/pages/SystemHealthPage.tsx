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
    return systemHealth.status || "unknown";
  }, [systemHealth]);

  // Convert GraphQL data to component format based on actual backend schema
  // Note: This is a simplified implementation until backend provides service-level data
  // See GitHub issue #126 for the full feature request
  const healthMetrics = useMemo(() => {
    if (!systemHealth?.metrics) return [];

    // Create basic health metrics based on available backend data
    return [
      {
        name: "System Status",
        status: systemHealth.status === "healthy" ? "healthy" : "warning",
        value:
          systemHealth.status === "healthy"
            ? "All systems operational"
            : "System issues detected",
        description: "Overall system health",
        trend: "stable" as const,
        lastCheck: systemHealth.last_updated,
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now",
      },
      {
        name: "User Activity",
        status: "healthy",
        value: `${systemHealth.metrics.active_users} active users`,
        description: "Current user activity",
        trend: "stable" as const,
        lastCheck: systemHealth.last_updated,
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&var-metric=users",
      },
      {
        name: "Database",
        status: "healthy",
        value: `${systemHealth.metrics.database_size_mb.toFixed(1)} MB`,
        description: "Database size",
        trend: "stable" as const,
        lastCheck: systemHealth.last_updated,
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&var-metric=database",
      },
    ];
  }, [systemHealth]);

  const services = useMemo(() => {
    if (!systemHealth?.metrics) return [];

    // Create basic service information based on available backend data
    return [
      {
        name: "Backend API",
        status: "running",
        uptime: "Unknown", // Not available in current schema
        version: "Unknown", // Not available in current schema
        resources: {
          cpu: 0, // Not available in current schema
          memory: 0, // Not available in current schema
          disk: 0, // Not available in current schema
        },
      },
      {
        name: "PostgreSQL",
        status: "running",
        uptime: "Unknown", // Not available in current schema
        version: "Unknown", // Not available in current schema
        resources: {
          cpu: 0, // Not available in current schema
          memory: 0, // Not available in current schema
          disk: 0, // Not available in current schema
        },
      },
      {
        name: "Data Crawler",
        status: "running",
        uptime: "Unknown", // Not available in current schema
        version: "Unknown", // Not available in current schema
        resources: {
          cpu: 0, // Not available in current schema
          memory: 0, // Not available in current schema
          disk: 0, // Not available in current schema
        },
      },
    ];
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
          <Typography variant="h4" gutterBottom>
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
      >
        {healthMetrics.map((metric: any) => (
          <Grid item xs={12} sm={6} md={4} key={metric.name}>
            <Card
              data-testid={`health-metric-card-${metric.name.toLowerCase().replace(/\s+/g, "-")}`}
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
      <Paper sx={{ p: 3, mb: 3 }} data-testid="services-status-section">
        <Typography variant="h6" gutterBottom>
          Service Status
        </Typography>
        <List data-testid="services-list">
          {services.map((service: any, index: number) => (
            <React.Fragment key={service.name}>
              <ListItem>
                <ListItemIcon>{getStatusIcon(service.status)}</ListItemIcon>
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
                          <Memory fontSize="small" color="action" />
                          <Typography variant="caption">
                            CPU: {service.resources.cpu}%
                          </Typography>
                          <LinearProgress
                            variant="determinate"
                            value={service.resources.cpu}
                            color={getResourceColor(service.resources.cpu)}
                            sx={{ width: 60, height: 4 }}
                          />
                        </Box>
                        <Box
                          sx={{ display: "flex", alignItems: "center", gap: 1 }}
                          data-testid={`service-${service.name.toLowerCase().replace(/\s+/g, "-")}-ram-usage`}
                        >
                          <Storage fontSize="small" color="action" />
                          <Typography variant="caption">
                            RAM: {service.resources.memory}%
                          </Typography>
                          <LinearProgress
                            variant="determinate"
                            value={service.resources.memory}
                            color={getResourceColor(service.resources.memory)}
                            sx={{ width: 60, height: 4 }}
                          />
                        </Box>
                        <Box
                          sx={{ display: "flex", alignItems: "center", gap: 1 }}
                          data-testid={`service-${service.name.toLowerCase().replace(/\s+/g, "-")}-disk-usage`}
                        >
                          <Storage fontSize="small" color="action" />
                          <Typography variant="caption">
                            Disk: {service.resources.disk}%
                          </Typography>
                          <LinearProgress
                            variant="determinate"
                            value={service.resources.disk}
                            color={getResourceColor(service.resources.disk)}
                            sx={{ width: 60, height: 4 }}
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
      <Paper sx={{ p: 3 }} data-testid="quick-actions-section">
        <Typography variant="h6" gutterBottom>
          Quick Actions
        </Typography>
        <Grid container spacing={2} data-testid="quick-actions-grid">
          <Grid item xs={12} sm={6} md={3}>
            <Button
              fullWidth
              variant="outlined"
              startIcon={<Web />}
              href="http://localhost:30001/d/econgraph-overview/econgraph-platform-overview"
              target="_blank"
              rel="noopener noreferrer"
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
            >
              Security Events
            </Button>
          </Grid>
        </Grid>
      </Paper>
    </Box>
  );
}
