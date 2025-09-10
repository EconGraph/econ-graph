/**
 * REQUIREMENT: Professional chart analytics page with Bloomberg Terminal-level capabilities
 * PURPOSE: Showcase advanced technical analysis, multi-series comparison, and collaboration
 * This demonstrates the professional economic analysis features of EconGraph
 */

import React, { useState, useEffect, useCallback } from 'react';
import {
  Box,
  Container,
  Typography,
  Paper,
  Grid,
  Button,
  Card,
  CardContent,
  Alert,
  // Skeleton, // Unused import
  Fab,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  List,
  ListItem,
  ListItemText,
  Checkbox,
  FormControlLabel,
} from '@mui/material';
import {
  Analytics as AnalyticsIcon,
  TrendingUp as TrendingUpIcon,
  ShowChart as ShowChartIcon,
  People as PeopleIcon,
  // Add as AddIcon, // Unused import
  Timeline as TimelineIcon,
} from '@mui/icons-material';
import { useParams, useNavigate } from 'react-router-dom';
import ProfessionalChart, { SeriesData } from '../components/charts/ProfessionalChart';
import ChartCollaboration, {
  ChartAnnotation,
  ChartComment,
} from '../components/charts/ChartCollaboration';
// Removed unused imports: useSeriesDetail, useSeriesData

// Mock data for demonstration - in real app this would come from GraphQL
const SAMPLE_SERIES: SeriesData[] = [
  {
    id: 'GDPC1',
    title: 'Real Gross Domestic Product',
    description: 'Seasonally Adjusted Annual Rate',
    data: [
      { date: '2020-01-01', value: 19254.69 },
      { date: '2020-04-01', value: 17303.38 },
      { date: '2020-07-01', value: 18596.49 },
      { date: '2020-10-01', value: 18767.8 },
      { date: '2021-01-01', value: 19055.65 },
      { date: '2021-04-01', value: 19368.31 },
      { date: '2021-07-01', value: 19478.89 },
      { date: '2021-10-01', value: 19806.93 },
      { date: '2022-01-01', value: 19427.05 },
      { date: '2022-04-01', value: 19625.95 },
      { date: '2022-07-01', value: 19742.17 },
      { date: '2022-10-01', value: 20037.72 },
      { date: '2023-01-01', value: 20237.34 },
      { date: '2023-04-01', value: 20478.69 },
      { date: '2023-07-01', value: 20680.78 },
      { date: '2023-10-01', value: 20937.01 },
    ],
    color: '#2196f3',
    unit: 'Billions of Chained 2012 Dollars',
    frequency: 'Quarterly',
  },
  {
    id: 'UNRATE',
    title: 'Unemployment Rate',
    description: 'Seasonally Adjusted',
    data: [
      { date: '2020-01-01', value: 3.5 },
      { date: '2020-04-01', value: 14.8 },
      { date: '2020-07-01', value: 10.2 },
      { date: '2020-10-01', value: 6.9 },
      { date: '2021-01-01', value: 6.3 },
      { date: '2021-04-01', value: 6.1 },
      { date: '2021-07-01', value: 5.4 },
      { date: '2021-10-01', value: 4.2 },
      { date: '2022-01-01', value: 4.0 },
      { date: '2022-04-01', value: 3.6 },
      { date: '2022-07-01', value: 3.5 },
      { date: '2022-10-01', value: 3.7 },
      { date: '2023-01-01', value: 3.4 },
      { date: '2023-04-01', value: 3.7 },
      { date: '2023-07-01', value: 3.8 },
      { date: '2023-10-01', value: 3.9 },
    ],
    color: '#f44336',
    unit: 'Percent',
    frequency: 'Monthly',
  },
  {
    id: 'CPIAUCSL',
    title: 'Consumer Price Index',
    description: 'All Urban Consumers, Seasonally Adjusted',
    data: [
      { date: '2020-01-01', value: 258.678 },
      { date: '2020-04-01', value: 258.115 },
      { date: '2020-07-01', value: 259.918 },
      { date: '2020-10-01', value: 260.388 },
      { date: '2021-01-01', value: 261.582 },
      { date: '2021-04-01', value: 267.054 },
      { date: '2021-07-01', value: 273.003 },
      { date: '2021-10-01', value: 276.589 },
      { date: '2022-01-01', value: 283.716 },
      { date: '2022-04-01', value: 289.109 },
      { date: '2022-07-01', value: 296.276 },
      { date: '2022-10-01', value: 298.012 },
      { date: '2023-01-01', value: 300.536 },
      { date: '2023-04-01', value: 303.363 },
      { date: '2023-07-01', value: 307.026 },
      { date: '2023-10-01', value: 307.671 },
    ],
    color: '#4caf50',
    unit: 'Index 1982-84=100',
    frequency: 'Monthly',
  },
];

const MOCK_COLLABORATORS = [
  { id: '1', name: 'Dr. Sarah Chen', avatar: '', isOnline: true, role: 'owner' as const },
  { id: '2', name: 'Michael Rodriguez', avatar: '', isOnline: true, role: 'editor' as const },
  { id: '3', name: 'Emma Thompson', avatar: '', isOnline: false, role: 'viewer' as const },
  { id: '4', name: 'David Kim', avatar: '', isOnline: true, role: 'editor' as const },
];

const CURRENT_USER = {
  id: 'current-user',
  name: 'Economic Analyst',
  avatar: '',
};

const ProfessionalAnalysis: React.FC = () => {
  useParams<{ id?: string }>(); // Intentionally unused - for future route parameter handling
  const navigate = useNavigate();

  const [primarySeries] = useState<SeriesData>(SAMPLE_SERIES[0]); // Setter intentionally unused
  const [secondarySeries, setSecondarySeries] = useState<SeriesData[]>([]);
  const [annotations, setAnnotations] = useState<ChartAnnotation[]>([]);
  const [collaborationOpen, setCollaborationOpen] = useState(false);
  const [seriesSelectionOpen, setSeriesSelectionOpen] = useState(false);
  const [selectedSeriesIds, setSelectedSeriesIds] = useState<string[]>([]);

  // Mock annotations for demonstration
  useEffect(() => {
    const mockAnnotations: ChartAnnotation[] = [
      {
        id: '1',
        date: '2020-03-01',
        title: 'COVID-19 Pandemic Impact',
        description: 'WHO declares COVID-19 a pandemic, triggering massive economic disruption',
        color: '#f44336',
        type: 'line',
        author: MOCK_COLLABORATORS[0],
        createdAt: '2024-01-15T10:00:00Z',
        updatedAt: '2024-01-15T10:00:00Z',
        isVisible: true,
        isPinned: true,
        tags: ['pandemic', 'crisis', 'recession'],
        comments: [
          {
            id: 'c1',
            content:
              'This marks the beginning of the steepest economic contraction since the Great Depression.',
            author: MOCK_COLLABORATORS[1],
            createdAt: '2024-01-15T10:30:00Z',
            isResolved: false,
          },
          {
            id: 'c2',
            content: 'Notice how unemployment spiked immediately while GDP collapsed in Q2 2020.',
            author: CURRENT_USER,
            createdAt: '2024-01-15T11:00:00Z',
            isResolved: false,
          },
        ],
      },
      {
        id: '2',
        date: '2021-07-01',
        title: 'Economic Recovery Peak',
        description: 'GDP growth reaches recovery peak as pandemic restrictions ease',
        color: '#4caf50',
        type: 'point',
        author: CURRENT_USER,
        createdAt: '2024-01-15T14:00:00Z',
        updatedAt: '2024-01-15T14:00:00Z',
        isVisible: true,
        isPinned: false,
        tags: ['recovery', 'growth'],
        comments: [],
      },
      {
        id: '3',
        date: '2022-01-01',
        title: 'Fed Rate Hike Cycle Begins',
        description:
          'Federal Reserve begins aggressive interest rate increases to combat inflation',
        color: '#ff9800',
        type: 'line',
        author: MOCK_COLLABORATORS[1],
        createdAt: '2024-01-15T16:00:00Z',
        updatedAt: '2024-01-15T16:00:00Z',
        isVisible: true,
        isPinned: false,
        tags: ['monetary-policy', 'inflation', 'fed'],
        comments: [
          {
            id: 'c3',
            content: 'This policy shift significantly impacted economic growth trajectory.',
            author: MOCK_COLLABORATORS[0],
            createdAt: '2024-01-15T16:30:00Z',
            isResolved: false,
          },
        ],
      },
    ];

    setAnnotations(mockAnnotations);
  }, []);

  const handleAnnotationAdd = useCallback(
    (newAnnotation: Omit<ChartAnnotation, 'id' | 'createdAt' | 'updatedAt'>) => {
      const annotation: ChartAnnotation = {
        ...newAnnotation,
        id: `annotation-${Date.now()}`,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };

      setAnnotations(prev => [...prev, annotation]);
    },
    []
  );

  const handleAnnotationUpdate = useCallback((id: string, updates: Partial<ChartAnnotation>) => {
    setAnnotations(prev =>
      prev.map(annotation =>
        annotation.id === id
          ? { ...annotation, ...updates, updatedAt: new Date().toISOString() }
          : annotation
      )
    );
  }, []);

  const handleAnnotationDelete = useCallback((id: string) => {
    setAnnotations(prev => prev.filter(annotation => annotation.id !== id));
  }, []);

  const handleCommentAdd = useCallback(
    (annotationId: string, comment: Omit<ChartComment, 'id' | 'createdAt'>) => {
      const newComment: ChartComment = {
        ...comment,
        id: `comment-${Date.now()}`,
        createdAt: new Date().toISOString(),
      };

      setAnnotations(prev =>
        prev.map(annotation =>
          annotation.id === annotationId
            ? { ...annotation, comments: [...annotation.comments, newComment] }
            : annotation
        )
      );
    },
    []
  );

  const handleSeriesAdd = useCallback(() => {
    setSeriesSelectionOpen(true);
  }, []);

  const handleSeriesSelection = useCallback(() => {
    const newSecondary = SAMPLE_SERIES.filter(
      series => selectedSeriesIds.includes(series.id) && series.id !== primarySeries.id
    );
    setSecondarySeries(newSecondary);
    setSeriesSelectionOpen(false);
    setSelectedSeriesIds([]);
  }, [selectedSeriesIds, primarySeries.id]);

  const handleSeriesToggle = useCallback((seriesId: string) => {
    setSelectedSeriesIds(prev =>
      prev.includes(seriesId) ? prev.filter(id => id !== seriesId) : [...prev, seriesId]
    );
  }, []);

  return (
    <Container maxWidth={false} sx={{ py: 3, pr: collaborationOpen ? '400px' : 0 }}>
      {/* Header */}
      <Box sx={{ mb: 3 }}>
        <Typography
          variant='h4'
          component='h1'
          sx={{ mb: 1, display: 'flex', alignItems: 'center' }}
        >
          <AnalyticsIcon sx={{ mr: 2, fontSize: 40, color: 'primary.main' }} />
          Professional Chart Analytics
        </Typography>
        <Typography variant='subtitle1' color='text.secondary'>
          Bloomberg Terminal-level economic analysis with technical indicators, multi-series
          comparison, and collaboration
        </Typography>
      </Box>

      {/* Key Metrics Cards */}
      <Grid container spacing={3} sx={{ mb: 3 }}>
        <Grid item xs={12} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 1 }}>
                <TrendingUpIcon color='primary' sx={{ mr: 1 }} />
                <Typography variant='h6'>Primary Series</Typography>
              </Box>
              <Typography variant='h5' color='primary'>
                {primarySeries.title}
              </Typography>
              <Typography variant='body2' color='text.secondary'>
                Latest: {primarySeries.data[primarySeries.data.length - 1]?.value.toLocaleString()}{' '}
                {primarySeries.unit}
              </Typography>
            </CardContent>
          </Card>
        </Grid>

        <Grid item xs={12} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 1 }}>
                <ShowChartIcon color='secondary' sx={{ mr: 1 }} />
                <Typography variant='h6'>Secondary Series</Typography>
              </Box>
              <Typography variant='h5' color='secondary'>
                {secondarySeries.length}
              </Typography>
              <Typography variant='body2' color='text.secondary'>
                Comparison series active
              </Typography>
            </CardContent>
          </Card>
        </Grid>

        <Grid item xs={12} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 1 }}>
                <TimelineIcon color='success' sx={{ mr: 1 }} />
                <Typography variant='h6'>Annotations</Typography>
              </Box>
              <Typography variant='h5' color='success.main'>
                {annotations.length}
              </Typography>
              <Typography variant='body2' color='text.secondary'>
                Chart annotations added
              </Typography>
            </CardContent>
          </Card>
        </Grid>

        <Grid item xs={12} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 1 }}>
                <PeopleIcon color='info' sx={{ mr: 1 }} />
                <Typography variant='h6'>Collaborators</Typography>
              </Box>
              <Typography variant='h5' color='info.main'>
                {MOCK_COLLABORATORS.filter(c => c.isOnline).length}
              </Typography>
              <Typography variant='body2' color='text.secondary'>
                Active collaborators
              </Typography>
            </CardContent>
          </Card>
        </Grid>
      </Grid>

      {/* Professional Chart */}
      <Paper elevation={3} sx={{ mb: 3 }}>
        <ProfessionalChart
          primarySeries={primarySeries}
          secondarySeries={secondarySeries}
          height={600}
          showTechnicalAnalysis={true}
          showEconomicEvents={true}
          allowAnnotations={true}
          onSeriesAdd={handleSeriesAdd}
        />
      </Paper>

      {/* Analysis Summary */}
      <Grid container spacing={3}>
        <Grid item xs={12} md={8}>
          <Paper sx={{ p: 3 }}>
            <Typography variant='h6' sx={{ mb: 2 }}>
              Economic Analysis Summary
            </Typography>

            <Alert severity='info' sx={{ mb: 2 }}>
              <strong>Key Insight:</strong> The data shows a clear correlation between the COVID-19
              pandemic impact and economic indicators. GDP contracted sharply in Q2 2020 while
              unemployment spiked to historic highs.
            </Alert>

            <Typography variant='body1' paragraph>
              The professional chart analysis reveals several critical economic patterns:
            </Typography>

            <Box component='ul' sx={{ pl: 2 }}>
              <Typography component='li' variant='body2' paragraph>
                <strong>Pandemic Impact (March 2020):</strong> The steepest economic contraction
                since the Great Depression, with GDP falling from $19.25T to $17.30T in a single
                quarter.
              </Typography>

              <Typography component='li' variant='body2' paragraph>
                <strong>Recovery Phase (2021):</strong> Rapid economic recovery supported by
                unprecedented fiscal and monetary stimulus, with GDP returning to pre-pandemic
                levels by Q1 2021.
              </Typography>

              <Typography component='li' variant='body2' paragraph>
                <strong>Inflation Response (2022):</strong> Federal Reserve's aggressive rate hike
                cycle beginning in 2022 to combat rising inflation, leading to economic growth
                moderation.
              </Typography>

              <Typography component='li' variant='body2' paragraph>
                <strong>Current Outlook (2023-2024):</strong> Continued economic expansion with
                gradually moderating inflation and stable unemployment levels.
              </Typography>
            </Box>

            <Typography variant='body2' color='text.secondary' sx={{ mt: 2 }}>
              Analysis based on technical indicators, economic cycle detection, and collaborative
              annotations from economic research team.
            </Typography>
          </Paper>
        </Grid>

        <Grid item xs={12} md={4}>
          <Paper sx={{ p: 3 }}>
            <Typography variant='h6' sx={{ mb: 2 }}>
              Quick Actions
            </Typography>

            <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              <Button
                variant='outlined'
                startIcon={<ShowChartIcon />}
                onClick={handleSeriesAdd}
                fullWidth
              >
                Add Comparison Series
              </Button>

              <Button
                variant='outlined'
                startIcon={<PeopleIcon />}
                onClick={() => setCollaborationOpen(true)}
                fullWidth
              >
                Open Collaboration Panel
              </Button>

              <Button
                variant='outlined'
                startIcon={<AnalyticsIcon />}
                onClick={() => navigate('/dashboard')}
                fullWidth
              >
                Back to Dashboard
              </Button>
            </Box>

            <Typography variant='h6' sx={{ mt: 3, mb: 2 }}>
              Technical Analysis
            </Typography>

            <Typography variant='body2' color='text.secondary' paragraph>
              Available indicators: SMA, EMA, Bollinger Bands, RSI, ROC, Standard Deviation,
              Economic Cycle Detection, and Correlation Analysis.
            </Typography>

            <Typography variant='body2' color='text.secondary'>
              Enable technical analysis tools in the chart controls to overlay statistical
              indicators and identify economic trends and patterns.
            </Typography>
          </Paper>
        </Grid>
      </Grid>

      {/* Floating Action Button for Collaboration */}
      <Fab
        color='primary'
        aria-label='collaboration'
        sx={{
          position: 'fixed',
          bottom: 16,
          right: collaborationOpen ? 416 : 16,
          transition: 'right 0.3s ease',
        }}
        onClick={() => setCollaborationOpen(!collaborationOpen)}
      >
        <PeopleIcon />
      </Fab>

      {/* Chart Collaboration Panel */}
      <ChartCollaboration
        annotations={annotations}
        onAnnotationAdd={handleAnnotationAdd}
        onAnnotationUpdate={handleAnnotationUpdate}
        onAnnotationDelete={handleAnnotationDelete}
        onCommentAdd={handleCommentAdd}
        currentUser={CURRENT_USER}
        collaborators={MOCK_COLLABORATORS}
        isOpen={collaborationOpen}
        onToggle={() => setCollaborationOpen(!collaborationOpen)}
      />

      {/* Series Selection Dialog */}
      <Dialog
        open={seriesSelectionOpen}
        onClose={() => setSeriesSelectionOpen(false)}
        maxWidth='md'
        fullWidth
        aria-labelledby='series-selection-title'
        aria-describedby='series-selection-description'
        disableEnforceFocus={false}
        disableAutoFocus={false}
        disableRestoreFocus={false}
      >
        <DialogTitle id='series-selection-title'>Add Comparison Series</DialogTitle>
        <DialogContent>
          <Typography
            id='series-selection-description'
            variant='body2'
            color='text.secondary'
            sx={{ mb: 2 }}
          >
            Select additional economic series to overlay on the chart for comparative analysis.
          </Typography>

          <List>
            {SAMPLE_SERIES.filter(series => series.id !== primarySeries.id).map(series => (
              <ListItem key={series.id}>
                <FormControlLabel
                  control={
                    <Checkbox
                      checked={selectedSeriesIds.includes(series.id)}
                      onChange={() => handleSeriesToggle(series.id)}
                    />
                  }
                  label={
                    <ListItemText
                      primary={series.title}
                      secondary={`${series.description} • ${series.frequency} • ${series.unit}`}
                    />
                  }
                />
              </ListItem>
            ))}
          </List>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setSeriesSelectionOpen(false)}>Cancel</Button>
          <Button onClick={handleSeriesSelection} variant='contained'>
            Add Selected Series
          </Button>
        </DialogActions>
      </Dialog>
    </Container>
  );
};

export default ProfessionalAnalysis;
