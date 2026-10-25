import React, { useMemo } from 'react';
import {
  Box,
  Typography,
  Grid,
  Card,
  CardContent,
  CardActions,
  Button,
  Paper,
  CircularProgress,
  Alert,
} from '@mui/material';
import {
  AccountBalance as FedIcon,
  Work as BLSIcon,
  Public as CensusIcon,
  Language as WorldBankIcon,
  TrendingUp as TrendingUpIcon,
} from '@mui/icons-material';
import { useDataSources, type DataSource } from '../hooks/useSeriesData';

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

/**
 * REQUIREMENT: Show the data sources and how many series each one has.
 * PURPOSE: Let users see where the data comes from. Only figures the API actually reports are
 * shown (series counts, rate limits); there is no crawl-health or uptime data to show yet.
 * @returns The DataSourcesContent component.
 */
const DataSourcesContent: React.FC = () => {
  const { data: backendDataSources, isPending, error } = useDataSources();
  const dataSources: DataSource[] = useMemo(() => backendDataSources ?? [], [backendDataSources]);

  const totalSeries = useMemo(
    () => dataSources.reduce((sum, source) => sum + source.seriesCount, 0),
    [dataSources]
  );

  // A blocking spinner only while there is nothing to show yet; once sources have loaded
  // once, a later refetch error stays a nonblocking banner and the cached list stays up
  if (isPending && !backendDataSources) {
    return (
      <Box
        sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', minHeight: '400px' }}
      >
        <CircularProgress aria-label='Loading data sources' />
      </Box>
    );
  }

  if (error && !backendDataSources) {
    return (
      <Alert severity='error'>
        Could not load data sources: {error instanceof Error ? error.message : 'unknown error'}
      </Alert>
    );
  }

  return (
    <Box>
      {/* Page header */}
      <Box sx={{ mb: 4 }}>
        <Typography variant='h4' component='h1' gutterBottom>
          Data Sources
        </Typography>
        <Typography variant='body1' color='text.secondary'>
          Where EconGraph's data comes from
        </Typography>
      </Box>

      {/* A refresh failed, but the list below is still the last good data */}
      {error ? (
        <Alert severity='warning' sx={{ mb: 4 }}>
          Showing the last loaded sources; refreshing failed:{' '}
          {error instanceof Error ? error.message : 'unknown error'}
        </Alert>
      ) : null}

      {/* Summary statistics */}
      <Paper sx={{ p: 3, mb: 4 }}>
        <Grid container spacing={3}>
          <Grid item xs={6}>
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant='h3' color='primary'>
                {dataSources.length}
              </Typography>
              <Typography variant='body2' color='text.secondary'>
                Sources
              </Typography>
            </Box>
          </Grid>
          <Grid item xs={6}>
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant='h3' color='primary'>
                {totalSeries.toLocaleString()}
              </Typography>
              <Typography variant='body2' color='text.secondary'>
                Total Series
              </Typography>
            </Box>
          </Grid>
        </Grid>
      </Paper>

      {/* Data sources grid */}
      <Grid container spacing={3}>
        {dataSources.map(source => (
          <Grid item xs={12} md={6} key={source.id}>
            <Card
              component='section'
              aria-labelledby={`source-${source.id}`}
              sx={{ height: '100%', display: 'flex', flexDirection: 'column' }}
            >
              <CardContent sx={{ flexGrow: 1 }}>
                <Box sx={{ display: 'flex', alignItems: 'flex-start', mb: 2 }}>
                  <Box sx={{ mr: 2, color: 'primary.main' }}>{getDataSourceIcon(source.name)}</Box>
                  <Typography
                    id={`source-${source.id}`}
                    variant='h6'
                    component='h2'
                    sx={{ minWidth: 0 }}
                  >
                    {source.name}
                  </Typography>
                </Box>

                <Typography variant='body2' color='text.secondary' sx={{ mb: 2 }}>
                  {source.description || 'Economic data source'}
                </Typography>

                <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 1 }}>
                  <Typography variant='body2'>Series</Typography>
                  <Typography variant='body2' fontWeight='bold'>
                    {source.seriesCount.toLocaleString()}
                  </Typography>
                </Box>
                <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
                  <Typography variant='body2'>Crawl rate limit</Typography>
                  <Typography variant='body2' fontWeight='bold'>
                    {source.rateLimitPerMinute.toLocaleString()}/min
                  </Typography>
                </Box>
              </CardContent>

              <CardActions>
                <Button
                  size='small'
                  startIcon={<TrendingUpIcon />}
                  href={`/explore?source=${encodeURIComponent(source.id)}`}
                >
                  Browse Series
                </Button>
              </CardActions>
            </Card>
          </Grid>
        ))}
      </Grid>
    </Box>
  );
};

export default DataSourcesContent;
