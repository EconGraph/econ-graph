/**
 * Crawler Logs and Monitoring
 *
 * Features:
 * - Real-time log streaming
 * - Error monitoring and alerting
 * - Performance metrics and analytics
 * - Log filtering and search
 * - Export capabilities
 */

import React, {
  useState,
  useEffect,
  useCallback,
  useMemo,
} from "react";
import {
  Box,
  Card,
  CardContent,
  Typography,
  Grid,
  TextField,
  Button,
  Chip,
  Alert,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  IconButton,
  Tooltip,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Switch,
  FormControlLabel,
  LinearProgress,
  Badge,
} from "@mui/material";
import {
  Refresh,
  Download,
  Search,
  FilterList,
  Clear,
  Error,
  Info,
  CheckCircle,
  Schedule,
  Speed,
  Memory,
  Storage,
  NetworkCheck,
} from "@mui/icons-material";
import { format } from "date-fns";
import { useCrawlerLogs, useLogSearch } from "../hooks/useCrawlerLogs";

// Types for log entries and monitoring data
interface LogEntry {
  id: string;
  timestamp: string;
  level: "debug" | "info" | "warn" | "error" | "fatal";
  source: string;
  message: string;
  details?: any;
  duration?: number;
  status?: "success" | "failed" | "pending";
}

interface PerformanceMetrics {
  cpu_usage: number;
  memory_usage: number;
  disk_io: number;
  network_io: number;
  active_connections: number;
  queue_depth: number;
  processing_rate: number;
  error_rate: number;
}

interface CrawlerLogsProps {
  // Props can be added as needed
}

// Memoized component for expensive date formatting operations
const FormattedTimestamp = React.memo(
  ({ timestamp }: { timestamp: string }) => {
    const formattedTime = useMemo(
      () => format(new Date(timestamp), "HH:mm:ss.SSS"),
      [timestamp],
    );
    return <span>{formattedTime}</span>;
  },
);

const CrawlerLogs: React.FC<CrawlerLogsProps> = () => {
  const [filteredLogs, setFilteredLogs] = useState<LogEntry[]>([]);
  const [performanceMetrics, setPerformanceMetrics] =
    useState<PerformanceMetrics>({
      cpu_usage: 0,
      memory_usage: 0,
      disk_io: 0,
      network_io: 0,
      active_connections: 0,
      queue_depth: 0,
      processing_rate: 0,
      error_rate: 0,
    });

  const [searchTerm, setSearchTerm] = useState("");
  const [levelFilter, setLevelFilter] = useState<string>("all");
  const [sourceFilter, setSourceFilter] = useState<string>("all");
  const [autoRefresh, setAutoRefresh] = useState(true);

  // Use real GraphQL data
  const { logs, loading, error, refresh } = useCrawlerLogs(
    50,
    0,
    levelFilter === "all" ? undefined : levelFilter,
    sourceFilter === "all" ? undefined : sourceFilter,
    autoRefresh,
    5000,
  );

  // Use log search for search functionality
  const {
    searchResults,
    loading: searchLoading,
    error: searchError,
    refresh: refreshSearch,
  } = useLogSearch(
    searchTerm,
    { level: levelFilter, source: sourceFilter },
    searchTerm.length > 0,
  );

  // Filter logs based on search term and filters
  useEffect(() => {
    let logsToFilter = searchTerm.length > 0 ? searchResults : logs;

    const filtered = logsToFilter.filter((log: any) => {
      const matchesLevel = levelFilter === "all" || log.level === levelFilter;
      const matchesSource =
        sourceFilter === "all" || log.source === sourceFilter;
      const matchesSearch =
        searchTerm.length === 0 ||
        log.message.toLowerCase().includes(searchTerm.toLowerCase()) ||
        log.source.toLowerCase().includes(searchTerm.toLowerCase());

      return matchesLevel && matchesSource && matchesSearch;
    });

    setFilteredLogs(filtered);
  }, [logs, searchResults, levelFilter, sourceFilter, searchTerm]);

  // Mock performance metrics (TODO: Replace with real GraphQL call)
  useEffect(() => {
    const updateMetrics = () => {
      setPerformanceMetrics({
        cpu_usage: Math.random() * 100,
        memory_usage: Math.random() * 100,
        disk_io: Math.random() * 1000,
        network_io: Math.random() * 500,
        active_connections: Math.floor(Math.random() * 50) + 10,
        queue_depth: Math.floor(Math.random() * 100),
        processing_rate: Math.random() * 10,
        error_rate: Math.random() * 5,
      });
    };

    updateMetrics();
    const interval = setInterval(updateMetrics, 2000);
    return () => clearInterval(interval);
  }, []);

  const handleRefresh = useCallback(() => {
    refresh();
    if (searchTerm.length > 0) {
      refreshSearch();
    }
  }, [refresh, refreshSearch, searchTerm]);

  const handleExportLogs = useCallback(() => {
    // TODO: Implement actual export
    console.log("Exporting logs...");
  }, []);

  const handleClearLogs = useCallback(() => {
    setFilteredLogs([]);
  }, []);

  const getLevelColor = useCallback((level: string) => {
    switch (level) {
      case "error":
      case "fatal":
        return "error";
      case "warn":
        return "warning";
      case "info":
        return "info";
      case "debug":
        return "default";
      default:
        return "default";
    }
  }, []);

  const getStatusIcon = useCallback((status?: string) => {
    switch (status) {
      case "success":
        return <CheckCircle color="success" />;
      case "failed":
        return <Error color="error" />;
      case "pending":
        return <Schedule color="warning" />;
      default:
        return <Info />;
    }
  }, []);

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
        <Typography variant="h4" component="h1" data-testid="logs-title">
          📊 Crawler Logs & Monitoring
        </Typography>
        <Box sx={{ display: "flex", gap: 1 }}>
          <Button
            variant="outlined"
            startIcon={<Refresh />}
            onClick={handleRefresh}
            disabled={loading || searchLoading}
          >
            {loading || searchLoading ? "Refreshing..." : "Refresh"}
          </Button>
          <Button
            variant="outlined"
            startIcon={<Download />}
            onClick={handleExportLogs}
          >
            Export
          </Button>
          <Button
            variant="outlined"
            startIcon={<Clear />}
            onClick={handleClearLogs}
            color="error"
          >
            Clear
          </Button>
        </Box>
      </Box>

      {error && (
        <Alert severity="error" sx={{ mb: 3 }}>
          {error.message || error.toString()}
        </Alert>
      )}

      {searchError && (
        <Alert severity="warning" sx={{ mb: 3 }}>
          Search error: {searchError.message || searchError.toString()}
        </Alert>
      )}

      {/* Performance Metrics */}
      <Grid container spacing={3} sx={{ mb: 3 }}>
        <Grid item xs={12} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
                <Speed sx={{ mr: 1, color: "primary.main" }} />
                <Typography variant="h6">CPU Usage</Typography>
              </Box>
              <Typography variant="h4" color="primary">
                {performanceMetrics.cpu_usage.toFixed(1)}%
              </Typography>
              <LinearProgress
                variant="determinate"
                value={performanceMetrics.cpu_usage}
                sx={{ mt: 1 }}
              />
            </CardContent>
          </Card>
        </Grid>

        <Grid item xs={12} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
                <Memory sx={{ mr: 1, color: "secondary.main" }} />
                <Typography variant="h6">Memory Usage</Typography>
              </Box>
              <Typography variant="h4" color="secondary">
                {performanceMetrics.memory_usage.toFixed(1)}%
              </Typography>
              <LinearProgress
                variant="determinate"
                value={performanceMetrics.memory_usage}
                color="secondary"
                sx={{ mt: 1 }}
              />
            </CardContent>
          </Card>
        </Grid>

        <Grid item xs={12} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
                <Storage sx={{ mr: 1, color: "warning.main" }} />
                <Typography variant="h6">Queue Depth</Typography>
              </Box>
              <Typography variant="h4" color="warning.main">
                {performanceMetrics.queue_depth}
              </Typography>
              <Typography variant="body2" color="text.secondary">
                items pending
              </Typography>
            </CardContent>
          </Card>
        </Grid>

        <Grid item xs={12} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
                <NetworkCheck sx={{ mr: 1, color: "error.main" }} />
                <Typography variant="h6">Error Rate</Typography>
              </Box>
              <Typography variant="h4" color="error.main">
                {performanceMetrics.error_rate.toFixed(1)}%
              </Typography>
              <Typography variant="body2" color="text.secondary">
                last hour
              </Typography>
            </CardContent>
          </Card>
        </Grid>
      </Grid>

      {/* Logs Interface */}
      <Card>
        <CardContent>
          <Box
            sx={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              mb: 2,
            }}
          >
            <Typography variant="h6" component="h2">
              Logs & Events
            </Typography>
            <Box sx={{ display: "flex", gap: 2, alignItems: "center" }}>
              <FormControlLabel
                control={
                  <Switch
                    checked={autoRefresh}
                    onChange={(e) => setAutoRefresh(e.target.checked)}
                  />
                }
                label="Auto-refresh"
              />
              <Badge badgeContent={filteredLogs.length} color="primary">
                <Typography variant="body2">Total Logs</Typography>
              </Badge>
            </Box>
          </Box>

          {/* Filters */}
          <Grid container spacing={2} sx={{ mb: 2 }}>
            <Grid item xs={12} md={4}>
              <TextField
                label="Search logs..."
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
                fullWidth
                size="small"
                InputProps={{
                  startAdornment: (
                    <Search sx={{ mr: 1, color: "action.active" }} />
                  ),
                }}
              />
            </Grid>
            <Grid item xs={12} md={3}>
              <FormControl fullWidth size="small">
                <InputLabel>Level</InputLabel>
                <Select
                  value={levelFilter}
                  onChange={(e) => setLevelFilter(e.target.value)}
                >
                  <MenuItem value="all">All Levels</MenuItem>
                  <MenuItem value="debug">Debug</MenuItem>
                  <MenuItem value="info">Info</MenuItem>
                  <MenuItem value="warn">Warning</MenuItem>
                  <MenuItem value="error">Error</MenuItem>
                  <MenuItem value="fatal">Fatal</MenuItem>
                </Select>
              </FormControl>
            </Grid>
            <Grid item xs={12} md={3}>
              <FormControl fullWidth size="small">
                <InputLabel>Source</InputLabel>
                <Select
                  value={sourceFilter}
                  onChange={(e) => setSourceFilter(e.target.value)}
                >
                  <MenuItem value="all">All Sources</MenuItem>
                  <MenuItem value="FRED">FRED</MenuItem>
                  <MenuItem value="BLS">BLS</MenuItem>
                  <MenuItem value="Census">Census</MenuItem>
                  <MenuItem value="WorldBank">World Bank</MenuItem>
                </Select>
              </FormControl>
            </Grid>
            <Grid item xs={12} md={2}>
              <Button
                variant="outlined"
                startIcon={<FilterList />}
                onClick={() => {
                  setSearchTerm("");
                  setLevelFilter("all");
                  setSourceFilter("all");
                }}
                fullWidth
              >
                Clear Filters
              </Button>
            </Grid>
          </Grid>

          {/* Logs Table */}
          <TableContainer
            component={Paper}
            variant="outlined"
            sx={{ maxHeight: 600 }}
          >
            <Table stickyHeader>
              <TableHead>
                <TableRow>
                  <TableCell>Timestamp</TableCell>
                  <TableCell>Level</TableCell>
                  <TableCell>Source</TableCell>
                  <TableCell>Message</TableCell>
                  <TableCell>Status</TableCell>
                  <TableCell>Duration</TableCell>
                  <TableCell>Actions</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {filteredLogs.map((log) => (
                  <TableRow key={log.id} hover>
                    <TableCell>
                      <Typography variant="body2">
                        <FormattedTimestamp timestamp={log.timestamp} />
                      </Typography>
                    </TableCell>
                    <TableCell>
                      <Chip
                        label={log.level.toUpperCase()}
                        color={getLevelColor(log.level)}
                        size="small"
                      />
                    </TableCell>
                    <TableCell>
                      <Typography variant="body2" fontWeight="medium">
                        {log.source}
                      </Typography>
                    </TableCell>
                    <TableCell>
                      <Typography variant="body2">{log.message}</Typography>
                      {log.details && (
                        <Typography variant="caption" color="text.secondary">
                          {JSON.stringify(log.details)}
                        </Typography>
                      )}
                    </TableCell>
                    <TableCell>
                      {log.status && (
                        <Box sx={{ display: "flex", alignItems: "center" }}>
                          {getStatusIcon(log.status)}
                          <Typography variant="body2" sx={{ ml: 1 }}>
                            {log.status}
                          </Typography>
                        </Box>
                      )}
                    </TableCell>
                    <TableCell>
                      {log.duration && (
                        <Typography variant="body2">
                          {log.duration.toFixed(2)}s
                        </Typography>
                      )}
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
              </TableBody>
            </Table>
          </TableContainer>

          {filteredLogs.length === 0 && (
            <Box sx={{ textAlign: "center", py: 4 }}>
              <Typography variant="body1" color="text.secondary">
                No logs found matching your criteria
              </Typography>
            </Box>
          )}
        </CardContent>
      </Card>
    </Box>
  );
};

export default CrawlerLogs;
