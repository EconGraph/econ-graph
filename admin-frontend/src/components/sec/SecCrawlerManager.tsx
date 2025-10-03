/**
 * SEC Crawler Manager Component
 *
 * Features:
 * - Company search and selection
 * - Crawl configuration (form types, date ranges, filters)
 * - Crawl execution and monitoring
 * - RSS feed import functionality
 * - Crawl history and status tracking
 */

import React, { useState, useCallback } from "react";
import {
  Box,
  Card,
  CardContent,
  Typography,
  Grid,
  Button,
  TextField,
  Switch,
  FormControlLabel,
  Chip,
  Alert,
  LinearProgress,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Stepper,
  Step,
  StepLabel,
  StepContent,
} from "@mui/material";
import {
  PlayArrow,
  Download,
  Business,
  CheckCircle,
  Error,
  Warning,
  History,
  Settings,
} from "@mui/icons-material";
import CompanySearch from "./CompanySearch";
import { useSecCrawler } from "../../hooks/useSecCrawler";
import { Company, SecCrawlResult } from "../../types";

interface SecCrawlerManagerProps {
  company?: Company;
  onCrawlComplete?: (result: SecCrawlResult) => void;
  onCrawlError?: (error: Error) => void;
  className?: string;
}

export const SecCrawlerManager: React.FC<SecCrawlerManagerProps> = ({
  className,
}) => {
  const [selectedCompany, setSelectedCompany] = useState<any>(null);
  const [crawlConfig, setCrawlConfig] = useState({
    formTypes: "10-K,10-Q",
    startDate: "",
    endDate: "",
    excludeAmended: false,
    excludeRestated: false,
    maxFileSize: 52428800, // 50MB
  });
  const [activeStep, setActiveStep] = useState(0);
  const [crawlDialogOpen, setCrawlDialogOpen] = useState(false);
  const [rssDialogOpen, setRssDialogOpen] = useState(false);

  const { crawlCompany, importRssFeed, isCrawling, progress, status, error } =
    useSecCrawler();

  // Handle company selection
  const handleCompanySelect = useCallback((company: any) => {
    setSelectedCompany(company);
    setActiveStep(1);
  }, []);

  // Handle crawl configuration change
  const handleConfigChange = useCallback((field: string, value: any) => {
    setCrawlConfig((prev) => ({
      ...prev,
      [field]: value,
    }));
  }, []);

  // Handle crawl execution
  const handleCrawl = useCallback(async () => {
    if (!selectedCompany) return;

    try {
      await crawlCompany({
        cik: selectedCompany.cik,
        form_types: crawlConfig.formTypes,
        start_date: crawlConfig.startDate || undefined,
        end_date: crawlConfig.endDate || undefined,
        exclude_amended: crawlConfig.excludeAmended,
        exclude_restated: crawlConfig.excludeRestated,
        max_file_size: crawlConfig.maxFileSize,
      });
      setCrawlDialogOpen(false);
    } catch (err) {
      console.error("Crawl error:", err);
    }
  }, [selectedCompany, crawlConfig, crawlCompany]);

  // Handle RSS import
  const handleRssImport = useCallback(async () => {
    try {
      await importRssFeed({
        rss_url: undefined, // Use default SEC RSS
        max_filings: 100,
        form_types: "10-K,10-Q,8-K",
      });
      setRssDialogOpen(false);
    } catch (err) {
      console.error("RSS import error:", err);
    }
  }, [importRssFeed]);

  const steps = [
    {
      label: "Select Company",
      description: "Search and select a company to crawl",
      content: (
        <Box>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            Search for companies by name, ticker symbol, or CIK. The search uses
            PostgreSQL fulltext indices for fast, fuzzy matching.
          </Typography>
          <CompanySearch
            onCompanySelect={handleCompanySelect}
            showCrawlButton={false}
            maxResults={20}
          />
        </Box>
      ),
    },
    {
      label: "Configure Crawl",
      description: "Set crawl parameters and filters",
      content: (
        <Box>
          <Grid container spacing={3}>
            <Grid item xs={12} md={6}>
              <TextField
                fullWidth
                label="Form Types"
                value={crawlConfig.formTypes}
                onChange={(e) =>
                  handleConfigChange("formTypes", e.target.value)
                }
                helperText="Comma-separated list (e.g., 10-K,10-Q,8-K)"
                placeholder="10-K,10-Q"
              />
            </Grid>
            <Grid item xs={12} md={3}>
              <TextField
                fullWidth
                label="Start Date"
                type="date"
                value={crawlConfig.startDate}
                onChange={(e) =>
                  handleConfigChange("startDate", e.target.value)
                }
                InputLabelProps={{ shrink: true }}
              />
            </Grid>
            <Grid item xs={12} md={3}>
              <TextField
                fullWidth
                label="End Date"
                type="date"
                value={crawlConfig.endDate}
                onChange={(e) => handleConfigChange("endDate", e.target.value)}
                InputLabelProps={{ shrink: true }}
              />
            </Grid>
            <Grid item xs={12} md={6}>
              <TextField
                fullWidth
                label="Max File Size (bytes)"
                type="number"
                value={crawlConfig.maxFileSize}
                onChange={(e) =>
                  handleConfigChange("maxFileSize", parseInt(e.target.value))
                }
                helperText="Maximum file size to download"
              />
            </Grid>
            <Grid item xs={12} md={6}>
              <Box sx={{ display: "flex", flexDirection: "column", gap: 1 }}>
                <FormControlLabel
                  control={
                    <Switch
                      checked={crawlConfig.excludeAmended}
                      onChange={(e) =>
                        handleConfigChange("excludeAmended", e.target.checked)
                      }
                    />
                  }
                  label="Exclude Amended Filings"
                />
                <FormControlLabel
                  control={
                    <Switch
                      checked={crawlConfig.excludeRestated}
                      onChange={(e) =>
                        handleConfigChange("excludeRestated", e.target.checked)
                      }
                    />
                  }
                  label="Exclude Restated Filings"
                />
              </Box>
            </Grid>
          </Grid>
        </Box>
      ),
    },
    {
      label: "Execute Crawl",
      description: "Review and start the crawl operation",
      content: (
        <Box>
          {selectedCompany && (
            <Card sx={{ mb: 2 }}>
              <CardContent>
                <Typography variant="h6" gutterBottom>
                  Selected Company
                </Typography>
                <Box
                  sx={{ display: "flex", alignItems: "center", gap: 1, mb: 1 }}
                >
                  <Business color="primary" />
                  <Typography variant="subtitle1">
                    {selectedCompany.name}
                  </Typography>
                  {selectedCompany.ticker && (
                    <Chip label={selectedCompany.ticker} size="small" />
                  )}
                </Box>
                <Typography variant="body2" color="text.secondary">
                  CIK: {selectedCompany.cik}
                </Typography>
              </CardContent>
            </Card>
          )}

          <Grid container spacing={2}>
            <Grid item xs={12} sm={6}>
              <Button
                variant="contained"
                startIcon={<PlayArrow />}
                onClick={() => setCrawlDialogOpen(true)}
                disabled={!selectedCompany || isCrawling}
                fullWidth
              >
                Start Crawl
              </Button>
            </Grid>
            <Grid item xs={12} sm={6}>
              <Button
                variant="outlined"
                startIcon={<Download />}
                onClick={() => setRssDialogOpen(true)}
                disabled={isCrawling}
                fullWidth
              >
                Import RSS Feed
              </Button>
            </Grid>
          </Grid>
        </Box>
      ),
    },
  ];

  return (
    <Box className={className}>
      <Typography variant="h4" gutterBottom>
        SEC Crawler Management
      </Typography>
      <Typography variant="body1" color="text.secondary" sx={{ mb: 3 }}>
        Search for companies and crawl their SEC EDGAR filings. Use fulltext
        search with fuzzy matching to find companies by name, ticker, or CIK.
      </Typography>

      {/* Error Display */}
      {error && (
        <Alert severity="error" sx={{ mb: 3 }}>
          {error.message}
        </Alert>
      )}

      {/* Loading Indicator */}
      {isCrawling && (
        <Box sx={{ mb: 3 }}>
          <LinearProgress variant="determinate" value={progress} />
          <Typography variant="body2" sx={{ mt: 1 }}>
            Processing crawl operation... {progress}%
          </Typography>
        </Box>
      )}

      {/* Main Content */}
      <Grid container spacing={3}>
        {/* Crawl Configuration */}
        <Grid item xs={12} md={8}>
          <Card>
            <CardContent>
              <Typography variant="h6" gutterBottom>
                Crawl Configuration
              </Typography>

              <Stepper activeStep={activeStep} orientation="vertical">
                {steps.map((step, index) => (
                  <Step key={step.label}>
                    <StepLabel>{step.label}</StepLabel>
                    <StepContent>
                      <Typography
                        variant="body2"
                        color="text.secondary"
                        sx={{ mb: 2 }}
                      >
                        {step.description}
                      </Typography>
                      {step.content}
                      <Box sx={{ mt: 2 }}>
                        <Button
                          variant="contained"
                          onClick={() => setActiveStep(index + 1)}
                          disabled={index === 0 && !selectedCompany}
                          sx={{ mr: 1 }}
                        >
                          {index === steps.length - 1 ? "Finish" : "Continue"}
                        </Button>
                        {index > 0 && (
                          <Button onClick={() => setActiveStep(index - 1)}>
                            Back
                          </Button>
                        )}
                      </Box>
                    </StepContent>
                  </Step>
                ))}
              </Stepper>
            </CardContent>
          </Card>
        </Grid>

        {/* Quick Actions */}
        <Grid item xs={12} md={4}>
          <Card>
            <CardContent>
              <Typography variant="h6" gutterBottom>
                Quick Actions
              </Typography>

              <Box sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
                <Button
                  variant="outlined"
                  startIcon={<Download />}
                  onClick={() => setRssDialogOpen(true)}
                  disabled={isCrawling}
                >
                  Import RSS Feed
                </Button>

                <Button
                  variant="outlined"
                  startIcon={<History />}
                  disabled={isCrawling}
                >
                  View Crawl History
                </Button>

                <Button
                  variant="outlined"
                  startIcon={<Settings />}
                  disabled={isCrawling}
                >
                  Crawler Settings
                </Button>
              </Box>
            </CardContent>
          </Card>

          {/* Status Display */}
          {status !== "idle" && (
            <Card sx={{ mt: 2 }}>
              <CardContent>
                <Typography variant="h6" gutterBottom>
                  Crawl Status
                </Typography>
                <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
                  {status === "completed" ? (
                    <CheckCircle color="success" />
                  ) : status === "error" ? (
                    <Error color="error" />
                  ) : (
                    <Warning color="warning" />
                  )}
                  <Typography variant="body2">
                    Status: {status} {isCrawling && `(${progress}%)`}
                  </Typography>
                </Box>
              </CardContent>
            </Card>
          )}
        </Grid>
      </Grid>

      {/* Crawl Confirmation Dialog */}
      <Dialog
        open={crawlDialogOpen}
        onClose={() => setCrawlDialogOpen(false)}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>Confirm Crawl Operation</DialogTitle>
        <DialogContent>
          <Typography variant="body1" gutterBottom>
            Are you sure you want to start crawling SEC filings for{" "}
            <strong>{selectedCompany?.name}</strong>?
          </Typography>
          <Typography variant="body2" color="text.secondary">
            This operation may take several minutes depending on the number of
            filings.
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setCrawlDialogOpen(false)}>Cancel</Button>
          <Button
            variant="contained"
            onClick={handleCrawl}
            disabled={isCrawling}
          >
            Start Crawl
          </Button>
        </DialogActions>
      </Dialog>

      {/* RSS Import Dialog */}
      <Dialog
        open={rssDialogOpen}
        onClose={() => setRssDialogOpen(false)}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>Import SEC EDGAR RSS Feed</DialogTitle>
        <DialogContent>
          <Typography variant="body1" gutterBottom>
            Import recent SEC EDGAR filings from the official RSS feed.
          </Typography>
          <Typography variant="body2" color="text.secondary">
            This will discover new companies and filing information.
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setRssDialogOpen(false)}>Cancel</Button>
          <Button
            variant="contained"
            onClick={handleRssImport}
            disabled={isCrawling}
          >
            Import RSS
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
};

export default SecCrawlerManager;
