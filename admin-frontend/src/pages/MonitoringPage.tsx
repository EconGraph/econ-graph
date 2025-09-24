// REQUIREMENT: System monitoring interface that integrates with existing Grafana dashboards
// PURPOSE: Provide access to Grafana dashboards and embed key metrics for quick overview
// This leverages the existing monitoring infrastructure while providing admin-specific views

import React, { useState, useEffect, useCallback, useMemo } from "react";
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

interface DashboardInfo {
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
  const [loading, setLoading] = useState(true);
  const [dashboards, setDashboards] = useState<DashboardInfo[]>([]);
  const [systemStatus] = useState({
    overall: "healthy" as "healthy" | "warning" | "error",
    services: {
      backend: "healthy",
      database: "healthy",
      crawler: "warning",
      grafana: "healthy",
    },
    alerts: 2,
  });

  // Memoize calculated values
  const totalSeries = useMemo(
    () => dashboards.reduce((sum, d) => sum + d.metrics.totalSeries, 0),
    [dashboards],
  );

  const activeCrawlers = useMemo(
    () => dashboards.reduce((sum, d) => sum + d.metrics.activeCrawlers, 0),
    [dashboards],
  );

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

  const serviceStatusCards = useMemo(
    () =>
      Object.entries(systemStatus.services).map(([service, status]) => (
        <Grid item xs={12} sm={6} md={3} key={service}>
          <Card>
            <CardContent>
              <Box
                sx={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                }}
              >
                <Typography variant="h6">{service}</Typography>
                <Box
                  sx={{
                    display: "flex",
                    alignItems: "center",
                    gap: 1,
                  }}
                >
                  <Box
                    sx={{
                      width: 8,
                      height: 8,
                      borderRadius: "50%",
                      backgroundColor: getStatusColor(status),
                    }}
                  />
                  <Typography variant="body2">
                    {status.toUpperCase()}
                  </Typography>
                </Box>
              </Box>
            </CardContent>
          </Card>
        </Grid>
      )),
    [systemStatus.services, getStatusColor],
  );

  // Real Grafana dashboards from our existing monitoring infrastructure
  useEffect(() => {
    const grafanaDashboards: DashboardInfo[] = [
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
          totalSeries: 0, // Will be populated from Prometheus metrics
          activeCrawlers: 0, // Will be populated from crawler status
          dataPoints: 0, // Will be populated from database statistics
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
          totalSeries: 0, // Will be populated from database metrics
          activeCrawlers: 0,
          dataPoints: 0, // Will be populated from table statistics
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
          activeCrawlers: 0, // Will be populated from crawler metrics
          dataPoints: 0,
          uptime: "100%",
        },
      },
    ];

    setDashboards(grafanaDashboards);
    setLoading(false);
  }, []);

  const handleTabChange = useCallback(
    (event: React.SyntheticEvent, newValue: number) => {
      setTabValue(newValue);
    },
    [],
  );

  const handleRefresh = useCallback(() => {
    setLoading(true);
    // In real implementation, this would refresh data from Grafana API
    setTimeout(() => setLoading(false), 1000);
  }, []);

  const formatNumber = useCallback((num: number) => {
    return new Intl.NumberFormat().format(num);
  }, []);

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
            System Monitoring
          </Typography>
          <Typography variant="subtitle1" color="text.secondary">
            Grafana dashboards and system metrics
          </Typography>
        </Box>
        <Box sx={{ display: "flex", gap: 1 }}>
          <Button
            variant="outlined"
            startIcon={<Launch />}
            href="http://localhost:30001"
            target="_blank"
            rel="noopener noreferrer"
            data-testid="open-grafana-button"
          >
            Open Grafana
          </Button>
          <IconButton
            onClick={handleRefresh}
            disabled={loading}
            aria-label="Refresh monitoring data"
            data-testid="monitor-refresh-button"
          >
            <Refresh />
          </IconButton>
        </Box>
      </Box>

      {/* System Status Overview */}
      <Grid container spacing={3} sx={{ mb: 3 }}>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
                <Chip
                  icon={getStatusIcon(systemStatus.overall)}
                  label={systemStatus.overall.toUpperCase()}
                  color={getStatusColor(systemStatus.overall)}
                  size="small"
                />
              </Box>
              <Typography variant="h6">Overall Status</Typography>
              <Typography variant="body2" color="text.secondary">
                System Health
              </Typography>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Typography variant="h4" color="error">
                {systemStatus.alerts}
              </Typography>
              <Typography variant="h6">Active Alerts</Typography>
              <Typography variant="body2" color="text.secondary">
                Requires attention
              </Typography>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Typography variant="h4">{totalSeries}</Typography>
              <Typography variant="h6">Total Series</Typography>
              <Typography variant="body2" color="text.secondary">
                Across all dashboards
              </Typography>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Typography variant="h4">{activeCrawlers}</Typography>
              <Typography variant="h6">Active Crawlers</Typography>
              <Typography variant="body2" color="text.secondary">
                Data collection
              </Typography>
            </CardContent>
          </Card>
        </Grid>
      </Grid>

      {/* Service Status */}
      <Paper sx={{ p: 3, mb: 3 }}>
        <Typography
          variant="h6"
          gutterBottom
          data-testid="service-status-title"
        >
          Service Status
        </Typography>
        <Grid container spacing={2}>
          {useMemo(
            () =>
              Object.entries(systemStatus.services).map(([service, status]) => (
                <Grid item xs={12} sm={6} md={3} key={service}>
                  <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
                    <Chip
                      icon={getStatusIcon(status)}
                      label={service.toUpperCase()}
                      color={getStatusColor(status)}
                      size="small"
                    />
                  </Box>
                </Grid>
              )),
            [systemStatus.services, getStatusIcon, getStatusColor],
          )}
        </Grid>
      </Paper>

      <Paper sx={{ width: "100%" }}>
        <Box sx={{ borderBottom: 1, borderColor: "divider" }}>
          <Tabs
            value={tabValue}
            onChange={handleTabChange}
            data-testid="monitor-tabs"
          >
            <Tab label="Dashboard Overview" data-testid="tab-overview" />
            <Tab label="Embedded Views" data-testid="tab-embedded" />
            <Tab label="Quick Metrics" data-testid="tab-metrics" />
          </Tabs>
        </Box>

        {/* Dashboard Overview Tab */}
        <TabPanel value={tabValue} index={0}>
          <Typography variant="h6" gutterBottom>
            Available Grafana Dashboards
          </Typography>
          <Grid container spacing={3}>
            {dashboards.map((dashboard) => (
              <Grid item xs={12} md={6} lg={4} key={dashboard.id}>
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
                          {dashboard.title}
                        </Typography>
                        <Chip
                          icon={getStatusIcon(dashboard.status)}
                          label={dashboard.status.toUpperCase()}
                          color={getStatusColor(dashboard.status)}
                          size="small"
                        />
                      </Box>
                      <IconButton
                        size="small"
                        href={dashboard.url}
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        <OpenInNew />
                      </IconButton>
                    </Box>

                    <Typography
                      variant="body2"
                      color="text.secondary"
                      sx={{ mb: 2 }}
                    >
                      {dashboard.description}
                    </Typography>

                    <Box sx={{ mb: 2 }}>
                      <Typography variant="caption" color="text.secondary">
                        Last Update:{" "}
                        {new Date(dashboard.lastUpdate).toLocaleString()}
                      </Typography>
                    </Box>

                    <Box sx={{ display: "flex", gap: 1, flexWrap: "wrap" }}>
                      <Chip
                        label={`${formatNumber(dashboard.metrics.totalSeries)} series`}
                        size="small"
                        variant="outlined"
                      />
                      <Chip
                        label={`${dashboard.metrics.activeCrawlers} crawlers`}
                        size="small"
                        variant="outlined"
                      />
                      <Chip
                        label={dashboard.metrics.uptime}
                        size="small"
                        variant="outlined"
                        color="success"
                      />
                    </Box>

                    <Box sx={{ mt: 2, display: "flex", gap: 1 }}>
                      <Button
                        size="small"
                        variant="contained"
                        href={dashboard.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        startIcon={<Dashboard />}
                      >
                        Open Dashboard
                      </Button>
                      <Button
                        size="small"
                        variant="outlined"
                        href={dashboard.embedUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        startIcon={<Fullscreen />}
                      >
                        Embed View
                      </Button>
                    </Box>
                  </CardContent>
                </Card>
              </Grid>
            ))}
          </Grid>
        </TabPanel>

        {/* Embedded Views Tab */}
        <TabPanel value={tabValue} index={1}>
          <Typography variant="h6" gutterBottom>
            Embedded Dashboard Views
          </Typography>
          <Alert severity="info" sx={{ mb: 3 }}>
            These are embedded views from Grafana dashboards. Click the
            fullscreen icon to open in Grafana.
          </Alert>

          <Grid container spacing={3}>
            {dashboards.map((dashboard) => (
              <Grid item xs={12} lg={6} key={dashboard.id}>
                <Paper sx={{ p: 2 }}>
                  <Box
                    sx={{
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "center",
                      mb: 2,
                    }}
                  >
                    <Typography variant="h6">{dashboard.title}</Typography>
                    <Tooltip title="Open in Grafana">
                      <IconButton
                        size="small"
                        href={dashboard.url}
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        <OpenInNew />
                      </IconButton>
                    </Tooltip>
                  </Box>

                  {/* Embedded iframe - in real implementation, this would be the actual Grafana embed */}
                  <Box
                    sx={{
                      height: 300,
                      border: 1,
                      borderColor: "divider",
                      borderRadius: 1,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      backgroundColor: "grey.50",
                    }}
                  >
                    {loading ? (
                      <CircularProgress />
                    ) : (
                      <Box sx={{ textAlign: "center" }}>
                        <Dashboard
                          sx={{ fontSize: 48, color: "text.secondary", mb: 1 }}
                        />
                        <Typography variant="body2" color="text.secondary">
                          Embedded Grafana Dashboard
                        </Typography>
                        <Typography variant="caption" color="text.secondary">
                          {dashboard.title}
                        </Typography>
                        <Box sx={{ mt: 2 }}>
                          <Button
                            variant="outlined"
                            size="small"
                            href={dashboard.url}
                            target="_blank"
                            rel="noopener noreferrer"
                          >
                            Open in Grafana
                          </Button>
                        </Box>
                      </Box>
                    )}
                  </Box>
                </Paper>
              </Grid>
            ))}
          </Grid>
        </TabPanel>

        {/* Quick Metrics Tab */}
        <TabPanel value={tabValue} index={2}>
          <Typography variant="h6" gutterBottom>
            Quick System Metrics
          </Typography>

          <Grid container spacing={3}>
            {dashboards.map((dashboard) => (
              <Grid item xs={12} sm={6} md={4} key={dashboard.id}>
                <Card>
                  <CardContent>
                    <Typography variant="h6" gutterBottom>
                      {dashboard.title}
                    </Typography>

                    <Box sx={{ mb: 2 }}>
                      <Typography variant="body2" color="text.secondary">
                        Total Series
                      </Typography>
                      <Typography variant="h4" color="primary">
                        {formatNumber(dashboard.metrics.totalSeries)}
                      </Typography>
                    </Box>

                    <Box sx={{ mb: 2 }}>
                      <Typography variant="body2" color="text.secondary">
                        Active Crawlers
                      </Typography>
                      <Typography variant="h5">
                        {dashboard.metrics.activeCrawlers}
                      </Typography>
                    </Box>

                    <Box sx={{ mb: 2 }}>
                      <Typography variant="body2" color="text.secondary">
                        Data Points
                      </Typography>
                      <Typography variant="h6">
                        {formatNumber(dashboard.metrics.dataPoints)}
                      </Typography>
                    </Box>

                    <Box
                      sx={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                      }}
                    >
                      <Chip
                        icon={getStatusIcon(dashboard.status)}
                        label={dashboard.status.toUpperCase()}
                        color={getStatusColor(dashboard.status)}
                        size="small"
                      />
                      <Button
                        size="small"
                        href={dashboard.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        startIcon={<TrendingUp />}
                      >
                        View Details
                      </Button>
                    </Box>
                  </CardContent>
                </Card>
              </Grid>
            ))}
          </Grid>
        </TabPanel>
      </Paper>
    </Box>
  );
}
