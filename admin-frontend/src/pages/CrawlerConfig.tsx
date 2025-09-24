/**
 * Crawler Configuration Management
 *
 * Features:
 * - Data source configuration
 * - Crawl scheduling and frequency
 * - Rate limiting and throttling
 * - Error handling and retry policies
 * - Data source health monitoring
 */

import React, { useState, useMemo, useCallback } from "react";
import {
  Box,
  Card,
  CardContent,
  Typography,
  Grid,
  Switch,
  FormControlLabel,
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
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Slider,
} from "@mui/material";
import {
  Settings,
  Save,
  Refresh,
  Edit,
  Add,
  CheckCircle,
  Error,
  Warning,
  Info,
  Speed,
  Security,
} from "@mui/icons-material";

// Types for crawler configuration
interface DataSource {
  id: string;
  name: string;
  enabled: boolean;
  priority: number;
  rate_limit: number;
  retry_attempts: number;
  timeout_seconds: number;
  last_success: string | null;
  last_error: string | null;
  health_status: "healthy" | "warning" | "error";
}

interface CrawlerConfigData {
  global_enabled: boolean;
  max_workers: number;
  queue_size_limit: number;
  default_timeout: number;
  default_retry_attempts: number;
  rate_limit_global: number;
  schedule_frequency: string;
  error_threshold: number;
  maintenance_mode: boolean;
}

const CrawlerConfig: React.FC = () => {
  const [config, setConfig] = useState<CrawlerConfigData>({
    global_enabled: true,
    max_workers: 5,
    queue_size_limit: 10000,
    default_timeout: 30,
    default_retry_attempts: 3,
    rate_limit_global: 10,
    schedule_frequency: "hourly",
    error_threshold: 5,
    maintenance_mode: false,
  });

  const [dataSources, setDataSources] = useState<DataSource[]>([
    {
      id: "fred",
      name: "Federal Reserve Economic Data (FRED)",
      enabled: true,
      priority: 1,
      rate_limit: 5,
      retry_attempts: 3,
      timeout_seconds: 30,
      last_success: new Date(Date.now() - 10 * 60 * 1000).toISOString(),
      last_error: null,
      health_status: "healthy",
    },
    {
      id: "bls",
      name: "Bureau of Labor Statistics (BLS)",
      enabled: true,
      priority: 2,
      rate_limit: 3,
      retry_attempts: 3,
      timeout_seconds: 45,
      last_success: new Date(Date.now() - 25 * 60 * 1000).toISOString(),
      last_error: null,
      health_status: "healthy",
    },
    {
      id: "census",
      name: "US Census Bureau",
      enabled: true,
      priority: 3,
      rate_limit: 2,
      retry_attempts: 5,
      timeout_seconds: 60,
      last_success: new Date(Date.now() - 45 * 60 * 1000).toISOString(),
      last_error: "Connection timeout",
      health_status: "warning",
    },
    {
      id: "worldbank",
      name: "World Bank",
      enabled: false,
      priority: 4,
      rate_limit: 1,
      retry_attempts: 3,
      timeout_seconds: 30,
      last_success: null,
      last_error: "API key expired",
      health_status: "error",
    },
  ]);

  // Create separate components for expensive operations to isolate re-renders
  const LastSuccessCell = ({ lastSuccess }: { lastSuccess: string | null }) => {
    const formattedDate = useMemo(() => {
      return lastSuccess ? new Date(lastSuccess).toLocaleString() : "Never";
    }, [lastSuccess]);

    return (
      <Typography variant="body2" color="text.secondary">
        {formattedDate}
      </Typography>
    );
  };

  // Memoize the data sources array to prevent unnecessary re-renders
  const memoizedDataSources = useMemo(() => dataSources, [dataSources]);

  // Optimized event handlers with useCallback to prevent unnecessary re-renders
  const handleEditSource = useCallback((source: DataSource) => {
    setEditingSource(source);
    setEditDialogOpen(true);
  }, []);

  const handleCloseDialog = useCallback(() => {
    setEditDialogOpen(false);
    setEditingSource(null);
  }, []);

  const handleSaveSource = useCallback(async (updatedSource: DataSource) => {
    setSaving(true);
    try {
      setDataSources((prev: DataSource[]) =>
        prev.map((source: DataSource) =>
          source.id === updatedSource.id ? updatedSource : source,
        ),
      );
      setEditDialogOpen(false);
      setEditingSource(null);
    } catch (error) {
      setError("Failed to save data source configuration");
    } finally {
      setSaving(false);
    }
  }, []);

  const handleTestConnection = useCallback(async (sourceId: string) => {
    try {
      // Simulate connection test
      await new Promise((resolve) => setTimeout(resolve, 1000));
      // Update health status
      setDataSources((prev: DataSource[]) =>
        prev.map((source: DataSource) =>
          source.id === sourceId
            ? { ...source, health_status: "healthy" as const }
            : source,
        ),
      );
    } catch (error) {
      setError("Connection test failed");
    }
  }, []);

  const [editingSource, setEditingSource] = useState<DataSource | null>(null);
  const [editDialogOpen, setEditDialogOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleConfigChange = useCallback(
    (field: keyof CrawlerConfigData, value: any) => {
      setConfig((prev: CrawlerConfigData) => ({
        ...prev,
        [field]: value,
      }));
    },
    [],
  );

  const handleDataSourceChange = useCallback(
    (id: string, field: keyof DataSource, value: any) => {
      setDataSources((prev: DataSource[]) =>
        prev.map((source) =>
          source.id === id ? { ...source, [field]: value } : source,
        ),
      );
    },
    [],
  );

  const handleSaveConfig = useCallback(async () => {
    try {
      setSaving(true);
      setError(null);

      // TODO: Implement actual GraphQL mutation
      // const mutation = `
      //   mutation UpdateCrawlerConfig($config: CrawlerConfigInput!) {
      //     updateCrawlerConfig(config: $config) {
      //       success
      //       message
      //     }
      //   }
      // `;

      // Configuration and data sources are being saved

      // Simulate API call
      await new Promise((resolve) => setTimeout(resolve, 1000));

      setSaving(false);
    } catch (err) {
      setError((err as Error)?.message || "Failed to save configuration");
      setSaving(false);
    }
  }, []);

  const getHealthStatusColor = useCallback((status: string) => {
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

  const getHealthStatusIcon = useCallback((status: string) => {
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
        <Typography variant="h4" component="h1" data-testid="config-title">
          ⚙️ Crawler Configuration
        </Typography>
        <Button
          variant="contained"
          startIcon={<Save />}
          onClick={handleSaveConfig}
          disabled={saving}
          data-testid="save-configuration-button"
        >
          {saving ? "Saving..." : "Save Configuration"}
        </Button>
      </Box>

      {error && (
        <Alert severity="error" sx={{ mb: 3 }} onClose={() => setError(null)}>
          {error}
        </Alert>
      )}

      <Grid container spacing={3}>
        {/* Global Configuration */}
        <Grid item xs={12} md={6}>
          <Card>
            <CardContent>
              <Typography
                variant="h6"
                component="h2"
                sx={{ mb: 2 }}
                data-testid="global-settings-title"
              >
                <Settings sx={{ mr: 1, verticalAlign: "middle" }} />
                Global Settings
              </Typography>

              <Grid container spacing={2}>
                <Grid item xs={12}>
                  <FormControlLabel
                    control={
                      <Switch
                        checked={config.global_enabled}
                        onChange={(e) =>
                          handleConfigChange("global_enabled", e.target.checked)
                        }
                      />
                    }
                    label="Enable Crawler"
                  />
                </Grid>

                <Grid item xs={12}>
                  <FormControlLabel
                    control={
                      <Switch
                        checked={config.maintenance_mode}
                        onChange={(e) =>
                          handleConfigChange(
                            "maintenance_mode",
                            e.target.checked,
                          )
                        }
                      />
                    }
                    label="Maintenance Mode"
                  />
                </Grid>

                <Grid item xs={6}>
                  <TextField
                    label="Max Workers"
                    type="number"
                    value={config.max_workers}
                    onChange={(e) =>
                      handleConfigChange(
                        "max_workers",
                        parseInt(e.target.value),
                      )
                    }
                    fullWidth
                    size="small"
                    data-testid="max-workers-input"
                    inputProps={{ "aria-label": "Maximum number of workers" }}
                  />
                </Grid>

                <Grid item xs={6}>
                  <TextField
                    label="Queue Size Limit"
                    type="number"
                    value={config.queue_size_limit}
                    onChange={(e) =>
                      handleConfigChange(
                        "queue_size_limit",
                        parseInt(e.target.value),
                      )
                    }
                    fullWidth
                    size="small"
                    data-testid="queue-size-limit-input"
                    inputProps={{ "aria-label": "Queue size limit" }}
                  />
                </Grid>

                <Grid item xs={6}>
                  <TextField
                    label="Default Timeout (seconds)"
                    type="number"
                    value={config.default_timeout}
                    onChange={(e) =>
                      handleConfigChange(
                        "default_timeout",
                        parseInt(e.target.value),
                      )
                    }
                    fullWidth
                    size="small"
                  />
                </Grid>

                <Grid item xs={6}>
                  <TextField
                    label="Default Retry Attempts"
                    type="number"
                    value={config.default_retry_attempts}
                    onChange={(e) =>
                      handleConfigChange(
                        "default_retry_attempts",
                        parseInt(e.target.value),
                      )
                    }
                    fullWidth
                    size="small"
                  />
                </Grid>

                <Grid item xs={12}>
                  <Typography
                    variant="body2"
                    color="text.secondary"
                    sx={{ mb: 1 }}
                  >
                    Global Rate Limit (requests per minute)
                  </Typography>
                  <Slider
                    value={config.rate_limit_global}
                    onChange={(_, value) =>
                      handleConfigChange("rate_limit_global", value)
                    }
                    min={1}
                    max={100}
                    step={1}
                    marks={[
                      { value: 1, label: "1" },
                      { value: 25, label: "25" },
                      { value: 50, label: "50" },
                      { value: 75, label: "75" },
                      { value: 100, label: "100" },
                    ]}
                    valueLabelDisplay="auto"
                  />
                </Grid>

                <Grid item xs={12}>
                  <FormControl fullWidth size="small">
                    <InputLabel>Schedule Frequency</InputLabel>
                    <Select
                      value={config.schedule_frequency}
                      onChange={(e) =>
                        handleConfigChange("schedule_frequency", e.target.value)
                      }
                      data-testid="schedule-frequency-select"
                      inputProps={{
                        "aria-label": "Schedule frequency for crawler",
                      }}
                      MenuProps={{ disablePortal: true }}
                    >
                      <MenuItem value="every_15_minutes">
                        Every 15 Minutes
                      </MenuItem>
                      <MenuItem value="hourly">Hourly</MenuItem>
                      <MenuItem value="every_4_hours">Every 4 Hours</MenuItem>
                      <MenuItem value="daily">Daily</MenuItem>
                      <MenuItem value="weekly">Weekly</MenuItem>
                    </Select>
                  </FormControl>
                </Grid>
              </Grid>
            </CardContent>
          </Card>
        </Grid>

        {/* Data Sources Configuration */}
        <Grid item xs={12} md={6}>
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
                <Typography
                  variant="h6"
                  component="h2"
                  data-testid="data-sources-title"
                >
                  <Speed sx={{ mr: 1, verticalAlign: "middle" }} />
                  Data Sources
                </Typography>
                <Button variant="outlined" startIcon={<Add />} size="small">
                  Add Source
                </Button>
              </Box>

              <TableContainer component={Paper} variant="outlined">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell data-testid="data-sources-table-source-header">
                        Source
                      </TableCell>
                      <TableCell>Status</TableCell>
                      <TableCell>Health</TableCell>
                      <TableCell>Actions</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {memoizedDataSources.map((source) => (
                      <TableRow key={source.id}>
                        <TableCell>
                          <Box>
                            <Typography variant="body2" fontWeight="medium">
                              {source.name}
                            </Typography>
                            <Typography
                              variant="caption"
                              color="text.secondary"
                            >
                              Priority: {source.priority} | Rate:{" "}
                              {source.rate_limit}/min
                            </Typography>
                          </Box>
                        </TableCell>
                        <TableCell>
                          <Chip
                            label={source.enabled ? "Enabled" : "Disabled"}
                            color={source.enabled ? "success" : "default"}
                            size="small"
                          />
                        </TableCell>
                        <TableCell>
                          <Chip
                            icon={getHealthStatusIcon(source.health_status)}
                            label={source.health_status}
                            color={getHealthStatusColor(source.health_status)}
                            size="small"
                          />
                        </TableCell>
                        <TableCell>
                          <Tooltip title="Edit Configuration">
                            <IconButton
                              size="small"
                              onClick={() => handleEditSource(source)}
                            >
                              <Edit />
                            </IconButton>
                          </Tooltip>
                          <Tooltip title="Test Connection">
                            <IconButton
                              size="small"
                              onClick={() => handleTestConnection(source.id)}
                            >
                              <Refresh />
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

        {/* Data Source Details */}
        <Grid item xs={12}>
          <Card>
            <CardContent>
              <Typography variant="h6" component="h2" sx={{ mb: 2 }}>
                <Security sx={{ mr: 1, verticalAlign: "middle" }} />
                Data Source Details
              </Typography>

              <TableContainer component={Paper} variant="outlined">
                <Table>
                  <TableHead>
                    <TableRow>
                      <TableCell data-testid="data-source-details-table-source-header">
                        Source
                      </TableCell>
                      <TableCell>Priority</TableCell>
                      <TableCell>Rate Limit</TableCell>
                      <TableCell>Retry Attempts</TableCell>
                      <TableCell>Timeout</TableCell>
                      <TableCell>Last Success</TableCell>
                      <TableCell>Last Error</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {memoizedDataSources.map((source) => (
                      <TableRow key={source.id}>
                        <TableCell>
                          <Box sx={{ display: "flex", alignItems: "center" }}>
                            <FormControlLabel
                              control={
                                <Switch
                                  checked={source.enabled}
                                  onChange={(e) =>
                                    handleDataSourceChange(
                                      source.id,
                                      "enabled",
                                      e.target.checked,
                                    )
                                  }
                                  size="small"
                                />
                              }
                              label=""
                            />
                            <Typography variant="body2">
                              {source.name}
                            </Typography>
                          </Box>
                        </TableCell>
                        <TableCell>
                          <TextField
                            type="number"
                            value={source.priority}
                            onChange={(e) =>
                              handleDataSourceChange(
                                source.id,
                                "priority",
                                parseInt(e.target.value),
                              )
                            }
                            size="small"
                            sx={{ width: 80 }}
                          />
                        </TableCell>
                        <TableCell>
                          <TextField
                            type="number"
                            value={source.rate_limit}
                            onChange={(e) =>
                              handleDataSourceChange(
                                source.id,
                                "rate_limit",
                                parseInt(e.target.value),
                              )
                            }
                            size="small"
                            sx={{ width: 80 }}
                          />
                        </TableCell>
                        <TableCell>
                          <TextField
                            type="number"
                            value={source.retry_attempts}
                            onChange={(e) =>
                              handleDataSourceChange(
                                source.id,
                                "retry_attempts",
                                parseInt(e.target.value),
                              )
                            }
                            size="small"
                            sx={{ width: 80 }}
                          />
                        </TableCell>
                        <TableCell>
                          <TextField
                            type="number"
                            value={source.timeout_seconds}
                            onChange={(e) =>
                              handleDataSourceChange(
                                source.id,
                                "timeout_seconds",
                                parseInt(e.target.value),
                              )
                            }
                            size="small"
                            sx={{ width: 80 }}
                          />
                        </TableCell>
                        <TableCell>
                          <LastSuccessCell lastSuccess={source.last_success} />
                        </TableCell>
                        <TableCell>
                          <Typography variant="body2" color="error">
                            {source.last_error || "None"}
                          </Typography>
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

      {/* Edit Data Source Dialog */}
      <Dialog
        open={editDialogOpen}
        onClose={handleCloseDialog}
        maxWidth="md"
        fullWidth
      >
        <DialogTitle data-testid="edit-dialog-title">
          Edit Data Source Configuration
        </DialogTitle>
        <DialogContent>
          {editingSource && (
            <Grid container spacing={2} sx={{ mt: 1 }}>
              <Grid item xs={12}>
                <TextField
                  label="Source Name"
                  value={editingSource.name}
                  onChange={(e) =>
                    setEditingSource((prev) =>
                      prev ? { ...prev, name: e.target.value } : null,
                    )
                  }
                  fullWidth
                />
              </Grid>
              <Grid item xs={6}>
                <TextField
                  label="Priority"
                  type="number"
                  value={editingSource.priority}
                  onChange={(e) =>
                    setEditingSource({
                      ...editingSource,
                      priority: parseInt(e.target.value),
                    })
                  }
                  fullWidth
                />
              </Grid>
              <Grid item xs={6}>
                <TextField
                  label="Rate Limit (requests/min)"
                  type="number"
                  value={editingSource.rate_limit}
                  onChange={(e) =>
                    setEditingSource({
                      ...editingSource,
                      rate_limit: parseInt(e.target.value),
                    })
                  }
                  fullWidth
                />
              </Grid>
              <Grid item xs={6}>
                <TextField
                  label="Retry Attempts"
                  type="number"
                  value={editingSource.retry_attempts}
                  onChange={(e) =>
                    setEditingSource({
                      ...editingSource,
                      retry_attempts: parseInt(e.target.value),
                    })
                  }
                  fullWidth
                />
              </Grid>
              <Grid item xs={6}>
                <TextField
                  label="Timeout (seconds)"
                  type="number"
                  value={editingSource.timeout_seconds}
                  onChange={(e) =>
                    setEditingSource({
                      ...editingSource,
                      timeout_seconds: parseInt(e.target.value),
                    })
                  }
                  fullWidth
                />
              </Grid>
            </Grid>
          )}
        </DialogContent>
        <DialogActions>
          <Button onClick={handleCloseDialog}>Cancel</Button>
          <Button
            variant="contained"
            onClick={() => editingSource && handleSaveSource(editingSource)}
          >
            Save
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
};

export default CrawlerConfig;
