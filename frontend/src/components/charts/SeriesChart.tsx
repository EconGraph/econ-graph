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
  type ActiveElement,
  type ChartEvent,
} from 'chart.js';
import annotationPlugin from 'chartjs-plugin-annotation';
import { Line } from 'react-chartjs-2';
import 'chartjs-adapter-date-fns';
import {
  Alert,
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
  formatTransformedValue,
  levelFractionDigits,
  scaledTickLabel,
  transformationsFor,
  describeTransformation,
  transformedAxisTitle,
  transformedUnits,
} from '../../utils/transformations';
import { formatIsoDate, formatObservationDate, parseIsoDate } from '../../utils/dates';
import { classifyFrequency } from '../../utils/frequency';
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

/** The longest a quarterly series' axis can span and still tick every quarter. */
const QUARTER_TICKS_MAX_MS = 6 * 365.25 * 86_400_000;

/**
 * Time-axis units that keep ticks on a series' own periods: a quarterly series ticks quarters
 * ("2025 Q2") over a short span and years over a long one, a semi-annual or annual series
 * years, never months or days ("Apr 2025"). Other frequencies are left to Chart.js.
 * @param frequency - The series' frequency.
 * @param spanMs - How long the axis spans.
 * @returns Chart.js `time` options.
 */
const timeUnits = (frequency: string, spanMs: number) => {
  const periodDays = classifyFrequency(frequency)?.periodDays ?? 0;
  if (periodDays === 91 && spanMs <= QUARTER_TICKS_MAX_MS) return { unit: 'quarter' as const };
  if (periodDays >= 91) return { minUnit: 'year' as const };
  return {};
};

const NO_ANNOTATIONS: readonly SeriesChartAnnotation[] = [];

// Click events the annotation plugin has already handled. Chart.js hands the same event object
// to the plugin first and then to the chart's own onClick, which skips events listed here, so
// a click on an annotation doesn't also pick the point under it.
const annotationClicks = new WeakSet<object>();

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
  /**
   * Called with the observation nearest a click on the plot (not on an annotation), for
   * example to annotate it. Pass a stable function (`useCallback`).
   */
  onPointClick?: (point: SeriesChartPoint) => void;
  /**
   * Shown in place of the plot when `data` has no values (e.g. a transformation the series is
   * too short for), so the controls stay to pick something else.
   */
  emptyMessage?: React.ReactNode;
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
 * @param props.onPointClick - Called with the observation nearest a click on the plot.
 * @param props.emptyMessage - Shown instead of the plot when the series has no values; a date
 *   range that leaves out all of them gets its own message instead.
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
  onPointClick,
  emptyMessage,
}) => {
  const theme = useTheme();

  const handleAnnotationClick = React.useMemo(
    () =>
      onAnnotationClick || onPointClick
        ? (id: string, event: unknown) => {
            if (typeof event === 'object' && event !== null) annotationClicks.add(event);
            onAnnotationClick?.(id);
          }
        : undefined,
    [onAnnotationClick, onPointClick]
  );

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
  // The pickers flag a start after the end, but the chart would plot an empty, backwards axis.
  const rangeInverted =
    isValidDate(range.start) && isValidDate(range.end) && range.start > range.end;
  const axisRange = React.useMemo(() => visibleRange(range, shownData), [range, shownData]);
  // From every point, so a level keeps its decimals whatever the date range.
  const levelDigits = React.useMemo(() => levelFractionDigits(data.map(d => d.value)), [data]);
  const largeValues = shownData.some(d => d.value !== null && Math.abs(d.value) >= 1e6);
  const timeSpan =
    axisRange.start && axisRange.end ? axisRange.end.getTime() - axisRange.start.getTime() : 0;

  const annotationOptions = React.useMemo(
    () =>
      buildAnnotationOptions(
        annotations,
        axisRange,
        theme.palette.warning.main,
        handleAnnotationClick
      ),
    [annotations, axisRange, theme.palette.warning.main, handleAnnotationClick]
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

  const valueUnit = transformedUnits(transformation, units);

  const chartOptions = React.useMemo(() => {
    return {
      responsive: true,
      maintainAspectRatio: false,
      interaction: {
        mode: 'index' as const,
        intersect: false,
      },
      // With the index interaction, the active element is the point nearest the click's x.
      onClick: onPointClick
        ? (event: ChartEvent, elements: ActiveElement[]) => {
            if (annotationClicks.has(event)) return;
            const point = elements.length > 0 ? shownData[elements[0].index] : undefined;
            if (point) onPointClick(point);
          }
        : undefined,
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
                ? formatObservationDate(point.date, frequency, {
                    year: 'numeric',
                    month: 'long',
                    day: 'numeric',
                  })
                : '';
            },
            label: (item: TooltipItem<'line'>) => {
              const point = item.raw as { date: string; revisionDate?: string };
              const value = item.parsed.y;
              const formattedValue =
                value === null ? 'n/a' : formatTransformedValue(value, transformation, levelDigits);
              const lines = [`${item.dataset.label}: ${formattedValue} ${valueUnit}`.trimEnd()];
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
            ...timeUnits(frequency, timeSpan),
            displayFormats: {
              day: 'MMM dd',
              week: 'MMM dd',
              month: 'MMM yyyy',
              quarter: "yyyy 'Q'q",
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
            text: transformedAxisTitle(transformation, units),
          },
          grid: {
            color: theme.palette.divider,
          },
          // Dollar levels such as BEA's GDP run to 14 digits; scale them ("28.6T"). Smaller
          // values keep Chart.js' own labels.
          ticks: largeValues
            ? {
                callback: (value: number | string, _index: number, ticks: { value: number }[]) =>
                  scaledTickLabel(
                    Number(value),
                    ticks.map(t => t.value)
                  ) ?? Number(value).toLocaleString('en-US'),
              }
            : {},
        },
      },
    };
  }, [
    frequency,
    timeSpan,
    levelDigits,
    largeValues,
    transformed,
    transformation,
    units,
    valueUnit,
    title,
    transformationLabel,
    theme,
    annotationOptions,
    range,
    shownData,
    onPointClick,
  ]);

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
              <InputLabel id='series-transformation-label'>Transformation</InputLabel>
              <Select
                labelId='series-transformation-label'
                value={selectedTransformation}
                onChange={e => onTransformationChange?.(e.target.value as DataTransformation)}
                label='Transformation'
                disabled={!onTransformationChange}
              >
                {transformationsFor(frequency).map(option => (
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

      {rangeInverted ? (
        <Alert severity='warning'>
          The start date ({formatDate(range.start as Date)}) is after the end date (
          {formatDate(range.end as Date)}). Pick a start date on or before the end date.
        </Alert>
      ) : shownData.some(d => d.value !== null) ? (
        <Box
          sx={{ height: 400 }}
          data-testid='series-chart'
          role='img'
          aria-label={`Line chart of ${title}`}
        >
          <Line data={chartData} options={chartOptions} />
        </Box>
      ) : (
        <Alert severity='info'>
          {data.some(d => d.value !== null)
            ? 'No observations in the chosen date range. Widen or clear the dates to see them.'
            : (emptyMessage ?? 'No observations to show.')}
        </Alert>
      )}

      <Box sx={{ mt: 2, pt: 2, borderTop: 1, borderColor: 'divider' }}>
        <Typography variant='caption' color='text.secondary'>
          Frequency: {frequency} • Data Points: {shownData.filter(d => d.value !== null).length}
        </Typography>
      </Box>
    </Paper>
  );
};

export default React.memo(SeriesChart);
