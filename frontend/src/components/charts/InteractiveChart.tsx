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
  Grid,
  Chip,
  useTheme,
} from '@mui/material';
import { DatePicker } from '@mui/x-date-pickers/DatePicker';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import {
  DataTransformation,
  TRANSFORMATION_OPTIONS,
  describeTransformation,
} from '../../utils/transformations';
import { formatIsoDate, parseIsoDate } from '../../utils/dates';

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
  /** Points to plot, already transformed by the backend. */
  data: DataPoint[];
  title: string;
  units: string;
  frequency: string;
  /** Transformation the data carries; the selector shows it. */
  transformation?: DataTransformation;
  /**
   * Transformation the selector shows, when it differs from the data's (a newer choice
   * that is still loading). Defaults to `transformation`.
   */
  selectedTransformation?: DataTransformation;
  /** Called when the user picks another transformation; the caller refetches. */
  onTransformationChange?: (transformation: DataTransformation) => void;
}

/**
 * REQUIREMENT: Interactive charts with mouse-overs to see individual values and dates in tooltips
 * PURPOSE: Provide rich, interactive visualization of economic time series data
 * This implements the core charting functionality with modern UX patterns.
 * @param root0 - The component props object.
 * @param root0.data - The time series data to display.
 * @param root0.title - The chart title.
 * @param root0.units - The units for the data values.
 * @param root0.frequency - The frequency of the data (daily, monthly, etc.).
 * @param root0.transformation - The backend transformation the data carries.
 * @param root0.selectedTransformation - The transformation the selector shows.
 * @param root0.onTransformationChange - Called with a newly selected transformation.
 * @returns JSX element representing the interactive chart.
 */
const InteractiveChart: React.FC<ChartProps> = ({
  data,
  title,
  units,
  frequency,
  transformation = 'NONE',
  selectedTransformation = transformation,
  onTransformationChange,
}) => {
  const theme = useTheme();

  // State for chart controls
  const [startDate, setStartDate] = React.useState<Date | null>(null);
  const [endDate, setEndDate] = React.useState<Date | null>(null);

  // Filter by the chosen date range. Transformations are computed by the backend over the
  // whole series, so filtering here keeps the first year of a year-over-year view.
  const shownData = React.useMemo(
    () =>
      data.filter(d => {
        const date = parseIsoDate(d.date);
        return (!startDate || date >= startDate) && (!endDate || date <= endDate);
      }),
    [data, startDate, endDate]
  );

  // Chart.js configuration
  const chartData = {
    datasets: [
      {
        label: title,
        data: shownData.map(d => ({
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
    ],
  };

  const transformationLabel = describeTransformation(transformation);
  const transformed = transformation !== 'NONE';

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
        text: transformed ? `${title} (${transformationLabel})` : title,
        font: {
          size: 16,
          weight: 'bold' as const,
        },
      },
      legend: {
        display: false,
      },
      tooltip: {
        // REQUIREMENT: Mouse-overs to see individual values and corresponding dates in tooltips
        callbacks: {
          title: (context: TooltipItem<'line'>[]) => {
            const date = new Date(context[0].parsed.x ?? 0);
            return date.toLocaleDateString('en-US', {
              year: 'numeric',
              month: 'long',
              day: 'numeric',
            });
          },
          label: (context: TooltipItem<'line'>) => {
            const value = context.parsed.y;
            const dataPoint = context.raw as any;
            const transformationUnit = transformed ? '%' : units;

            let label = `${context.dataset.label}: ${value?.toFixed(2)} ${transformationUnit}`;

            if (dataPoint.revisionDate && dataPoint.revisionDate !== dataPoint.x) {
              label += `\nRevised: ${formatIsoDate(dataPoint.revisionDate)}`;
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
          text: transformed ? 'Percent Change' : units,
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
                value={selectedTransformation}
                onChange={e => onTransformationChange?.(e.target.value as DataTransformation)}
                label='Transformation'
                disabled={!onTransformationChange}
              >
                {TRANSFORMATION_OPTIONS.map(option => (
                  <MenuItem key={option.value} value={option.value}>
                    {option.label}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
          </Grid>
        </Grid>

        {/* Active filters display */}
        <Box sx={{ mt: 2, display: 'flex', flexWrap: 'wrap', gap: 1 }}>
          {transformed && (
            <Chip
              label={transformationLabel}
              size='small'
              color='primary'
              onDelete={onTransformationChange ? () => onTransformationChange('NONE') : undefined}
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
          Frequency: {frequency} • Data Points: {shownData.filter(d => d.value !== null).length}
        </Typography>
      </Box>
    </Paper>
  );
};

// Memoize the component to prevent unnecessary re-renders
export default React.memo(InteractiveChart);
