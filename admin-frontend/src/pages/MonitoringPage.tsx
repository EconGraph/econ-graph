// REQUIREMENT: System monitoring interface that integrates with existing Grafana dashboards
// PURPOSE: Provide access to Grafana dashboards and embed key metrics for quick overview
// This leverages the existing monitoring infrastructure while providing admin-specific views
//
// UPDATED: Now uses React Query for data fetching, caching, and real-time updates
// This improves performance, reduces complexity, and provides better real-time monitoring

import React, { useState, useCallback } from "react";
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
  Tabs,
  Tab,
  IconButton,
  Tooltip,
  CircularProgress,
} from "@mui/material";
import {
  OpenInNew,
  Refresh,
  Dashboard,
  TrendingUp,
  Warning,
  CheckCircle,
  Error,
  Info,
  Launch,
  Fullscreen,
} from "@mui/icons-material";
import {
  useDashboards,
  useSystemStatus,
  useRefreshMonitoring,
  useMonitoringMetrics,
} from "../hooks/useMonitoring";

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
}

function TabPanel(props: TabPanelProps) {
  const { children, value, index, ...other } = props;

  return (
    <div
      role="tabpanel"
      hidden={value !== index}
      id={`monitoring-tabpanel-${index}`}
      aria-labelledby={`monitoring-tab-${index}`}
      {...other}
    >
      {value === index && <Box sx={{ p: 3 }}>{children}</Box>}
    </div>
  );
}

export default function MonitoringPage() {
  const [tabValue, setTabValue] = useState(0);

  // React Query hooks for data fetching
  const {
    data: dashboards = [],
    isLoading: dashboardsLoading,
    error: dashboardsError,
  } = useDashboards();

  const {
    data: systemStatus,
    isLoading: statusLoading,
    error: statusError,
  } = useSystemStatus();

  const refreshMonitoringMutation = useRefreshMonitoring();

  // Aggregated metrics from React Query
  const metrics = useMonitoringMetrics();

  const getStatusColor = useCallback((status: string) => {
    switch (status) {
      case "healthy":
        return "success";
      case "warning":
        return "warning";
      case "error":
        return "error";
      default:
        return "default";
    }
  }, []);

  const getStatusIcon = useCallback((status: string) => {
    switch (status) {
      case "healthy":
        return <CheckCircle />;
      case "warning":
        return <Warning />;
      case "error":
        return <Error />;
      default:
        return <Info />;
    }
  }, []);

  const handleTabChange = useCallback(
    (_event: React.SyntheticEvent, newValue: number) => {
      setTabValue(newValue);
    },
    [],
  );

  const handleRefresh = useCallback(async () => {
    try {
      await refreshMonitoringMutation.mutateAsync();
    } catch (error) {
      console.error("Failed to refresh monitoring data:", error);
    }
  }, [refreshMonitoringMutation]);

  const formatNumber = useCallback((num: number) => {
    return new Intl.NumberFormat().format(num);
  }, []);

  // Loading state
  if (dashboardsLoading || statusLoading) {
    return (
      <Box
        sx={{
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          height: 400,
        }}
      >
        <CircularProgress size={60} />
        <Typography variant="h6" sx={{ ml: 2 }}>
          Loading monitoring data...
        </Typography>
      </Box>
    );
  }

  // Error state
  if (dashboardsError || statusError) {
    return (
      <Box sx={{ p: 3 }}>
        <Alert severity="error" sx={{ mb: 2 }}>
          Failed to load monitoring data:{" "}
          {dashboardsError?.message || statusError?.message}
        </Alert>
        <Button variant="contained" onClick={handleRefresh}>
          Retry
        </Button>
      </Box>
    );
  }

  return (
    <Box sx={{ p: 3 }}>
      <Box
        sx={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          mb: 3,
        }}
      >
        <Typography variant="h4" gutterBottom>
          System Monitoring
        </Typography>
        <Box sx={{ display: "flex", gap: 1 }}>
          <Tooltip title="Refresh Data">
            <IconButton
              onClick={handleRefresh}
              disabled={refreshMonitoringMutation.isPending}
            >
              <Refresh />
            </IconButton>
          </Tooltip>
          <Button
            variant="outlined"
            startIcon={<Launch />}
            href="http://localhost:30001"
            target="_blank"
            rel="noopener noreferrer"
          >
            Open Grafana
          </Button>
        </Box>
      </Box>

      {/* System Status Overview */}
      {systemStatus && (
        <Grid container spacing={3} sx={{ mb: 3 }}>
          <Grid item xs={12} sm={6} md={3}>
            <Card>
              <CardContent>
                <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
                  {getStatusIcon(systemStatus.overall)}
                  <Typography variant="h6" sx={{ ml: 1 }}>
                    System Status
                  </Typography>
                </Box>
                <Chip
                  label={systemStatus.overall.toUpperCase()}
                  color={getStatusColor(systemStatus.overall) as any}
                  size="small"
                />
              </CardContent>
            </Card>
          </Grid>
          <Grid item xs={12} sm={6} md={3}>
            <Card>
              <CardContent>
                <Typography color="textSecondary" gutterBottom>
                  Active Alerts
                </Typography>
                <Typography variant="h4">{systemStatus.alerts}</Typography>
              </CardContent>
            </Card>
          </Grid>
          <Grid item xs={12} sm={6} md={3}>
            <Card>
              <CardContent>
                <Typography color="textSecondary" gutterBottom>
                  Total Series
                </Typography>
                <Typography variant="h4">
                  {formatNumber(metrics.totalSeries)}
                </Typography>
              </CardContent>
            </Card>
          </Grid>
          <Grid item xs={12} sm={6} md={3}>
            <Card>
              <CardContent>
                <Typography color="textSecondary" gutterBottom>
                  Active Crawlers
                </Typography>
                <Typography variant="h4">{metrics.activeCrawlers}</Typography>
              </CardContent>
            </Card>
          </Grid>
        </Grid>
      )}

      {/* Service Status */}
      {systemStatus && (
        <Paper sx={{ p: 2, mb: 3 }}>
          <Typography variant="h6" gutterBottom>
            Service Status
          </Typography>
          <Grid container spacing={2}>
            {Object.entries(systemStatus.services).map(([service, status]) => (
              <Grid item xs={6} sm={3} key={service}>
                <Box sx={{ display: "flex", alignItems: "center" }}>
                  {getStatusIcon(status)}
                  <Typography
                    variant="body2"
                    sx={{ ml: 1, textTransform: "capitalize" }}
                  >
                    {service}
                  </Typography>
                  <Chip
                    label={status}
                    color={getStatusColor(status) as any}
                    size="small"
                    sx={{ ml: 1 }}
                  />
                </Box>
              </Grid>
            ))}
          </Grid>
        </Paper>
      )}

      {/* Dashboard Tabs */}
      <Paper sx={{ width: "100%" }}>
        <Tabs
          value={tabValue}
          onChange={handleTabChange}
          aria-label="monitoring tabs"
        >
          <Tab label="Dashboards" />
          <Tab label="Metrics Overview" />
        </Tabs>

        <TabPanel value={tabValue} index={0}>
          <Grid container spacing={3}>
            {dashboards.map((dashboard) => (
              <Grid item xs={12} md={6} lg={4} key={dashboard.id}>
                <Card
                  sx={{
                    height: "100%",
                    display: "flex",
                    flexDirection: "column",
                  }}
                >
                  <CardContent sx={{ flexGrow: 1 }}>
                    <Box
                      sx={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "start",
                        mb: 2,
                      }}
                    >
                      <Typography variant="h6" component="h3">
                        {dashboard.title}
                      </Typography>
                      <Chip
                        label={dashboard.status}
                        color={getStatusColor(dashboard.status) as any}
                        size="small"
                      />
                    </Box>

                    <Typography
                      variant="body2"
                      color="text.secondary"
                      sx={{ mb: 2 }}
                    >
                      {dashboard.description}
                    </Typography>

                    <Grid container spacing={1} sx={{ mb: 2 }}>
                      <Grid item xs={6}>
                        <Typography variant="caption" color="text.secondary">
                          Series
                        </Typography>
                        <Typography variant="body2" fontWeight="medium">
                          {formatNumber(dashboard.metrics.totalSeries)}
                        </Typography>
                      </Grid>
                      <Grid item xs={6}>
                        <Typography variant="caption" color="text.secondary">
                          Crawlers
                        </Typography>
                        <Typography variant="body2" fontWeight="medium">
                          {dashboard.metrics.activeCrawlers}
                        </Typography>
                      </Grid>
                      <Grid item xs={6}>
                        <Typography variant="caption" color="text.secondary">
                          Data Points
                        </Typography>
                        <Typography variant="body2" fontWeight="medium">
                          {formatNumber(dashboard.metrics.dataPoints)}
                        </Typography>
                      </Grid>
                      <Grid item xs={6}>
                        <Typography variant="caption" color="text.secondary">
                          Uptime
                        </Typography>
                        <Typography variant="body2" fontWeight="medium">
                          {dashboard.metrics.uptime}
                        </Typography>
                      </Grid>
                    </Grid>

                    <Typography variant="caption" color="text.secondary">
                      Last updated:{" "}
                      {new Date(dashboard.lastUpdate).toLocaleString()}
                    </Typography>
                  </CardContent>

                  <Box sx={{ p: 2, pt: 0 }}>
                    <Button
                      variant="contained"
                      startIcon={<OpenInNew />}
                      href={dashboard.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      fullWidth
                      sx={{ mb: 1 }}
                    >
                      Open Dashboard
                    </Button>
                    <Button
                      variant="outlined"
                      startIcon={<Fullscreen />}
                      href={dashboard.embedUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      fullWidth
                    >
                      Embed View
                    </Button>
                  </Box>
                </Card>
              </Grid>
            ))}
          </Grid>
        </TabPanel>

        <TabPanel value={tabValue} index={1}>
          <Grid container spacing={3}>
            <Grid item xs={12} sm={6} md={3}>
              <Card>
                <CardContent>
                  <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
                    <Dashboard color="primary" />
                    <Typography variant="h6" sx={{ ml: 1 }}>
                      Total Dashboards
                    </Typography>
                  </Box>
                  <Typography variant="h4">
                    {metrics.totalDashboards}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    {metrics.healthyDashboards} healthy
                  </Typography>
                </CardContent>
              </Card>
            </Grid>

            <Grid item xs={12} sm={6} md={3}>
              <Card>
                <CardContent>
                  <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
                    <TrendingUp color="success" />
                    <Typography variant="h6" sx={{ ml: 1 }}>
                      Total Series
                    </Typography>
                  </Box>
                  <Typography variant="h4">
                    {formatNumber(metrics.totalSeries)}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    Across all dashboards
                  </Typography>
                </CardContent>
              </Card>
            </Grid>

            <Grid item xs={12} sm={6} md={3}>
              <Card>
                <CardContent>
                  <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
                    <CheckCircle color="info" />
                    <Typography variant="h6" sx={{ ml: 1 }}>
                      Data Points
                    </Typography>
                  </Box>
                  <Typography variant="h4">
                    {formatNumber(metrics.totalDataPoints)}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    Total collected
                  </Typography>
                </CardContent>
              </Card>
            </Grid>

            <Grid item xs={12} sm={6} md={3}>
              <Card>
                <CardContent>
                  <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
                    <Warning color="warning" />
                    <Typography variant="h6" sx={{ ml: 1 }}>
                      Average Uptime
                    </Typography>
                  </Box>
                  <Typography variant="h4">
                    {metrics.averageUptime.toFixed(1)}%
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    System reliability
                  </Typography>
                </CardContent>
              </Card>
            </Grid>
          </Grid>
        </TabPanel>
      </Paper>
    </Box>
  );
}
