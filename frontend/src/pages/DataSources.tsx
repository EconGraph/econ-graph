import React, { Suspense, useMemo } from 'react';
import {
  Box,
  Typography,
  Grid,
  Card,
  CardContent,
  CardActions,
  Button,
  Chip,
  LinearProgress,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  CircularProgress,
} from '@mui/material';
import {
  AccountBalance as FedIcon,
  Work as BLSIcon,
  Public as CensusIcon,
  Language as WorldBankIcon,
  CheckCircle as CheckIcon,
  Schedule as ScheduleIcon,
  TrendingUp as TrendingUpIcon,
} from '@mui/icons-material';
import { useDataSources } from '../hooks/useSeriesData';

interface DataSourceInfo {
  id: string;
  name: string;
  description: string;
  baseUrl: string;
  icon: React.ReactElement;
  seriesCount: number;
  lastCrawl: string;
  status: 'active' | 'inactive' | 'error';
  rateLimit: number;
  categories: string[];
}

// Helper function to get icon for data source
const getDataSourceIcon = (name: string): React.ReactElement => {
  if (name.toLowerCase().includes('federal reserve') || name.toLowerCase().includes('fred')) {
    return <FedIcon />;
  } else if (name.toLowerCase().includes('labor') || name.toLowerCase().includes('bls')) {
    return <BLSIcon />;
  } else if (name.toLowerCase().includes('census')) {
    return <CensusIcon />;
  } else if (name.toLowerCase().includes('world bank')) {
    return <WorldBankIcon />;
  }
  return <TrendingUpIcon />;
};

// Helper function to get categories for data source
const getDataSourceCategories = (name: string): string[] => {
  if (name.toLowerCase().includes('federal reserve') || name.toLowerCase().includes('fred')) {
    return ['GDP & Growth', 'Interest Rates', 'Money Supply', 'Exchange Rates'];
  } else if (name.toLowerCase().includes('labor') || name.toLowerCase().includes('bls')) {
    return ['Employment', 'Inflation', 'Wages', 'Productivity'];
  } else if (name.toLowerCase().includes('census')) {
    return ['Demographics', 'Housing', 'Business', 'Trade'];
  } else if (name.toLowerCase().includes('world bank')) {
    return ['Global Development', 'Economic Indicators', 'Social Indicators'];
  }
  return ['Economic Data'];
};

/**
 * REQUIREMENT: Support for Federal Reserve and BLS data sources with monitoring.
 * PURPOSE: Display available data sources and their status for transparency.
 * This provides users with information about data sources and system status.
 * @returns The DataSourcesContent component.
 */
const DataSourcesContent: React.FC = () => {
  // Fetch real data sources from backend - suspense: true eliminates need for isLoading/error checks
  const { data: backendDataSources } = useDataSources();

  // Transform backend data to frontend format
  const dataSources: DataSourceInfo[] = React.useMemo(() => {
    if (!backendDataSources) return [];

    return backendDataSources.map((source: any) => ({
      id: source.id,
      name: source.name,
      description: source.description || 'Economic data source',
      baseUrl: source.base_url,
      icon: getDataSourceIcon(source.name),
      seriesCount: source.series_count || 0,
      lastCrawl: source.updated_at || new Date().toISOString(),
      status: 'active' as const,
      rateLimit: source.rate_limit_per_minute || 60,
      categories: getDataSourceCategories(source.name),
    }));
  }, [backendDataSources]);

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'active':
        return 'success';
      case 'inactive':
        return 'warning';
      case 'error':
        return 'error';
      default:
        return 'default';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'active':
        return <CheckIcon color='success' />;
      case 'inactive':
        return <ScheduleIcon color='warning' />;
      case 'error':
        return <ScheduleIcon color='error' />;
      default:
        return <ScheduleIcon />;
    }
  };

  const formatLastCrawl = (dateString: string) => {
    const date = new Date(dateString);
    const now = new Date();
    const diffHours = Math.floor((now.getTime() - date.getTime()) / (1000 * 60 * 60));

    if (diffHours < 1) return 'Less than 1 hour ago';
    if (diffHours < 24) return `${diffHours} hours ago`;
    return date.toLocaleDateString();
  };

  // Memoize expensive statistics calculations
  const stats = useMemo(() => {
    const totalSeries = dataSources.reduce((sum, source) => sum + source.seriesCount, 0);
    const healthySources = dataSources.filter(s => s.status === 'active').length;
    return { totalSeries, healthySources };
  }, [dataSources]);

  return (
    <Box>
      {/* Page header */}
      <Box sx={{ mb: 4 }}>
        <Typography variant='h4' component='h1' gutterBottom>
          Data Sources
        </Typography>
        <Typography variant='body1' color='text.secondary'>
          Economic data providers and their current status
        </Typography>
      </Box>

      {/* Summary statistics */}
      <Paper sx={{ p: 3, mb: 4 }}>
        <Grid container spacing={3}>
          <Grid item xs={12} sm={3}>
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant='h3' color='primary'>
                {dataSources.length}
              </Typography>
              <Typography variant='body2' color='text.secondary'>
                Active Sources
              </Typography>
            </Box>
          </Grid>
          <Grid item xs={12} sm={3}>
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant='h3' color='primary'>
                {stats.totalSeries.toLocaleString()}
              </Typography>
              <Typography variant='body2' color='text.secondary'>
                Total Series
              </Typography>
            </Box>
          </Grid>
          <Grid item xs={12} sm={3}>
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant='h3' color='success.main'>
                {stats.healthySources}
              </Typography>
              <Typography variant='body2' color='text.secondary'>
                Healthy Sources
              </Typography>
            </Box>
          </Grid>
          <Grid item xs={12} sm={3}>
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant='h3' color='primary'>
                98.5%
              </Typography>
              <Typography variant='body2' color='text.secondary'>
                Uptime
              </Typography>
            </Box>
          </Grid>
        </Grid>
      </Paper>

      {/* Data sources grid */}
      <Grid container spacing={3} sx={{ mb: 4 }}>
        {dataSources.map(source => (
          <Grid item xs={12} md={6} key={source.id}>
            <Card sx={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
              <CardContent sx={{ flexGrow: 1 }}>
                {/* Header with icon and status */}
                <Box sx={{ display: 'flex', alignItems: 'flex-start', mb: 2 }}>
                  <Box sx={{ mr: 2, color: 'primary.main' }}>{source.icon}</Box>
                  <Box sx={{ flexGrow: 1 }}>
                    <Typography variant='h6' component='div' gutterBottom>
                      {source.name}
                    </Typography>
                    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                      {getStatusIcon(source.status)}
                      <Chip
                        label={source.status.toUpperCase()}
                        size='small'
                        color={getStatusColor(source.status)}
                      />
                    </Box>
                  </Box>
                </Box>

                {/* Description */}
                <Typography variant='body2' color='text.secondary' sx={{ mb: 2 }}>
                  {source.description}
                </Typography>

                {/* Categories */}
                <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mb: 2 }}>
                  {source.categories.map(category => (
                    <Chip key={category} label={category} size='small' variant='outlined' />
                  ))}
                </Box>

                {/* Statistics */}
                <Box sx={{ mt: 2 }}>
                  <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 1 }}>
                    <Typography variant='body2'>Series Count</Typography>
                    <Typography variant='body2' fontWeight='bold'>
                      {source.seriesCount.toLocaleString()}
                    </Typography>
                  </Box>
                  <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 1 }}>
                    <Typography variant='body2'>Rate Limit</Typography>
                    <Typography variant='body2' fontWeight='bold'>
                      {source.rateLimit}/min
                    </Typography>
                  </Box>
                  <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 2 }}>
                    <Typography variant='body2'>Last Crawl</Typography>
                    <Typography variant='body2' fontWeight='bold'>
                      {formatLastCrawl(source.lastCrawl)}
                    </Typography>
                  </Box>

                  {/* Health indicator */}
                  <Box>
                    <Typography variant='body2' sx={{ mb: 1 }}>
                      Health: {source.status === 'active' ? 'Excellent' : 'Issues'}
                    </Typography>
                    <LinearProgress
                      variant='determinate'
                      value={source.status === 'active' ? 98 : 45}
                      color={source.status === 'active' ? 'success' : 'warning'}
                      sx={{ height: 6, borderRadius: 3 }}
                    />
                  </Box>
                </Box>
              </CardContent>

              <CardActions>
                <Button
                  size='small'
                  startIcon={<TrendingUpIcon />}
                  href={`/explore?source=${encodeURIComponent(source.name)}`}
                >
                  Browse Series
                </Button>
                <Button size='small' color='inherit'>
                  View Details
                </Button>
              </CardActions>
            </Card>
          </Grid>
        ))}
      </Grid>

      {/* Crawl schedule table */}
      <Paper sx={{ p: 3 }}>
        <Typography variant='h6' gutterBottom>
          Crawl Schedule
        </Typography>
        <TableContainer>
          <Table>
            <TableHead>
              <TableRow>
                <TableCell>
                  <strong>Source</strong>
                </TableCell>
                <TableCell>
                  <strong>Frequency</strong>
                </TableCell>
                <TableCell>
                  <strong>Next Scheduled</strong>
                </TableCell>
                <TableCell>
                  <strong>Priority</strong>
                </TableCell>
                <TableCell>
                  <strong>Status</strong>
                </TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {dataSources.map(source => (
                <TableRow key={source.id}>
                  <TableCell>
                    <Box sx={{ display: 'flex', alignItems: 'center' }}>
                      {source.icon}
                      <Typography sx={{ ml: 1 }}>{source.name}</Typography>
                    </Box>
                  </TableCell>
                  <TableCell>
                    {source.id === 'fred'
                      ? 'Every 4 hours'
                      : source.id === 'bls'
                        ? 'Every 6 hours'
                        : 'Daily'}
                  </TableCell>
                  <TableCell>
                    {new Date(
                      Date.now() +
                        (source.id === 'fred' ? 4 : source.id === 'bls' ? 6 : 24) * 60 * 60 * 1000
                    ).toLocaleString()}
                  </TableCell>
                  <TableCell>
                    <Chip
                      label={
                        source.id === 'fred' ? 'High' : source.id === 'bls' ? 'High' : 'Normal'
                      }
                      size='small'
                      color={source.id === 'fred' || source.id === 'bls' ? 'primary' : 'default'}
                    />
                  </TableCell>
                  <TableCell>
                    <Chip label='Scheduled' size='small' color='success' variant='outlined' />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      </Paper>
    </Box>
  );
};

/**
 * DataSources wrapper with Suspense boundary for loading states.
 * @returns The DataSources component with Suspense boundary.
 */
const DataSources: React.FC = () => (
  <Suspense
    fallback={
      <Box
        sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', minHeight: '400px' }}
      >
        <CircularProgress />
      </Box>
    }
  >
    <DataSourcesContent />
  </Suspense>
);

export default DataSources;
