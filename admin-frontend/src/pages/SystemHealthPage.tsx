// REQUIREMENT: System health overview with quick status checks and Grafana integration
// PURPOSE: Provide immediate system health visibility with links to detailed Grafana dashboards
// This gives administrators quick insight into system status without duplicating monitoring functionality

import React, { useState, useEffect } from "react";
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

interface HealthMetric {
  name: string;
  status: "healthy" | "warning" | "error" | "unknown";
  value: string;
  description: string;
  trend?: "up" | "down" | "stable";
  lastCheck: string;
  grafanaUrl?: string;
}

interface ServiceStatus {
  name: string;
  status: "running" | "stopped" | "degraded" | "unknown";
  uptime: string;
  version: string;
  resources: {
    cpu: number;
    memory: number;
    disk: number;
  };
}

export default function SystemHealthPage() {
  const [loading, setLoading] = useState(true);
  const [lastUpdate, setLastUpdate] = useState(new Date());
  const [healthMetrics, setHealthMetrics] = useState<HealthMetric[]>([]);
  const [services, setServices] = useState<ServiceStatus[]>([]);

  // Mock data - in real implementation, this would fetch from system APIs
  useEffect(() => {
    const mockHealthMetrics: HealthMetric[] = [
      {
        name: "System Uptime",
        status: "healthy",
        value: "99.9%",
        description: "Overall system availability",
        trend: "stable",
        lastCheck: "2024-01-15T10:30:00Z",
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&var-service=all",
      },
      {
        name: "Response Time",
        status: "healthy",
        value: "120ms",
        description: "Average API response time",
        trend: "down",
        lastCheck: "2024-01-15T10:30:00Z",
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&var-metric=response_time",
      },
      {
        name: "Database Connections",
        status: "warning",
        value: "85%",
        description: "Active database connections",
        trend: "up",
        lastCheck: "2024-01-15T10:30:00Z",
        grafanaUrl:
          "http://localhost:30001/d/database-statistics/database-statistics?orgId=1&from=now-1h&to=now&var-metric=db_connections",
      },
      {
        name: "Memory Usage",
        status: "healthy",
        value: "68%",
        description: "System memory utilization",
        trend: "stable",
        lastCheck: "2024-01-15T10:30:00Z",
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&var-metric=memory",
      },
      {
        name: "Disk Space",
        status: "warning",
        value: "78%",
        description: "Available disk space",
        trend: "up",
        lastCheck: "2024-01-15T10:30:00Z",
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&var-metric=disk",
      },
      {
        name: "Active Users",
        status: "healthy",
        value: "24",
        description: "Currently active users",
        trend: "stable",
        lastCheck: "2024-01-15T10:30:00Z",
        grafanaUrl:
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview?orgId=1&from=now-1h&to=now&var-metric=active_users",
      },
    ];

    const mockServices: ServiceStatus[] = [
      {
        name: "Backend API",
        status: "running",
        uptime: "7d 12h 30m",
        version: "v1.2.3",
        resources: { cpu: 45, memory: 62, disk: 12 },
      },
      {
        name: "PostgreSQL",
        status: "running",
        uptime: "7d 12h 30m",
        version: "14.8",
        resources: { cpu: 25, memory: 78, disk: 45 },
      },
      {
        name: "Data Crawler",
        status: "degraded",
        uptime: "2d 8h 15m",
        version: "v1.1.0",
        resources: { cpu: 85, memory: 45, disk: 8 },
      },
      {
        name: "Grafana",
        status: "running",
        uptime: "7d 12h 30m",
        version: "10.2.0",
        resources: { cpu: 15, memory: 35, disk: 5 },
      },
      {
        name: "NGINX",
        status: "running",
        uptime: "7d 12h 30m",
        version: "1.24.0",
        resources: { cpu: 5, memory: 12, disk: 2 },
      },
    ];

    setHealthMetrics(mockHealthMetrics);
    setServices(mockServices);
    setLoading(false);
  }, []);

  const handleRefresh = () => {
    setLoading(true);
    // In real implementation, this would refresh data from system APIs
    setTimeout(() => {
      setLastUpdate(new Date());
      setLoading(false);
    }, 1000);
  };

  const getStatusColor = (status: string) => {
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
  };

  const getStatusIcon = (status: string) => {
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
  };

  const getTrendIcon = (trend?: string) => {
    switch (trend) {
      case "up":
        return <TrendingUp sx={{ color: "warning.main" }} />;
      case "down":
        return (
          <TrendingUp
            sx={{ color: "success.main", transform: "rotate(180deg)" }}
          />
        );
      case "stable":
        return (
          <TrendingUp sx={{ color: "info.main", transform: "rotate(90deg)" }} />
        );
      default:
        return null;
    }
  };

  const getResourceColor = (value: number) => {
    if (value >= 90) return "error";
    if (value >= 75) return "warning";
    return "success";
  };

  const overallStatus = services.some((s) => s.status === "stopped")
    ? "error"
    : services.some((s) => s.status === "degraded")
      ? "warning"
      : "healthy";

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
        <Typography variant="h6">
          System Status: {overallStatus.toUpperCase()}
        </Typography>
        <Typography variant="body2">
          Last updated: {lastUpdate.toLocaleString()}
          {loading && " (Refreshing...)"}
        </Typography>
      </Alert>

      {/* Health Metrics Grid */}
      <Grid container spacing={3} sx={{ mb: 3 }}>
        {healthMetrics.map((metric) => (
          <Grid item xs={12} sm={6} md={4} key={metric.name}>
            <Card>
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
                    <Typography variant="h6" gutterBottom>
                      {metric.name}
                    </Typography>
                    <Chip
                      icon={getStatusIcon(metric.status)}
                      label={metric.status.toUpperCase()}
                      color={getStatusColor(metric.status)}
                      size="small"
                    />
                  </Box>
                  {metric.grafanaUrl && (
                    <Tooltip title="View in Grafana">
                      <IconButton
                        size="small"
                        href={metric.grafanaUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        <OpenInNew />
                      </IconButton>
                    </Tooltip>
                  )}
                </Box>

                <Typography variant="h4" color="primary" gutterBottom>
                  {metric.value}
                </Typography>

                <Typography
                  variant="body2"
                  color="text.secondary"
                  sx={{ mb: 1 }}
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
                    {new Date(metric.lastCheck).toLocaleTimeString()}
                  </Typography>
                  {getTrendIcon(metric.trend)}
                </Box>
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>

      {/* Services Status */}
      <Paper sx={{ p: 3, mb: 3 }}>
        <Typography variant="h6" gutterBottom>
          Service Status
        </Typography>
        <List>
          {services.map((service, index) => (
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
      <Paper sx={{ p: 3 }}>
        <Typography variant="h6" gutterBottom>
          Quick Actions
        </Typography>
        <Grid container spacing={2}>
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
