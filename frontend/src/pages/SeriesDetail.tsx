import React from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import {
  Box,
  Typography,
  Grid,
  Card,
  CardContent,
  Chip,
  Button,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Breadcrumbs,
  Link,
  Skeleton,
  Alert,
  LinearProgress,
} from '@mui/material';
import {
  ArrowBack as ArrowBackIcon,
  Info as InfoIcon,
  TrendingUp as TrendingUpIcon,
} from '@mui/icons-material';

import SeriesChart, { SeriesChartAnnotation } from '../components/charts/SeriesChart';
import SeriesAnnotationsPanel from '../components/charts/SeriesAnnotationsPanel';
import { useSeriesData, useSeriesDetail, SeriesDataPoint } from '../hooks/useSeriesData';
import { useSeriesAnnotations } from '../hooks/useSeriesAnnotations';
import { useSeriesAnnotationEditor } from '../components/annotations/useSeriesAnnotationEditor';
import { DataTransformation, describeTransformation } from '../utils/transformations';
import { formatIsoDate } from '../utils/dates';

/**
 * Series page: one economic series' metadata, chart and latest observations, read from
 * the GraphQL API. Transformations are computed by the backend.
 */

const BackToExplorer: React.FC = () => {
  const navigate = useNavigate();
  return (
    <Button startIcon={<ArrowBackIcon />} onClick={() => navigate('/explore')}>
      Back to Explorer
    </Button>
  );
};

const PageSkeleton: React.FC = () => (
  <Box data-testid='series-detail-loading'>
    <Skeleton variant='text' width='60%' height={40} sx={{ mb: 2 }} />
    <Skeleton variant='text' width='80%' height={24} sx={{ mb: 4 }} />
    <Grid container spacing={3}>
      <Grid item xs={12} lg={8}>
        <Skeleton variant='rectangular' height={500} />
      </Grid>
      <Grid item xs={12} lg={4}>
        <Skeleton variant='rectangular' height={300} />
      </Grid>
    </Grid>
  </Box>
);

const formatValue = (value: number | null): string =>
  value === null
    ? 'N/A'
    : value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });

const hasValues = (points: SeriesDataPoint[] | undefined): points is SeriesDataPoint[] =>
  !!points && points.some(p => p.value !== null);

/**
 * Today as a local calendar date.
 * @returns `YYYY-MM-DD`.
 */
const todayIso = () => {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
};

const SeriesDetailContent: React.FC<{ seriesId: string }> = ({ seriesId }) => {
  const navigate = useNavigate();
  const [transformation, setTransformation] = React.useState<DataTransformation>('NONE');

  const detail = useSeriesDetail(seriesId);
  const series = detail.data;
  const data = useSeriesData(seriesId, { transformation, enabled: !!series });
  const annotations = useSeriesAnnotations(seriesId, { enabled: !!series });
  const [selectedAnnotationId, setSelectedAnnotationId] = React.useState<string | null>(null);

  const chartAnnotations = React.useMemo<SeriesChartAnnotation[]>(
    () =>
      (annotations.data ?? []).map(a => ({
        kind: 'point',
        id: a.id,
        label: a.title,
        date: a.date,
        color: a.color,
      })),
    [annotations.data]
  );

  // The create form opens on the latest observation unless a point on the chart was clicked.
  const latestDate = React.useMemo(() => {
    const points = data.data?.points ?? [];
    for (let i = points.length - 1; i >= 0; i--) {
      if (points[i].value !== null) return points[i].date;
    }
    return undefined;
  }, [data.data?.points]);
  const editor = useSeriesAnnotationEditor({
    seriesId,
    defaultDate: latestDate ?? series?.endDate?.slice(0, 10) ?? todayIso(),
    onCreated: setSelectedAnnotationId,
    onDeleted: id => setSelectedAnnotationId(selected => (selected === id ? null : selected)),
  });

  if (detail.isLoading) {
    return <PageSkeleton />;
  }

  if (detail.isError) {
    return (
      <Box>
        <Alert
          severity='error'
          sx={{ mb: 3 }}
          action={
            <Button color='inherit' size='small' onClick={() => detail.refetch()}>
              Retry
            </Button>
          }
        >
          Could not load this series. {(detail.error as Error | null)?.message}
        </Alert>
        <BackToExplorer />
      </Box>
    );
  }

  if (!series) {
    return (
      <Box>
        <Alert severity='warning' sx={{ mb: 3 }}>
          Series not found. It may have been removed, or the link may be wrong.
        </Alert>
        <BackToExplorer />
      </Box>
    );
  }

  const units = series.units ?? '';
  // Label what is shown: while a new transformation loads, the previous one stays on screen.
  const shownTransformation = data.data?.transformation ?? transformation;
  const transformationLabel = describeTransformation(shownTransformation);
  const points = data.data?.points;
  const recentPoints = hasValues(points)
    ? points
        .filter(p => p.value !== null)
        .slice(-10)
        .reverse()
    : [];

  let chart: React.ReactNode;
  if (data.isLoading) {
    chart = <Skeleton variant='rectangular' height={500} data-testid='series-chart-loading' />;
  } else if (data.isError) {
    chart = (
      <Alert
        severity='error'
        action={
          <Box sx={{ display: 'flex', gap: 1 }}>
            <Button color='inherit' size='small' onClick={() => data.refetch()}>
              Retry
            </Button>
            {transformation !== 'NONE' && (
              <Button color='inherit' size='small' onClick={() => setTransformation('NONE')}>
                Show levels
              </Button>
            )}
          </Box>
        }
      >
        Could not load observations for this series. {(data.error as Error | null)?.message}
      </Alert>
    );
  } else if (hasValues(points)) {
    chart = (
      <Box>
        {data.isPreviousData && <LinearProgress aria-label='Loading transformation' />}
        <SeriesChart
          data={points}
          title={series.title}
          units={units}
          frequency={series.frequency}
          transformation={shownTransformation}
          selectedTransformation={transformation}
          onTransformationChange={setTransformation}
          annotations={chartAnnotations}
          onAnnotationClick={setSelectedAnnotationId}
          onPointClick={editor.onPointClick}
        />
      </Box>
    );
  } else if (shownTransformation !== 'NONE') {
    chart = (
      <Alert
        severity='info'
        action={
          <Button color='inherit' size='small' onClick={() => setTransformation('NONE')}>
            Show levels
          </Button>
        }
      >
        Not enough observations to compute {transformationLabel.toLowerCase()} for this series.
      </Alert>
    );
  } else {
    chart = (
      <Paper sx={{ p: 3 }}>
        <Typography variant='body1'>No observations for this series yet.</Typography>
        <Typography variant='body2' color='text.secondary'>
          Data appears here after the next successful crawl of {series.source?.name ?? 'its source'}
          .
        </Typography>
      </Paper>
    );
  }

  return (
    <Box>
      <Breadcrumbs sx={{ mb: 2 }}>
        <Link
          color='inherit'
          href='/explore'
          onClick={e => {
            e.preventDefault();
            navigate('/explore');
          }}
        >
          Explore
        </Link>
        <Typography color='text.primary'>{series.title}</Typography>
      </Breadcrumbs>

      <Box sx={{ mb: 4 }}>
        <Typography variant='h4' component='h1' gutterBottom>
          {series.title}
        </Typography>
        {series.description && (
          <Typography variant='body1' color='text.secondary' sx={{ mb: 2 }}>
            {series.description}
          </Typography>
        )}
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1 }}>
          {series.source && <Chip label={series.source.name} color='primary' variant='outlined' />}
          <Chip label={series.frequency} variant='outlined' />
          {series.units && <Chip label={series.units} variant='outlined' />}
          {series.seasonalAdjustment && (
            <Chip label={series.seasonalAdjustment} variant='outlined' />
          )}
        </Box>
      </Box>

      <Grid container spacing={3}>
        <Grid item xs={12} lg={8}>
          {chart}
        </Grid>

        <Grid item xs={12} lg={4}>
          <Box sx={{ mb: 3 }}>
            <SeriesAnnotationsPanel
              annotations={annotations.data ?? []}
              isLoading={annotations.isLoading}
              error={annotations.isError ? (annotations.error as Error | null) : null}
              onRetry={() => annotations.refetch()}
              selectedId={selectedAnnotationId}
              onSelect={setSelectedAnnotationId}
              headerAction={editor.headerAction}
              renderAnnotationExtra={editor.renderAnnotationExtra}
            />
          </Box>
          {editor.dialogs}

          <Card sx={{ mb: 3 }}>
            <CardContent>
              <Typography variant='h6' gutterBottom sx={{ display: 'flex', alignItems: 'center' }}>
                <InfoIcon sx={{ mr: 1 }} />
                Series Information
              </Typography>

              <Table size='small'>
                <TableBody>
                  <TableRow>
                    <TableCell>
                      <strong>Series ID</strong>
                    </TableCell>
                    <TableCell>{series.externalId}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell>
                      <strong>Source</strong>
                    </TableCell>
                    <TableCell>{series.source?.name ?? '—'}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell>
                      <strong>Frequency</strong>
                    </TableCell>
                    <TableCell>{series.frequency}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell>
                      <strong>Units</strong>
                    </TableCell>
                    <TableCell>{series.units ?? '—'}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell>
                      <strong>Date Range</strong>
                    </TableCell>
                    <TableCell>
                      {series.startDate ? formatIsoDate(series.startDate) : '—'} to{' '}
                      {series.endDate ? formatIsoDate(series.endDate) : '—'}
                    </TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell>
                      <strong>Last Updated</strong>
                    </TableCell>
                    <TableCell>
                      {series.lastUpdated
                        ? new Date(series.lastUpdated).toLocaleDateString('en-US', {
                            year: 'numeric',
                            month: 'short',
                            day: 'numeric',
                          })
                        : '—'}
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </CardContent>
          </Card>

          {recentPoints.length > 0 && (
            <Card>
              <CardContent>
                <Typography
                  variant='h6'
                  gutterBottom
                  sx={{ display: 'flex', alignItems: 'center' }}
                >
                  <TrendingUpIcon sx={{ mr: 1 }} />
                  Recent Data
                </Typography>

                <TableContainer>
                  <Table size='small' aria-label='Recent observations'>
                    <TableHead>
                      <TableRow>
                        <TableCell>
                          <strong>Date</strong>
                        </TableCell>
                        <TableCell align='right'>
                          <strong>{transformationLabel || units || 'Value'}</strong>
                        </TableCell>
                        <TableCell align='center'>
                          <strong>Type</strong>
                        </TableCell>
                      </TableRow>
                    </TableHead>
                    <TableBody>
                      {recentPoints.map(point => (
                        <TableRow key={point.date}>
                          <TableCell>{formatIsoDate(point.date)}</TableCell>
                          <TableCell align='right'>{formatValue(point.value)}</TableCell>
                          <TableCell align='center'>
                            <Chip
                              label={point.isOriginalRelease ? 'Original' : 'Revised'}
                              size='small'
                              color={point.isOriginalRelease ? 'secondary' : 'primary'}
                              variant='outlined'
                            />
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </TableContainer>
              </CardContent>
            </Card>
          )}
        </Grid>
      </Grid>
    </Box>
  );
};

const SeriesDetail: React.FC = () => {
  const { id } = useParams<{ id: string }>();

  if (!id) {
    return (
      <Box>
        <Alert severity='error' sx={{ mb: 3 }}>
          No series ID provided
        </Alert>
        <BackToExplorer />
      </Box>
    );
  }

  // key resets the chosen transformation when navigating to another series
  return <SeriesDetailContent key={id} seriesId={id} />;
};

export default SeriesDetail;
