/**
 * Financial Data Viewer Component
 *
 * Features:
 * - Display financial statements from SEC EDGAR filings
 * - Interactive charts and visualizations
 * - Company financial metrics and ratios
 * - Time-series analysis of financial data
 * - Export capabilities for financial reports
 */

import React, { useState, useCallback, useMemo } from 'react';
import {
  Box,
  Card,
  CardContent,
  Typography,
  Grid,
  Tabs,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  Chip,
  Button,
  IconButton,
  Tooltip,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Alert,
  CircularProgress,
} from '@mui/material';
import {
  TrendingUp,
  TrendingDown,
  Business,
  Description,
  Download,
  Share,
  Compare,
  Assessment,
  Timeline,
  BarChart,
  PieChart,
  TableChart,
} from '@mui/icons-material';
import { useFinancialData } from '../../hooks/useFinancialData';

interface FinancialDataViewerProps {
  companyId: string;
  className?: string;
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
      role='tabpanel'
      hidden={value !== index}
      id={`financial-tabpanel-${index}`}
      aria-labelledby={`financial-tab-${index}`}
      {...other}
    >
      {value === index && <Box sx={{ p: 3 }}>{children}</Box>}
    </div>
  );
}

export const FinancialDataViewer: React.FC<FinancialDataViewerProps> = ({
  companyId,
  className,
}) => {
  const [activeTab, setActiveTab] = useState(0);
  const [selectedStatements, setSelectedStatements] = useState<string[]>([]);
  const [compareDialogOpen, setCompareDialogOpen] = useState(false);
  const [exportDialogOpen, setExportDialogOpen] = useState(false);

  const { company, financialStatements, loading, error, loadCompany, loadFinancialStatements } =
    useFinancialData();

  // Load data on mount
  React.useEffect(() => {
    if (companyId) {
      loadCompany(companyId);
      loadFinancialStatements(companyId);
    }
  }, [companyId, loadCompany, loadFinancialStatements]);

  // Handle tab change
  const handleTabChange = useCallback((event: React.SyntheticEvent, newValue: number) => {
    setActiveTab(newValue);
  }, []);

  // Handle statement selection
  const handleStatementSelect = useCallback((statementId: string) => {
    setSelectedStatements(prev =>
      prev.includes(statementId) ? prev.filter(id => id !== statementId) : [...prev, statementId]
    );
  }, []);

  // Format currency values
  const formatCurrency = useCallback((value: number | null) => {
    if (value === null || value === undefined) return 'N/A';
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(value);
  }, []);

  // Format dates
  const formatDate = useCallback((dateString: string) => {
    return new Date(dateString).toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  }, []);

  // Get filing type color
  const getFilingTypeColor = useCallback((filingType: string) => {
    switch (filingType) {
      case '10-K':
        return 'primary';
      case '10-Q':
        return 'secondary';
      case '8-K':
        return 'warning';
      default:
        return 'default';
    }
  }, []);

  // Group statements by fiscal year
  const statementsByYear = useMemo(() => {
    const grouped: { [key: number]: any[] } = {};
    financialStatements.forEach(statement => {
      const year = statement.fiscal_year;
      if (!grouped[year]) {
        grouped[year] = [];
      }
      grouped[year].push(statement);
    });
    return grouped;
  }, [financialStatements]);

  const tabs = [
    { label: 'Overview', icon: <Assessment /> },
    { label: 'Income Statement', icon: <TrendingUp /> },
    { label: 'Balance Sheet', icon: <TableChart /> },
    { label: 'Cash Flow', icon: <Timeline /> },
    { label: 'Ratios', icon: <BarChart /> },
  ];

  if (loading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', p: 4 }}>
        <CircularProgress />
      </Box>
    );
  }

  if (error) {
    return (
      <Alert severity='error' sx={{ m: 2 }}>
        Error loading financial data: {error.message}
      </Alert>
    );
  }

  return (
    <Box className={className}>
      {/* Company Header */}
      {company && (
        <Card sx={{ mb: 3 }}>
          <CardContent>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 2, mb: 2 }}>
              <Business color='primary' fontSize='large' />
              <Box>
                <Typography variant='h4'>{company.name}</Typography>
                {company.ticker && (
                  <Chip label={company.ticker} color='primary' variant='outlined' sx={{ mt: 1 }} />
                )}
              </Box>
            </Box>

            <Grid container spacing={2}>
              <Grid item xs={12} sm={6} md={3}>
                <Typography variant='body2' color='text.secondary'>
                  CIK
                </Typography>
                <Typography variant='body1'>{company.cik}</Typography>
              </Grid>
              <Grid item xs={12} sm={6} md={3}>
                <Typography variant='body2' color='text.secondary'>
                  Industry
                </Typography>
                <Typography variant='body1'>{company.industry || 'N/A'}</Typography>
              </Grid>
              <Grid item xs={12} sm={6} md={3}>
                <Typography variant='body2' color='text.secondary'>
                  Sector
                </Typography>
                <Typography variant='body1'>{company.sector || 'N/A'}</Typography>
              </Grid>
              <Grid item xs={12} sm={6} md={3}>
                <Typography variant='body2' color='text.secondary'>
                  Fiscal Year End
                </Typography>
                <Typography variant='body1'>{company.fiscal_year_end || 'N/A'}</Typography>
              </Grid>
            </Grid>
          </CardContent>
        </Card>
      )}

      {/* Financial Data Tabs */}
      <Card>
        <Box sx={{ borderBottom: 1, borderColor: 'divider' }}>
          <Tabs
            value={activeTab}
            onChange={handleTabChange}
            aria-label='financial data tabs'
            variant='scrollable'
            scrollButtons='auto'
          >
            {tabs.map((tab, index) => (
              <Tab
                key={tab.label}
                label={tab.label}
                icon={tab.icon}
                iconPosition='start'
                id={`financial-tab-${index}`}
                aria-controls={`financial-tabpanel-${index}`}
              />
            ))}
          </Tabs>
        </Box>

        {/* Overview Tab */}
        <TabPanel value={activeTab} index={0}>
          <Grid container spacing={3}>
            <Grid item xs={12} md={8}>
              <Typography variant='h6' gutterBottom>
                Recent Filings
              </Typography>
              <TableContainer component={Paper}>
                <Table>
                  <TableHead>
                    <TableRow>
                      <TableCell>Filing Type</TableCell>
                      <TableCell>Filing Date</TableCell>
                      <TableCell>Period End</TableCell>
                      <TableCell>Status</TableCell>
                      <TableCell>Actions</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {financialStatements.slice(0, 10).map(statement => (
                      <TableRow key={statement.id}>
                        <TableCell>
                          <Chip
                            label={statement.filing_type}
                            color={getFilingTypeColor(statement.filing_type)}
                            size='small'
                          />
                        </TableCell>
                        <TableCell>{formatDate(statement.filing_date)}</TableCell>
                        <TableCell>{formatDate(statement.period_end_date)}</TableCell>
                        <TableCell>
                          <Chip
                            label={statement.xbrl_processing_status}
                            color={
                              statement.xbrl_processing_status === 'completed'
                                ? 'success'
                                : statement.xbrl_processing_status === 'failed'
                                  ? 'error'
                                  : 'warning'
                            }
                            size='small'
                          />
                        </TableCell>
                        <TableCell>
                          <Tooltip title='View Details'>
                            <IconButton size='small'>
                              <Description />
                            </IconButton>
                          </Tooltip>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
            </Grid>

            <Grid item xs={12} md={4}>
              <Typography variant='h6' gutterBottom>
                Quick Actions
              </Typography>
              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <Button
                  variant='outlined'
                  startIcon={<Compare />}
                  onClick={() => setCompareDialogOpen(true)}
                >
                  Compare Companies
                </Button>
                <Button
                  variant='outlined'
                  startIcon={<Download />}
                  onClick={() => setExportDialogOpen(true)}
                >
                  Export Data
                </Button>
                <Button variant='outlined' startIcon={<Share />}>
                  Share Report
                </Button>
              </Box>
            </Grid>
          </Grid>
        </TabPanel>

        {/* Income Statement Tab */}
        <TabPanel value={activeTab} index={1}>
          <Typography variant='h6' gutterBottom>
            Income Statement Analysis
          </Typography>
          <Alert severity='info' sx={{ mb: 2 }}>
            Income statement data will be displayed here once XBRL processing is complete.
          </Alert>
        </TabPanel>

        {/* Balance Sheet Tab */}
        <TabPanel value={activeTab} index={2}>
          <Typography variant='h6' gutterBottom>
            Balance Sheet Analysis
          </Typography>
          <Alert severity='info' sx={{ mb: 2 }}>
            Balance sheet data will be displayed here once XBRL processing is complete.
          </Alert>
        </TabPanel>

        {/* Cash Flow Tab */}
        <TabPanel value={activeTab} index={3}>
          <Typography variant='h6' gutterBottom>
            Cash Flow Analysis
          </Typography>
          <Alert severity='info' sx={{ mb: 2 }}>
            Cash flow data will be displayed here once XBRL processing is complete.
          </Alert>
        </TabPanel>

        {/* Ratios Tab */}
        <TabPanel value={activeTab} index={4}>
          <Typography variant='h6' gutterBottom>
            Financial Ratios
          </Typography>
          <Alert severity='info' sx={{ mb: 2 }}>
            Financial ratios will be calculated and displayed here once XBRL processing is complete.
          </Alert>
        </TabPanel>
      </Card>

      {/* Compare Dialog */}
      <Dialog
        open={compareDialogOpen}
        onClose={() => setCompareDialogOpen(false)}
        maxWidth='md'
        fullWidth
      >
        <DialogTitle>Compare Companies</DialogTitle>
        <DialogContent>
          <Typography variant='body1' gutterBottom>
            Select companies to compare financial data.
          </Typography>
          <TextField
            fullWidth
            label='Search Companies'
            placeholder='Enter company name, ticker, or CIK'
            sx={{ mt: 2 }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setCompareDialogOpen(false)}>Cancel</Button>
          <Button variant='contained'>Compare</Button>
        </DialogActions>
      </Dialog>

      {/* Export Dialog */}
      <Dialog
        open={exportDialogOpen}
        onClose={() => setExportDialogOpen(false)}
        maxWidth='sm'
        fullWidth
      >
        <DialogTitle>Export Financial Data</DialogTitle>
        <DialogContent>
          <FormControl fullWidth sx={{ mb: 2 }}>
            <InputLabel>Export Format</InputLabel>
            <Select value='csv' label='Export Format'>
              <MenuItem value='csv'>CSV</MenuItem>
              <MenuItem value='excel'>Excel</MenuItem>
              <MenuItem value='pdf'>PDF Report</MenuItem>
            </Select>
          </FormControl>
          <FormControl fullWidth>
            <InputLabel>Date Range</InputLabel>
            <Select value='all' label='Date Range'>
              <MenuItem value='all'>All Data</MenuItem>
              <MenuItem value='year'>Last Year</MenuItem>
              <MenuItem value='quarter'>Last Quarter</MenuItem>
            </Select>
          </FormControl>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setExportDialogOpen(false)}>Cancel</Button>
          <Button variant='contained'>Export</Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
};

export default FinancialDataViewer;
