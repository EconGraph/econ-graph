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

import React, { useState, useCallback, useMemo } from "react";
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
} from "@mui/material";
import {
  PlayArrow,
  Stop,
  Refresh,
  CheckCircle,
  Error,
  Info,
} from "@mui/icons-material";
import { format } from "date-fns";
import { useCrawlerData } from "../hooks/useCrawlerData";

// Types for crawler data

interface CrawlerDashboardProps {
  // Props can be added as needed for configuration
}

// Memoized component for expensive date formatting operations
const FormattedDate = React.memo(
  ({
    dateString,
    formatString,
  }: {
    dateString: string;
    formatString: string;
  }) => {
    const formattedDate = useMemo(
      () => format(new Date(dateString), formatString),
      [dateString, formatString],
    );
    return <span>{formattedDate}</span>;
  },
);

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
  const { status, queueStats, logs, control, refreshAll, loading, error } =
    useCrawlerData(true); // This hook now uses React Query internally

  // Show snackbar notification
  const showNotification = useCallback(
    (message: string, severity: "success" | "error" | "info" = "info") => {
      setSnackbar({ open: true, message, severity });
    },
    [],
  );

  // Handle refresh
  const handleRefresh = useCallback(async () => {
    try {
      await refreshAll();
      showNotification("Data refreshed successfully", "success");
    } catch (err) {
      showNotification("Failed to refresh data", "error");
    }
  }, [refreshAll, showNotification]);

  // Handle trigger crawl
  const handleTriggerCrawl = useCallback(async () => {
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
  }, [control.actions, showNotification]);

  // Handle stop crawler
  const handleStopCrawler = useCallback(async () => {
    try {
      await control.actions.stopCrawler();
      showNotification("Crawler stopped successfully", "success");
    } catch (err) {
      showNotification("Failed to stop crawler", "error");
    }
  }, [control.actions, showNotification]);

  const getStatusColor = useCallback((status: boolean) => {
    return status ? "success" : "error";
  }, []);

  const getStatusIcon = useCallback((status: boolean) => {
    return status ? <CheckCircle /> : <Error />;
  }, []);

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
                    {status.status?.last_crawl ? (
                      <FormattedDate
                        dateString={status.status.last_crawl}
                        formatString="MMM dd, HH:mm"
                      />
                    ) : (
                      "Never"
                    )}
                  </Typography>
                </Grid>
                <Grid item xs={12}>
                  <Typography variant="body2" color="text.secondary">
                    Next Scheduled Crawl
                  </Typography>
                  <Typography variant="body1">
                    {status.status?.next_scheduled_crawl ? (
                      <FormattedDate
                        dateString={status.status.next_scheduled_crawl}
                        formatString="MMM dd, HH:mm"
                      />
                    ) : (
                      "Not scheduled"
                    )}
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
                      <TableCell>Message</TableCell>
                      <TableCell>Status</TableCell>
                      <TableCell>Duration</TableCell>
                      <TableCell>Actions</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {/* Real GraphQL log data */}
                    {logs.logs?.map((log: any, index: number) => (
                      <TableRow key={log.id || index}>
                        <TableCell>
                          <FormattedDate
                            dateString={log.timestamp}
                            formatString="HH:mm:ss"
                          />
                        </TableCell>
                        <TableCell>{log.source}</TableCell>
                        <TableCell>{log.message}</TableCell>
                        <TableCell>
                          <Chip
                            label={log.status}
                            color={
                              log.status === "completed"
                                ? "success"
                                : log.status === "failed"
                                  ? "error"
                                  : "default"
                            }
                            size="small"
                          />
                        </TableCell>
                        <TableCell>
                          {log.duration_ms
                            ? `${(log.duration_ms / 1000).toFixed(1)}s`
                            : "-"}
                        </TableCell>
                        <TableCell>
                          <Tooltip title="View Details">
                            <IconButton size="small">
                              <Info />
                            </IconButton>
                          </Tooltip>
                        </TableCell>
                      </TableRow>
                    ))}
                    {(!logs.logs || logs.logs.length === 0) && (
                      <TableRow>
                        <TableCell colSpan={6} align="center">
                          <Typography variant="body2" color="text.secondary">
                            No recent activity
                          </Typography>
                        </TableCell>
                      </TableRow>
                    )}
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
