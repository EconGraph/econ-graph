import React from 'react';
import {
  Chart as ChartJS,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  TimeScale,
  TooltipItem,
} from 'chart.js';
import annotationPlugin from 'chartjs-plugin-annotation';
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
import {
  EMPTY_DATE_RANGE,
  SeriesChartAnnotation,
  SeriesChartDateRange,
  SeriesChartPoint,
  boundTime,
  buildAnnotationOptions,
  selectShownPoints,
  visibleRange,
} from './seriesChartData';

export type {
  SeriesChartAnnotation,
  SeriesChartDateRange,
  SeriesChartPoint,
  SeriesChartPointAnnotation,
  SeriesChartRangeAnnotation,
} from './seriesChartData';
export { selectShownPoints } from './seriesChartData';

ChartJS.register(
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  TimeScale,
  annotationPlugin
);

const isValidDate = (date: Date | null): date is Date =>
  date !== null && !Number.isNaN(date.getTime());

const formatDate = (date: Date): string =>
  date.toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric' });

const NO_ANNOTATIONS: readonly SeriesChartAnnotation[] = [];

export interface SeriesChartProps {
  /** Points to plot, already transformed by the backend, in any order. */
  data: readonly SeriesChartPoint[];
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
  /**
   * Shown date range. Pass it together with `onDateRangeChange` to control the range from
   * the caller (for example to export what is shown); without `onDateRangeChange` the
   * pickers and chips are read-only. Leave both out and the chart keeps its own range.
   */
  dateRange?: SeriesChartDateRange;
  /** Called when the user changes the date range. */
  onDateRangeChange?: (range: SeriesChartDateRange) => void;
  /** Notes drawn on the chart: points as vertical lines, ranges as shaded boxes. */
  annotations?: readonly SeriesChartAnnotation[];
  /**
   * Called with an annotation's id when it is clicked on the chart. Pass a stable function
   * (`useCallback`); a new one each render rebuilds the chart options.
   */
  onAnnotationClick?: (id: string) => void;
}

/**
 * One economic series on a time axis, with date range and transformation controls and
 * optional annotations. Values come from the backend; the chart does no arithmetic.
 * @param props - The component props (see `SeriesChartProps`).
 * @param props.data - Points to plot.
 * @param props.title - Series title.
 * @param props.units - Units of the untransformed values.
 * @param props.frequency - Series frequency, for the caption.
 * @param props.transformation - Transformation the data carries.
 * @param props.selectedTransformation - Transformation the selector shows.
 * @param props.onTransformationChange - Called with a newly selected transformation.
 * @param props.dateRange - Controlled date range.
 * @param props.onDateRangeChange - Called when the user changes the date range.
 * @param props.annotations - Notes to draw on the chart.
 * @param props.onAnnotationClick - Called with a clicked annotation's id.
 * @returns The chart with its controls.
 */
const SeriesChart: React.FC<SeriesChartProps> = ({
  data,
  title,
  units,
  frequency,
  transformation = 'NONE',
  selectedTransformation = transformation,
  onTransformationChange,
  dateRange,
  onDateRangeChange,
  annotations = NO_ANNOTATIONS,
  onAnnotationClick,
}) => {
  const theme = useTheme();

  const [ownRange, setOwnRange] = React.useState<SeriesChartDateRange>(EMPTY_DATE_RANGE);
  const range = dateRange ?? ownRange;
  // A controlled range without a change handler can't be edited from the chart.
  const rangeLocked = dateRange !== undefined && onDateRangeChange === undefined;
  const setRange = (next: SeriesChartDateRange) => {
    if (dateRange === undefined) setOwnRange(next);
    onDateRangeChange?.(next);
  };

  // Transformations are computed by the backend over the whole series, so filtering here
  // keeps the first year of a year-over-year view.
  const shownData = React.useMemo(() => selectShownPoints(data, range), [data, range]);

  const annotationOptions = React.useMemo(
    () =>
      buildAnnotationOptions(
        annotations,
        visibleRange(range, shownData),
        theme.palette.warning.main,
        onAnnotationClick
      ),
    [annotations, range, shownData, theme.palette.warning.main, onAnnotationClick]
  );

  const chartData = React.useMemo(
    () => ({
      datasets: [
        {
          label: title,
          data: shownData.map(d => ({
            x: parseIsoDate(d.date).getTime(),
            y: d.value,
            date: d.date,
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
    }),
    [shownData, title, theme.palette.primary.main]
  );

  const transformationLabel = describeTransformation(transformation);
  const transformed = transformation !== 'NONE';

  const chartOptions = React.useMemo(() => {
    const valueUnit = transformed ? '%' : units;
    return {
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
          callbacks: {
            title: (items: TooltipItem<'line'>[]) => {
              const point = items[0]?.raw as { date?: string } | undefined;
              return point?.date
                ? formatIsoDate(point.date, { year: 'numeric', month: 'long', day: 'numeric' })
                : '';
            },
            label: (item: TooltipItem<'line'>) => {
              const point = item.raw as { date: string; revisionDate?: string };
              const value = item.parsed.y;
              const lines = [
                `${item.dataset.label}: ${value === null ? 'n/a' : value.toFixed(2)} ${valueUnit}`,
              ];
              if (point.revisionDate && point.revisionDate !== point.date) {
                lines.push(`Revised: ${formatIsoDate(point.revisionDate)}`);
              }
              return lines;
            },
          },
          backgroundColor: theme.palette.background.paper,
          titleColor: theme.palette.text.primary,
          bodyColor: theme.palette.text.primary,
          borderColor: theme.palette.divider,
          borderWidth: 1,
        },
        annotation: {
          // Without this the plugin inherits the chart's index mode and treats a click
          // anywhere on the plot as a click on the nearest annotation.
          interaction: { mode: 'nearest' as const, intersect: true },
          annotations: annotationOptions,
        },
      },
      scales: {
        x: {
          type: 'time' as const,
          // Pin the axis to the chosen range; open bounds follow the data.
          min: boundTime(range.start),
          max: boundTime(range.end),
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
  }, [transformed, units, title, transformationLabel, theme, annotationOptions, range]);

  return (
    <Paper sx={{ p: 3 }}>
      <Box sx={{ mb: 3 }}>
        <Typography variant='h6' gutterBottom>
          Chart Controls
        </Typography>

        <Grid container spacing={2} alignItems='center'>
          <Grid item xs={12} sm={6} md={3}>
            <LocalizationProvider dateAdapter={AdapterDateFns}>
              <DatePicker
                label='Start Date'
                value={range.start}
                readOnly={rangeLocked}
                maxDate={isValidDate(range.end) ? range.end : undefined}
                onChange={start => setRange({ ...range, start })}
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
                value={range.end}
                readOnly={rangeLocked}
                minDate={isValidDate(range.start) ? range.start : undefined}
                onChange={end => setRange({ ...range, end })}
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

        <Box sx={{ mt: 2, display: 'flex', flexWrap: 'wrap', gap: 1 }}>
          {transformed && (
            <Chip
              label={transformationLabel}
              size='small'
              color='primary'
              onDelete={onTransformationChange ? () => onTransformationChange('NONE') : undefined}
            />
          )}
          {isValidDate(range.start) && (
            <Chip
              label={`From: ${formatDate(range.start)}`}
              size='small'
              variant='outlined'
              onDelete={rangeLocked ? undefined : () => setRange({ ...range, start: null })}
            />
          )}
          {isValidDate(range.end) && (
            <Chip
              label={`To: ${formatDate(range.end)}`}
              size='small'
              variant='outlined'
              onDelete={rangeLocked ? undefined : () => setRange({ ...range, end: null })}
            />
          )}
        </Box>
      </Box>

      <Box sx={{ height: 400 }} data-testid='series-chart' aria-label={`Line chart of ${title}`}>
        <Line data={chartData} options={chartOptions} />
      </Box>

      <Box sx={{ mt: 2, pt: 2, borderTop: 1, borderColor: 'divider' }}>
        <Typography variant='caption' color='text.secondary'>
          Frequency: {frequency} • Data Points: {shownData.filter(d => d.value !== null).length}
        </Typography>
      </Box>
    </Paper>
  );
};

export default React.memo(SeriesChart);
