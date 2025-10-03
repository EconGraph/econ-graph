import React, { useState, useEffect } from 'react';
import {
  Card,
  CardContent,
  CardHeader,
  Tabs,
  Tab,
  Box,
  Typography,
  Alert,
  AlertTitle,
  Badge,
  Button,
  LinearProgress,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  Chip,
} from '@mui/material';
import {
  TrendingUp,
  Building2,
  FileText,
  BarChart3,
  PieChart,
  Activity,
  RefreshCw,
  Download,
  Share2,
  Eye,
  Clock,
} from 'lucide-react';
// import { FinancialStatement, FinancialRatio, Company } from '@/types/financial';
import { useQuery } from '@tanstack/react-query';
import { executeGraphQL } from '@/utils/graphql';
import { GET_FINANCIAL_DASHBOARD } from '@/test-utils/mocks/graphql/financial-queries';

interface FinancialDashboardProps {
  companyId: string;
  userType?: 'beginner' | 'intermediate' | 'advanced' | 'expert';
  showEducationalContent?: boolean;
  showCollaborativeFeatures?: boolean;
}

const useFinancialDashboardData = (companyId: string) => {
  return useQuery({
    queryKey: ['financial-dashboard', companyId],
    queryFn: async () => {
      const result = await executeGraphQL({
        query: GET_FINANCIAL_DASHBOARD,
        variables: { companyId },
      });
      if (result.errors) {
        throw new Error(result.errors[0].message);
      }
      return result.data.company;
    },
    suspense: false,
    staleTime: 5 * 60 * 1000, // 5 minutes
    cacheTime: 10 * 60 * 1000, // 10 minutes
  });
};

export const FinancialDashboard: React.FC<FinancialDashboardProps> = ({
  companyId,
  userType: _userType = 'intermediate',
  showEducationalContent: _showEducationalContent = true,
  showCollaborativeFeatures: _showCollaborativeFeatures = true,
}) => {
  const { data, isLoading, isError, error } = useFinancialDashboardData(companyId);
  const [activeTab, setActiveTab] = useState(0);
  const [selectedStatement, setSelectedStatement] = useState<string | null>(null);
  const [timeRange, _setTimeRange] = useState<'1Y' | '3Y' | '5Y' | '10Y'>('3Y');

  useEffect(() => {
    if (data?.financialStatements && data.financialStatements.length > 0) {
      setSelectedStatement(data.financialStatements[0].id);
    }
  }, [data]);

  const handleTabChange = (event: React.SyntheticEvent, newValue: number) => {
    setActiveTab(newValue);
  };

  if (isLoading) {
    return (
      <Box
        sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100vh' }}
      >
        <LinearProgress sx={{ width: '80%' }} />
      </Box>
    );
  }

  if (isError) {
    return (
      <Alert severity='error'>
        <AlertTitle>Error</AlertTitle>
        Failed to load financial dashboard:{' '}
        {error instanceof Error ? error.message : 'Unknown error'}
      </Alert>
    );
  }

  if (!data) {
    return (
      <Alert severity='warning'>
        <AlertTitle>No Data</AlertTitle>
        No financial data available for this company.
      </Alert>
    );
  }

  // The data IS the company object, not wrapped in a company field
  const company = data;
  const { financialStatements: statements, financialRatios: ratios, trends: _trends = [] } = data;

  if (!company) {
    return (
      <Alert severity='warning'>
        <AlertTitle>Error</AlertTitle>
        Company information not available
      </Alert>
    );
  }

  return (
    <Box sx={{ p: 3 }} className='space-y-6'>
      {/* Header Section */}
      <Card>
        <CardContent>
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
              <Box sx={{ p: 1, bgcolor: 'primary.light', borderRadius: 1 }}>
                <Building2 size={24} color='primary.main' />
              </Box>
              <Box>
                <Typography variant='h5' component='div'>
                  {company.name}
                </Typography>
                <Box
                  sx={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 1,
                    fontSize: '0.875rem',
                    color: 'text.secondary',
                  }}
                >
                  <Typography variant='body2'>Ticker:</Typography>
                  <Badge color='primary'>{company.ticker}</Badge>
                  <Typography variant='body2'>Industry:</Typography>
                  <Typography variant='body2'>{company.industry}</Typography>
                </Box>
              </Box>
            </Box>
            <Box sx={{ display: 'flex', gap: 1 }}>
              <Button variant='outlined' startIcon={<RefreshCw />}>
                Refresh
              </Button>
              <Button variant='contained' startIcon={<Download />}>
                Download
              </Button>
              {_showCollaborativeFeatures && (
                <Button variant='outlined' startIcon={<Share2 />}>
                  Share
                </Button>
              )}
            </Box>
          </Box>
        </CardContent>
      </Card>

      {/* Main Content Tabs */}
      <Tabs value={activeTab} onChange={handleTabChange} aria-label='Financial Dashboard Tabs'>
        <Tab label='Overview' value={0} icon={<Eye size={20} />} iconPosition='start' />
        <Tab label='Statements' value={1} icon={<FileText size={20} />} iconPosition='start' />
        <Tab label='Ratios' value={2} icon={<BarChart3 size={20} />} iconPosition='start' />
        <Tab label='Trends' value={3} icon={<TrendingUp size={20} />} iconPosition='start' />
        <Tab label='Comparison' value={4} icon={<PieChart size={20} />} iconPosition='start' />
        <Tab label='Analysis' value={5} icon={<Activity size={20} />} iconPosition='start' />
      </Tabs>

      {/* Tab Content */}
      {activeTab === 0 && (
        <Box className='space-y-4'>
          <div className='grid grid-cols-1 lg:grid-cols-2 gap-6'>
            <Card>
              <CardHeader title='Recent Filings' />
              <CardContent>
                <div className='space-y-3'>
                  {statements && statements.length > 0 ? (
                    statements.slice(0, 5).map((statement: any) => (
                      <div
                        key={statement.id}
                        className='flex items-center justify-between p-2 hover:bg-gray-50 rounded-md cursor-pointer'
                        onClick={() => setSelectedStatement(statement.id)}
                      >
                        <Typography variant='body2'>
                          {statement.type} - {statement.period}
                        </Typography>
                        <Clock size={16} className='text-muted-foreground' />
                      </div>
                    ))
                  ) : (
                    <Alert severity='info'>No financial statements available.</Alert>
                  )}
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader title='Key Financial Metrics' />
              <CardContent>
                <div className='space-y-4'>
                  {ratios.length > 0 ? (
                    ratios.slice(0, 6).map((ratio: any) => (
                      <div key={ratio.id} className='flex items-center justify-between'>
                        <Typography variant='body2'>{ratio.ratioDisplayName}</Typography>
                        <Typography variant='body2' color='text.secondary'>
                          {ratio.value.toFixed(2)}
                        </Typography>
                      </div>
                    ))
                  ) : (
                    <Alert severity='info'>No key financial metrics available.</Alert>
                  )}
                </div>
              </CardContent>
            </Card>
          </div>
        </Box>
      )}

      {activeTab === 1 && (
        <Box className='space-y-4'>
          {selectedStatement ? (
            <Card>
              <CardHeader title='Financial Statement Details' />
              <CardContent>
                <Typography variant='h6'>Statement ID: {selectedStatement}</Typography>
                <Typography variant='body1'>
                  This would show detailed financial statement data.
                </Typography>
              </CardContent>
            </Card>
          ) : (
            <Alert severity='info'>
              <AlertTitle>No Statement Selected</AlertTitle>
              Please select a financial statement to view details.
            </Alert>
          )}
        </Box>
      )}

      {activeTab === 2 && (
        <Card>
          <CardHeader title='Financial Ratios Analysis' />
          <CardContent>
            <TableContainer component={Paper}>
              <Table>
                <TableHead>
                  <TableRow>
                    <TableCell>Ratio Name</TableCell>
                    <TableCell>Value</TableCell>
                    <TableCell>Category</TableCell>
                    <TableCell>Benchmark</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {ratios.map((ratio: any) => (
                    <TableRow key={ratio.id}>
                      <TableCell>{ratio.ratioDisplayName}</TableCell>
                      <TableCell>{ratio.value.toFixed(3)}</TableCell>
                      <TableCell>
                        <Chip label={ratio.category} size='small' />
                      </TableCell>
                      <TableCell>
                        {ratio.benchmarkPercentile
                          ? `${ratio.benchmarkPercentile}th percentile`
                          : 'N/A'}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          </CardContent>
        </Card>
      )}

      {activeTab === 3 && (
        <Card>
          <CardHeader title='Trend Analysis' />
          <CardContent>
            <Typography variant='body1'>
              Trend analysis charts would be displayed here for the selected time range: {timeRange}
            </Typography>
          </CardContent>
        </Card>
      )}

      {activeTab === 4 && (
        <Card>
          <CardHeader title='Benchmark Comparison' />
          <CardContent>
            <Typography variant='body1'>
              Benchmark comparison charts would be displayed here.
            </Typography>
          </CardContent>
        </Card>
      )}

      {activeTab === 5 && (
        <Card>
          <CardHeader title='Advanced Analysis' />
          <CardContent>
            <Typography variant='body1'>
              This section would contain advanced financial analysis tools and visualizations.
            </Typography>
          </CardContent>
        </Card>
      )}
    </Box>
  );
};
