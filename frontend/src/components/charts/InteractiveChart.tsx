import React from 'react';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  TimeScale,
  TooltipItem,
} from 'chart.js';
import { Line } from 'react-chartjs-2';
import 'chartjs-adapter-date-fns';
import {
  Box,
  Paper,
  Typography,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  FormControlLabel,
  Switch,
  Grid,
  Chip,
  useTheme,
} from '@mui/material';
import { DatePicker } from '@mui/x-date-pickers/DatePicker';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  TimeScale
);

interface DataPoint {
  date: string;
  value: number | null;
  isOriginalRelease: boolean;
  revisionDate: string;
}

interface ChartProps {
  data: DataPoint[];
  title: string;
  units: string;
  frequency: string;
}

type TransformationType = 'none' | 'yoy' | 'qoq' | 'mom';

/**
 * REQUIREMENT: Interactive charts with mouse-overs to see individual values and dates in tooltips
 * PURPOSE: Provide rich, interactive visualization of economic time series data
 * This implements the core charting functionality with modern UX patterns.
 * @param root0 - The component props object.
 * @param root0.data - The time series data to display.
 * @param root0.title - The chart title.
 * @param root0.units - The units for the data values.
 * @param root0.frequency - The frequency of the data (daily, monthly, etc.).
 * @returns JSX element representing the interactive chart.
 */
const InteractiveChart: React.FC<ChartProps> = ({ data, title, units, frequency }) => {
  const theme = useTheme();

  // State for chart controls
  const [startDate, setStartDate] = React.useState<Date | null>(null);
  const [endDate, setEndDate] = React.useState<Date | null>(null);
  const [transformation, setTransformation] = React.useState<TransformationType>('none');
  const [showOriginalReleases, setShowOriginalReleases] = React.useState(false);
  const [showRevisedData, setShowRevisedData] = React.useState(true);

  // Filter and transform data based on controls
  const processedData = React.useMemo(() => {
    let filteredData = data;

    // Apply date range filter
    if (startDate) {
      filteredData = filteredData.filter(d => new Date(d.date) >= startDate);
    }
    if (endDate) {
      filteredData = filteredData.filter(d => new Date(d.date) <= endDate);
    }

    // Apply revision filter
    if (!showOriginalReleases && !showRevisedData) {
      return { original: [], revised: [] };
    }

    const originalData = showOriginalReleases ? filteredData.filter(d => d.isOriginalRelease) : [];

    const revisedData = showRevisedData
      ? filteredData.filter(d => !d.isOriginalRelease || showOriginalReleases)
      : [];

    // Apply transformation
    const transformData = (points: DataPoint[]) => {
      if (transformation === 'none') return points;

      return points
        .map((point, index) => {
          if (point.value === null) return point;

          let transformedValue: number | null = null;
          const currentDate = new Date(point.date);

          if (transformation === 'yoy') {
            // Find data point from same period previous year
            const previousYear = new Date(currentDate);
            previousYear.setFullYear(currentDate.getFullYear() - 1);
            const previousPoint = points.find(
              p =>
                Math.abs(new Date(p.date).getTime() - previousYear.getTime()) <
                32 * 24 * 60 * 60 * 1000 // Within ~1 month
            );
            if (previousPoint?.value && previousPoint.value !== 0) {
              transformedValue = ((point.value - previousPoint.value) / previousPoint.value) * 100;
            }
          } else if (transformation === 'qoq') {
            // Find previous quarter (3 months ago)
            const previousQuarter = new Date(currentDate);
            previousQuarter.setMonth(currentDate.getMonth() - 3);
            const previousPoint = points.find(
              p =>
                Math.abs(new Date(p.date).getTime() - previousQuarter.getTime()) <
                45 * 24 * 60 * 60 * 1000 // Within ~1.5 months
            );
            if (previousPoint?.value && previousPoint.value !== 0) {
              transformedValue = ((point.value - previousPoint.value) / previousPoint.value) * 100;
            }
          } else if (transformation === 'mom') {
            // Find previous month
            const previousMonth = new Date(currentDate);
            previousMonth.setMonth(currentDate.getMonth() - 1);
            const previousPoint = points.find(
              p =>
                Math.abs(new Date(p.date).getTime() - previousMonth.getTime()) <
                16 * 24 * 60 * 60 * 1000 // Within ~2 weeks
            );
            if (previousPoint?.value && previousPoint.value !== 0) {
              transformedValue = ((point.value - previousPoint.value) / previousPoint.value) * 100;
            }
          }

          return { ...point, value: transformedValue };
        })
        .filter(p => p.value !== null);
    };

    return {
      original: transformData(originalData),
      revised: transformData(revisedData),
    };
  }, [data, startDate, endDate, transformation, showOriginalReleases, showRevisedData]);

  // Chart.js configuration
  const chartData = {
    datasets: [
      ...(showRevisedData
        ? [
            {
              label: showOriginalReleases ? 'Revised Data' : title,
              data: processedData.revised.map(d => ({
                x: d.date,
                y: d.value,
                originalRelease: d.isOriginalRelease,
                revisionDate: d.revisionDate,
              })),
              borderColor: theme.palette.primary.main,
              backgroundColor: theme.palette.primary.main + '20',
              borderWidth: 2,
              pointRadius: 3,
              pointHoverRadius: 6,
              tension: 0.1,
            },
          ]
        : []),
      ...(showOriginalReleases
        ? [
            {
              label: 'Original Releases',
              data: processedData.original.map(d => ({
                x: d.date,
                y: d.value,
                originalRelease: d.isOriginalRelease,
                revisionDate: d.revisionDate,
              })),
              borderColor: theme.palette.secondary.main,
              backgroundColor: theme.palette.secondary.main + '20',
              borderWidth: 2,
              pointRadius: 3,
              pointHoverRadius: 6,
              tension: 0.1,
              borderDash: [5, 5],
            },
          ]
        : []),
    ],
  };

  const getTransformationLabel = (type: TransformationType): string => {
    switch (type) {
      case 'yoy':
        return 'Year-over-Year % Change';
      case 'qoq':
        return 'Quarter-over-Quarter % Change';
      case 'mom':
        return 'Month-over-Month % Change';
      default:
        return '';
    }
  };

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    interaction: {
      mode: 'index' as const,
      intersect: false,
    },
    plugins: {
      title: {
        display: true,
        text: `${title}${transformation !== 'none' ? ` (${getTransformationLabel(transformation)})` : ''}`,
        font: {
          size: 16,
          weight: 'bold' as const,
        },
      },
      legend: {
        display: showOriginalReleases && showRevisedData,
        position: 'top' as const,
      },
      tooltip: {
        // REQUIREMENT: Mouse-overs to see individual values and corresponding dates in tooltips
        callbacks: {
          title: (context: TooltipItem<'line'>[]) => {
            const date = new Date(context[0].parsed.x);
            return date.toLocaleDateString('en-US', {
              year: 'numeric',
              month: 'long',
              day: 'numeric',
            });
          },
          label: (context: TooltipItem<'line'>) => {
            const value = context.parsed.y;
            const dataPoint = context.raw as any;
            const transformationUnit = transformation !== 'none' ? '%' : units;

            let label = `${context.dataset.label}: ${value?.toFixed(2)} ${transformationUnit}`;

            if (dataPoint.revisionDate && dataPoint.revisionDate !== context.parsed.x) {
              const revisionDate = new Date(dataPoint.revisionDate).toLocaleDateString();
              label += `\nRevised: ${revisionDate}`;
            }

            if (dataPoint.originalRelease) {
              label += '\n(Original Release)';
            }

            return label;
          },
        },
        backgroundColor: theme.palette.background.paper,
        titleColor: theme.palette.text.primary,
        bodyColor: theme.palette.text.primary,
        borderColor: theme.palette.divider,
        borderWidth: 1,
      },
    },
    scales: {
      x: {
        type: 'time' as const,
        time: {
          displayFormats: {
            day: 'MMM dd',
            week: 'MMM dd',
            month: 'MMM yyyy',
            quarter: 'MMM yyyy',
            year: 'yyyy',
          },
        },
        title: {
          display: true,
          text: 'Date',
        },
      },
      y: {
        title: {
          display: true,
          text: transformation !== 'none' ? 'Percent Change' : units,
        },
        grid: {
          color: theme.palette.divider,
        },
      },
    },
  };

  return (
    <Paper sx={{ p: 3 }}>
      {/* Chart controls */}
      <Box sx={{ mb: 3 }}>
        <Typography variant='h6' gutterBottom>
          Chart Controls
        </Typography>

        <Grid container spacing={2} alignItems='center'>
          {/* Date range controls */}
          <Grid item xs={12} sm={6} md={3}>
            <LocalizationProvider dateAdapter={AdapterDateFns}>
              <DatePicker
                label='Start Date'
                value={startDate}
                onChange={setStartDate}
                slotProps={{
                  textField: {
                    size: 'small',
                    fullWidth: true,
                    inputProps: { 'data-testid': 'date-picker-start', 'aria-label': 'Start Date' },
                  },
                }}
              />
            </LocalizationProvider>
          </Grid>

          <Grid item xs={12} sm={6} md={3}>
            <LocalizationProvider dateAdapter={AdapterDateFns}>
              <DatePicker
                label='End Date'
                value={endDate}
                onChange={setEndDate}
                slotProps={{
                  textField: {
                    size: 'small',
                    fullWidth: true,
                    inputProps: { 'data-testid': 'date-picker-end', 'aria-label': 'End Date' },
                  },
                }}
              />
            </LocalizationProvider>
          </Grid>

          {/* Transformation control */}
          <Grid item xs={12} sm={6} md={3}>
            <FormControl fullWidth size='small'>
              <InputLabel>Transformation</InputLabel>
              <Select
                value={transformation}
                onChange={e => setTransformation(e.target.value as TransformationType)}
                label='Transformation'
              >
                <MenuItem value='none'>None</MenuItem>
                <MenuItem value='yoy'>Year-over-Year</MenuItem>
                <MenuItem value='qoq'>Quarter-over-Quarter</MenuItem>
                <MenuItem value='mom'>Month-over-Month</MenuItem>
              </Select>
            </FormControl>
          </Grid>

          {/* Revision controls */}
          <Grid item xs={12} md={3}>
            <Box>
              <FormControlLabel
                control={
                  <Switch
                    checked={showRevisedData}
                    onChange={e => setShowRevisedData(e.target.checked)}
                    size='small'
                  />
                }
                label='Revised Data'
              />
              <FormControlLabel
                control={
                  <Switch
                    checked={showOriginalReleases}
                    onChange={e => setShowOriginalReleases(e.target.checked)}
                    size='small'
                  />
                }
                label='Original Releases'
              />
            </Box>
          </Grid>
        </Grid>

        {/* Active filters display */}
        <Box sx={{ mt: 2, display: 'flex', flexWrap: 'wrap', gap: 1 }}>
          {transformation !== 'none' && (
            <Chip
              label={getTransformationLabel(transformation)}
              size='small'
              color='primary'
              onDelete={() => setTransformation('none')}
            />
          )}
          {startDate && (
            <Chip
              label={`From: ${startDate.toLocaleDateString()}`}
              size='small'
              variant='outlined'
              onDelete={() => setStartDate(null)}
            />
          )}
          {endDate && (
            <Chip
              label={`To: ${endDate.toLocaleDateString()}`}
              size='small'
              variant='outlined'
              onDelete={() => setEndDate(null)}
            />
          )}
        </Box>
      </Box>

      {/* Chart */}
      <Box
        sx={{ height: 400 }}
        data-testid='line-chart-container'
        aria-label='Interactive line chart'
      >
        <Line data={chartData} options={chartOptions} />
      </Box>

      {/* Chart info */}
      <Box sx={{ mt: 2, pt: 2, borderTop: 1, borderColor: 'divider' }}>
        <Typography variant='caption' color='text.secondary'>
          Frequency: {frequency} • Data Points:{' '}
          {processedData.revised.length + processedData.original.length} •
          {showOriginalReleases && showRevisedData && ' Both original and revised data shown'}
        </Typography>
      </Box>
    </Paper>
  );
};

export default InteractiveChart;
