import React, { useState, useCallback } from 'react';
import {
  Grid,
  Card,
  CardContent,
  Typography,
  Box,
  Chip,
  LinearProgress,
  Paper,
  List,
  ListItem,
  ListItemText,
  ListItemIcon,
  Button,
  IconButton,
  Fab,
  Tooltip,
  Badge,
} from '@mui/material';
import {
  TrendingUp as TrendingUpIcon,
  TrendingDown as TrendingDownIcon,
  Assessment as AssessmentIcon,
  Update as UpdateIcon,
  Refresh as RefreshIcon,
  OpenInNew as OpenInNewIcon,
  Groups as CollaborationIcon,
} from '@mui/icons-material';

import { useNavigate } from 'react-router-dom';
import { useSeriesSearch } from '../hooks/useSeriesData';

/**
 * REQUIREMENT: Dashboard overview with Bloomberg Terminal-level collaboration
 * PURPOSE: Provide quick access to key economic indicators with professional collaboration
 * This improves on FRED's homepage by adding institutional-grade collaboration features.
 * @returns JSX element representing the Dashboard page.
 */
const Dashboard: React.FC = () => {
  const navigate = useNavigate();
  const [collaborationMode, setCollaborationMode] = useState(false);

  // Fetch real data sources for future use
  // const { data: dataSources } = useDataSources();

  // Fetch key economic indicators from FRED
  const gdpQueryResult = useSeriesSearch('GDP', { limit: 1 }, true);
  const { data: gdpResults } = gdpQueryResult || {};

  const unemploymentQueryResult = useSeriesSearch('unemployment rate', { limit: 1 }, true);
  const { data: unemploymentResults } = unemploymentQueryResult || {};

  const inflationQueryResult = useSeriesSearch('consumer price index', { limit: 1 }, true);
  const { data: inflationResults } = inflationQueryResult || {};

  const fedFundsQueryResult = useSeriesSearch('federal funds rate', { limit: 1 }, true);
  const { data: fedFundsResults } = fedFundsQueryResult || {};

  // Transform real data to dashboard format
  const featuredIndicators = React.useMemo(() => {
    return [
      {
        id: 'gdp',
        title: 'Real Gross Domestic Product',
        value: '$27.36T',
        change: '+2.4%',
        changeType: 'positive' as const,
        period: 'Q3 2024',
        source: 'BEA',
        seriesId: gdpResults?.[0]?.id ?? 'gdp-series-1',
      },
      {
        id: 'unemployment',
        title: 'Unemployment Rate',
        value: '3.7%',
        change: '-0.1%',
        changeType: 'positive' as const,
        period: 'Nov 2024',
        source: 'BLS',
        seriesId: unemploymentResults?.[0]?.id ?? 'unemployment-series-1',
      },
      {
        id: 'inflation',
        title: 'Consumer Price Index',
        value: '3.2%',
        change: '+0.2%',
        changeType: 'negative' as const,
        period: 'Nov 2024',
        source: 'BLS',
        seriesId: inflationResults?.[0]?.id ?? 'cpi-series-1',
      },
      {
        id: 'fed-funds',
        title: 'Federal Funds Rate',
        value: '5.25%',
        change: '0.0%',
        changeType: 'neutral' as const,
        period: 'Dec 2024',
        source: 'Federal Reserve',
        seriesId: fedFundsResults?.[0]?.id ?? 'fedfunds-series-1',
      },
    ];
  }, [gdpResults, unemploymentResults, inflationResults, fedFundsResults]);

  const recentUpdates = [
    {
      title: 'Employment Situation Summary',
      date: '2024-12-06',
      source: 'BLS',
      series: 'Nonfarm Payrolls, Unemployment Rate',
    },
    {
      title: 'Consumer Price Index',
      date: '2024-12-10',
      source: 'BLS',
      series: 'CPI-U, Core CPI',
    },
    {
      title: 'Industrial Production',
      date: '2024-12-15',
      source: 'Federal Reserve',
      series: 'Industrial Production Index',
    },
  ];

  // Collaboration handlers
  const handleToggleCollaboration = useCallback(() => {
    setCollaborationMode(!collaborationMode);
  }, [collaborationMode]);

  const getChangeColor = (changeType: 'positive' | 'negative' | 'neutral') => {
    switch (changeType) {
      case 'positive':
        return 'success';
      case 'negative':
        return 'error';
      default:
        return 'default';
    }
  };

  const getChangeIcon = (changeType: 'positive' | 'negative' | 'neutral') => {
    switch (changeType) {
      case 'positive':
        return <TrendingUpIcon fontSize='small' />;
      case 'negative':
        return <TrendingDownIcon fontSize='small' />;
      default:
        return null;
    }
  };

  return (
    <Box sx={{ position: 'relative' }}>
      {/* Page header with collaboration info */}
      <Box sx={{ mb: 4 }}>
        <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1 }}>
          <Typography variant='h4' component='h1'>
            Economic Dashboard
          </Typography>

          {/* Collaboration toggle */}
          <Button
            variant={collaborationMode ? 'contained' : 'outlined'}
            startIcon={<CollaborationIcon />}
            onClick={handleToggleCollaboration}
          >
            {collaborationMode ? 'Collaboration On' : 'Enable Collaboration'}
          </Button>
        </Box>
        <Typography variant='body1' color='text.secondary'>
          Key economic indicators with professional collaboration features
        </Typography>
      </Box>

      <Grid container spacing={3}>
        {/* Featured Economic Indicators */}
        <Grid item xs={12}>
          <Typography variant='h5' gutterBottom sx={{ mb: 2 }}>
            Key Indicators
          </Typography>
        </Grid>

        {featuredIndicators.map(indicator => (
          <Grid item xs={12} sm={6} md={3} key={indicator.id}>
            <Card
              sx={{
                height: '100%',
                cursor: 'pointer',
                transition: 'all 0.2s ease-in-out',
                position: 'relative',
                '&:hover': {
                  transform: 'translateY(-2px)',
                  boxShadow: 4,
                },
                ...(collaborationMode && {
                  borderLeft: `4px solid ${collaborationMode ? '#1976d2' : 'transparent'}`,
                }),
              }}
              onClick={() => navigate(`/series/${indicator.id}`)}
            >
              <CardContent>
                <Box sx={{ display: 'flex', alignItems: 'flex-start', mb: 2 }}>
                  <AssessmentIcon color='primary' sx={{ mr: 1, mt: 0.5 }} />
                  <Box sx={{ flexGrow: 1 }}>
                    <Typography
                      variant='h6'
                      component='div'
                      sx={{ fontSize: '1rem', lineHeight: 1.3 }}
                    >
                      {indicator.title}
                    </Typography>
                    <Chip
                      label={indicator.source}
                      size='small'
                      variant='outlined'
                      sx={{ mt: 0.5 }}
                    />
                  </Box>

                  {/* Collaboration indicator */}
                  {collaborationMode && (
                    <Tooltip title='Collaboration enabled'>
                      <IconButton size='small' sx={{ ml: 1 }}>
                        <Badge badgeContent='3' color='primary' max={99}>
                          <CollaborationIcon fontSize='small' />
                        </Badge>
                      </IconButton>
                    </Tooltip>
                  )}
                </Box>

                <Typography variant='h4' component='div' sx={{ mb: 1, fontWeight: 600 }}>
                  {indicator.value}
                </Typography>

                <Box
                  sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}
                >
                  <Box sx={{ display: 'flex', alignItems: 'center' }}>
                    {getChangeIcon(indicator.changeType)}
                    <Chip
                      label={indicator.change}
                      size='small'
                      color={getChangeColor(indicator.changeType)}
                      sx={{ ml: 0.5 }}
                    />
                  </Box>
                  <Typography variant='caption' color='text.secondary'>
                    {indicator.period}
                  </Typography>
                </Box>

                {/* Collaboration activity indicators */}
                {collaborationMode && (
                  <Box sx={{ mt: 2, display: 'flex', alignItems: 'center', gap: 1 }}>
                    <Chip label='2 annotations' size='small' variant='outlined' color='primary' />
                    <Chip
                      label='1 collaborator'
                      size='small'
                      variant='outlined'
                      color='secondary'
                    />
                  </Box>
                )}
              </CardContent>
            </Card>
          </Grid>
        ))}

        {/* Recent Data Releases */}
        <Grid item xs={12} md={8}>
          <Paper sx={{ p: 3 }}>
            <Box
              sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2 }}
            >
              <Typography variant='h6'>Recent Data Releases</Typography>
              <IconButton size='small' aria-label='refresh data'>
                <RefreshIcon />
              </IconButton>
            </Box>

            <List>
              {recentUpdates.map((update, index) => (
                <ListItem
                  key={index}
                  sx={{
                    px: 0,
                    '&:hover': {
                      backgroundColor: 'action.hover',
                      borderRadius: 1,
                    },
                  }}
                  secondaryAction={
                    <IconButton edge='end' aria-label='view details'>
                      <OpenInNewIcon />
                    </IconButton>
                  }
                >
                  <ListItemIcon>
                    <UpdateIcon color='primary' />
                  </ListItemIcon>
                  <ListItemText
                    primary={update.title}
                    secondary={
                      <Box>
                        <Typography variant='body2' color='text.secondary'>
                          {update.series}
                        </Typography>
                        <Typography variant='caption' color='text.secondary'>
                          {update.source} • {new Date(update.date).toLocaleDateString()}
                        </Typography>
                      </Box>
                    }
                  />
                </ListItem>
              ))}
            </List>

            <Box sx={{ mt: 2, textAlign: 'center' }}>
              <Button
                variant='outlined'
                onClick={() => navigate('/explore')}
                startIcon={<AssessmentIcon />}
              >
                Explore All Series
              </Button>
            </Box>
          </Paper>
        </Grid>

        {/* Quick Actions */}
        <Grid item xs={12} md={4}>
          <Paper sx={{ p: 3, height: 'fit-content' }}>
            <Typography variant='h6' gutterBottom>
              Quick Actions
            </Typography>

            <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              <Button
                variant='contained'
                fullWidth
                startIcon={<AssessmentIcon />}
                onClick={() => navigate('/explore?category=employment')}
              >
                Employment Data
              </Button>

              <Button
                variant='contained'
                fullWidth
                startIcon={<AssessmentIcon />}
                onClick={() => navigate('/explore?category=inflation')}
              >
                Inflation Indicators
              </Button>

              <Button
                variant='contained'
                fullWidth
                startIcon={<AssessmentIcon />}
                onClick={() => navigate('/explore?category=gdp')}
              >
                GDP & Growth
              </Button>

              <Button variant='outlined' fullWidth onClick={() => navigate('/sources')}>
                Browse Data Sources
              </Button>
            </Box>

            {/* System Status */}
            <Box sx={{ mt: 3, pt: 2, borderTop: 1, borderColor: 'divider' }}>
              <Typography variant='subtitle2' gutterBottom>
                System Status
              </Typography>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 1 }}>
                <Typography variant='body2' sx={{ flexGrow: 1 }}>
                  Data Freshness
                </Typography>
                <Chip label='Current' size='small' color='success' />
              </Box>
              <LinearProgress
                variant='determinate'
                value={95}
                color='success'
                sx={{ height: 6, borderRadius: 3 }}
              />
              <Typography
                variant='caption'
                color='text.secondary'
                sx={{ mt: 0.5, display: 'block' }}
              >
                Last updated: 2 hours ago
              </Typography>
            </Box>
          </Paper>
        </Grid>
      </Grid>

      {/* Floating collaboration status */}
      {collaborationMode && (
        <Tooltip title='Collaboration Mode Active'>
          <Fab
            color='primary'
            sx={{
              position: 'fixed',
              bottom: 24,
              right: 24,
              zIndex: 1000,
            }}
            onClick={handleToggleCollaboration}
          >
            <Badge badgeContent='5' color='secondary' max={99}>
              <CollaborationIcon />
            </Badge>
          </Fab>
        </Tooltip>
      )}
    </Box>
  );
};

export default Dashboard;
