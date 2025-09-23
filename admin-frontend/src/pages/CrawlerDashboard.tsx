/**
 * Crawler Dashboard - Main administration interface for crawler management
 *
 * Features:
 * - Real-time crawler status monitoring
 * - Queue statistics and management
 * - Manual crawl triggers
 * - Performance metrics
 * - Error monitoring and logs
 */

import React, { useState, useEffect } from "react";
import {
  Box,
  Card,
  CardContent,
  Typography,
  Grid,
  Chip,
  Button,
  LinearProgress,
  Alert,
  Paper,
  Table,
  Snackbar,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  IconButton,
  Tooltip,
  CircularProgress,
  Divider,
} from "@mui/material";
import {
  PlayArrow,
  Stop,
  Refresh,
  Warning,
  CheckCircle,
  Error,
  Info,
  History,
  Settings,
  Analytics,
} from "@mui/icons-material";
import { format } from "date-fns";
import { useCrawlerData } from "../hooks/useCrawlerData";

// Types for crawler data
interface CrawlerStatus {
  is_running: boolean;
  active_workers: number;
  last_crawl: string | null;
  next_scheduled_crawl: string | null;
}

interface ActivityItem {
  id: string;
  timestamp: string;
  source: string;
  series: string;
  status: string;
  duration: number;
}

interface QueueStatistics {
  total_items: number;
  pending_items: number;
  processing_items: number;
  completed_items: number;
  failed_items: number;
  retrying_items: number;
  oldest_pending: string | null;
  average_processing_time: number | null;
}

interface CrawlerDashboardProps {
  // Props can be added as needed for configuration
}

const CrawlerDashboard: React.FC<CrawlerDashboardProps> = () => {
  const [snackbar, setSnackbar] = useState<{
    open: boolean;
    message: string;
    severity: "success" | "error" | "info";
  }>({
    open: false,
    message: "",
    severity: "info",
  });

  // Use real GraphQL data
  const { status, queueStats, control, refreshAll, loading, error } =
    useCrawlerData(true);

  // Show snackbar notification
  const showNotification = (
    message: string,
    severity: "success" | "error" | "info" = "info",
  ) => {
    setSnackbar({ open: true, message, severity });
  };

  // Handle refresh
  const handleRefresh = async () => {
    try {
      await refreshAll();
      showNotification("Data refreshed successfully", "success");
    } catch (err) {
      showNotification("Failed to refresh data", "error");
    }
  };

  // Handle trigger crawl
  const handleTriggerCrawl = async () => {
    try {
      await control.actions.triggerCrawl({
        sources: ["FRED"],
        series_ids: ["GDP"],
        priority: 1,
      });
      showNotification("Crawl triggered successfully", "success");
    } catch (err) {
      showNotification("Failed to trigger crawl", "error");
    }
  };

  // Handle stop crawler
  const handleStopCrawler = async () => {
    try {
      await control.actions.stopCrawler();
      showNotification("Crawler stopped successfully", "success");
    } catch (err) {
      showNotification("Failed to stop crawler", "error");
    }
  };

  const getStatusColor = (status: boolean) => {
    return status ? "success" : "error";
  };

  const getStatusIcon = (status: boolean) => {
    return status ? <CheckCircle /> : <Error />;
  };

  // Mock recent activity data (this would come from a separate query)
  const recentActivity: ActivityItem[] = [
    {
      id: "1",
      timestamp: new Date(Date.now() - 2 * 60 * 1000).toISOString(),
      source: "FRED",
      series: "GDP",
      status: "completed",
      duration: 2.3,
    },
    {
      id: "2",
      timestamp: new Date(Date.now() - 5 * 60 * 1000).toISOString(),
      source: "BLS",
      series: "UNEMPLOYMENT",
      status: "failed",
      duration: 0.1,
    },
    {
      id: "3",
      timestamp: new Date(Date.now() - 8 * 60 * 1000).toISOString(),
      source: "Census",
      series: "POPULATION",
      status: "completed",
      duration: 1.8,
    },
  ];

  if (loading) {
    return (
      <Box
        display="flex"
        justifyContent="center"
        alignItems="center"
        minHeight="400px"
      >
        <CircularProgress size={60} />
        <Typography variant="h6" sx={{ ml: 2 }}>
          Loading crawler data...
        </Typography>
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
        <Typography variant="h4" component="h1" data-testid="dashboard-title">
          🕷️ Crawler Administration
        </Typography>
        <Box sx={{ display: "flex", gap: 1 }}>
          <Button
            variant="outlined"
            startIcon={<Refresh />}
            onClick={handleRefresh}
            disabled={loading}
            data-testid="refresh-button"
            aria-label="Refresh crawler data"
          >
            {loading ? "Refreshing..." : "Refresh"}
          </Button>
          <Button
            variant="contained"
            startIcon={<PlayArrow />}
            onClick={handleTriggerCrawl}
            color="primary"
            data-testid="trigger-crawl-button"
            aria-label="Trigger manual crawl"
          >
            Trigger Crawl
          </Button>
          <Button
            variant="contained"
            startIcon={<Stop />}
            onClick={handleStopCrawler}
            color="error"
            data-testid="stop-crawler-button"
            aria-label="Stop crawler"
          >
            Stop Crawler
          </Button>
        </Box>
      </Box>

      {error && (
        <Alert severity="error" sx={{ mb: 3 }}>
          {error?.message || "An error occurred"}
        </Alert>
      )}

      <Grid container spacing={3}>
        {/* Crawler Status Card */}
        <Grid item xs={12} md={6}>
          <Card data-testid="crawler-status-card">
            <CardContent>
              <Box sx={{ display: "flex", alignItems: "center", mb: 2 }}>
                <Typography
                  variant="h6"
                  component="h2"
                  data-testid="crawler-status-title"
                >
                  Crawler Status
                </Typography>
                <Chip
                  icon={getStatusIcon(status.status?.is_running || false)}
                  label={status.status?.is_running ? "Running" : "Stopped"}
                  color={getStatusColor(status.status?.is_running || false)}
                  sx={{ ml: "auto" }}
                />
              </Box>

              <Grid container spacing={2}>
                <Grid item xs={6}>
                  <Typography variant="body2" color="text.secondary">
                    Active Workers
                  </Typography>
                  <Typography variant="h4">
                    {status.status?.active_workers || 0}
                  </Typography>
                </Grid>
                <Grid item xs={6}>
                  <Typography variant="body2" color="text.secondary">
                    Last Crawl
                  </Typography>
                  <Typography variant="body1">
                    {status.status?.last_crawl
                      ? format(
                          new Date(status.status.last_crawl),
                          "MMM dd, HH:mm",
                        )
                      : "Never"}
                  </Typography>
                </Grid>
                <Grid item xs={12}>
                  <Typography variant="body2" color="text.secondary">
                    Next Scheduled Crawl
                  </Typography>
                  <Typography variant="body1">
                    {status.status?.next_scheduled_crawl
                      ? format(
                          new Date(status.status.next_scheduled_crawl),
                          "MMM dd, HH:mm",
                        )
                      : "Not scheduled"}
                  </Typography>
                </Grid>
              </Grid>
            </CardContent>
          </Card>
        </Grid>

        {/* Queue Statistics Card */}
        <Grid item xs={12} md={6}>
          <Card data-testid="queue-statistics-card">
            <CardContent>
              <Typography
                variant="h6"
                component="h2"
                sx={{ mb: 2 }}
                data-testid="queue-statistics-title"
              >
                Queue Statistics
              </Typography>

              <Grid container spacing={2}>
                <Grid item xs={6}>
                  <Typography variant="body2" color="text.secondary">
                    Total Items
                  </Typography>
                  <Typography variant="h4">
                    {queueStats.statistics?.total_items || 0}
                  </Typography>
                </Grid>
                <Grid item xs={6}>
                  <Typography variant="body2" color="text.secondary">
                    Pending
                  </Typography>
                  <Typography variant="h4" color="warning.main">
                    {queueStats.statistics?.pending_items || 0}
                  </Typography>
                </Grid>
                <Grid item xs={6}>
                  <Typography variant="body2" color="text.secondary">
                    Processing
                  </Typography>
                  <Typography variant="h4" color="info.main">
                    {queueStats.statistics?.processing_items || 0}
                  </Typography>
                </Grid>
                <Grid item xs={6}>
                  <Typography variant="body2" color="text.secondary">
                    Completed
                  </Typography>
                  <Typography variant="h4" color="success.main">
                    {queueStats.statistics?.completed_items || 0}
                  </Typography>
                </Grid>
                <Grid item xs={6}>
                  <Typography variant="body2" color="text.secondary">
                    Failed
                  </Typography>
                  <Typography variant="h4" color="error.main">
                    {queueStats.statistics?.failed_items || 0}
                  </Typography>
                </Grid>
                <Grid item xs={6}>
                  <Typography variant="body2" color="text.secondary">
                    Retrying
                  </Typography>
                  <Typography variant="h4" color="warning.main">
                    {queueStats.statistics?.retrying_items || 0}
                  </Typography>
                </Grid>
              </Grid>

              {queueStats.statistics?.average_processing_time && (
                <Box sx={{ mt: 2 }}>
                  <Typography variant="body2" color="text.secondary">
                    Average Processing Time
                  </Typography>
                  <Typography variant="body1">
                    {queueStats.statistics.average_processing_time.toFixed(1)}s
                  </Typography>
                </Box>
              )}
            </CardContent>
          </Card>
        </Grid>

        {/* Queue Progress */}
        <Grid item xs={12}>
          <Card>
            <CardContent>
              <Typography variant="h6" component="h2" sx={{ mb: 2 }}>
                Queue Progress
              </Typography>

              {queueStats && (
                <Box>
                  <Box
                    sx={{
                      display: "flex",
                      justifyContent: "space-between",
                      mb: 1,
                    }}
                  >
                    <Typography variant="body2">
                      Processing: {queueStats.statistics?.processing_items || 0}{" "}
                      / {queueStats.statistics?.total_items || 0}
                    </Typography>
                    <Typography variant="body2">
                      {queueStats.statistics?.total_items
                        ? (
                            (queueStats.statistics.completed_items /
                              queueStats.statistics.total_items) *
                            100
                          ).toFixed(1)
                        : 0}
                      %
                    </Typography>
                  </Box>
                  <LinearProgress
                    variant="determinate"
                    value={
                      queueStats.statistics?.total_items
                        ? (queueStats.statistics.completed_items /
                            queueStats.statistics.total_items) *
                          100
                        : 0
                    }
                    sx={{ height: 8, borderRadius: 4 }}
                  />
                </Box>
              )}
            </CardContent>
          </Card>
        </Grid>

        {/* Recent Activity */}
        <Grid item xs={12}>
          <Card>
            <CardContent>
              <Typography variant="h6" component="h2" sx={{ mb: 2 }}>
                Recent Activity
              </Typography>

              <TableContainer component={Paper} variant="outlined">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>Time</TableCell>
                      <TableCell>Source</TableCell>
                      <TableCell>Series</TableCell>
                      <TableCell>Status</TableCell>
                      <TableCell>Duration</TableCell>
                      <TableCell>Actions</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {/* Mock data for recent activity */}
                    {[
                      {
                        time: new Date(Date.now() - 5 * 60 * 1000),
                        source: "FRED",
                        series: "GDP",
                        status: "completed",
                        duration: "2.3s",
                      },
                      {
                        time: new Date(Date.now() - 10 * 60 * 1000),
                        source: "BLS",
                        series: "UNRATE",
                        status: "failed",
                        duration: "0.8s",
                      },
                      {
                        time: new Date(Date.now() - 15 * 60 * 1000),
                        source: "Census",
                        series: "POP",
                        status: "completed",
                        duration: "1.5s",
                      },
                    ].map((activity, index) => (
                      <TableRow key={index}>
                        <TableCell>
                          {format(activity.time, "HH:mm:ss")}
                        </TableCell>
                        <TableCell>{activity.source}</TableCell>
                        <TableCell>{activity.series}</TableCell>
                        <TableCell>
                          <Chip
                            label={activity.status}
                            color={
                              activity.status === "completed"
                                ? "success"
                                : "error"
                            }
                            size="small"
                          />
                        </TableCell>
                        <TableCell>{activity.duration}</TableCell>
                        <TableCell>
                          <Tooltip title="View Details">
                            <IconButton size="small">
                              <Info />
                            </IconButton>
                          </Tooltip>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
            </CardContent>
          </Card>
        </Grid>
      </Grid>

      {/* Snackbar for notifications */}
      <Snackbar
        open={snackbar.open}
        autoHideDuration={6000}
        onClose={() => setSnackbar({ ...snackbar, open: false })}
        anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
      >
        <Alert
          onClose={() => setSnackbar({ ...snackbar, open: false })}
          severity={snackbar.severity}
          sx={{ width: "100%" }}
        >
          {snackbar.message}
        </Alert>
      </Snackbar>
    </Box>
  );
};

export default CrawlerDashboard;
