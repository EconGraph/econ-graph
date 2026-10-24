/**
 * Series transformations the backend computes (the GraphQL `DataTransformation` enum).
 *
 * The browser never transforms values itself: it sends one of these to `seriesData`
 * and plots what comes back.
 */

export type DataTransformation =
  | 'NONE'
  | 'YEAR_OVER_YEAR'
  | 'QUARTER_OVER_QUARTER'
  | 'MONTH_OVER_MONTH'
  | 'PERCENT_CHANGE'
  | 'LOG_DIFFERENCE';

export interface TransformationOption {
  value: DataTransformation;
  /** Label for the selector. */
  label: string;
  /** Suffix for the chart title and the value column; empty for NONE. */
  description: string;
  /**
   * Unit of the transformed values: `'%'` for a percent change, `''` for a dimensionless
   * one, null where the values keep the series' own units.
   */
  units: string | null;
  /** Y-axis title; null where it is the series' own units. */
  axisTitle: string | null;
}

export const TRANSFORMATION_OPTIONS: readonly TransformationOption[] = [
  { value: 'NONE', label: 'None', description: '', units: null, axisTitle: null },
  {
    value: 'YEAR_OVER_YEAR',
    label: 'Year-over-Year',
    description: 'Year-over-Year % Change',
    units: '%',
    axisTitle: 'Percent Change',
  },
  {
    value: 'QUARTER_OVER_QUARTER',
    label: 'Quarter-over-Quarter',
    description: 'Quarter-over-Quarter % Change',
    units: '%',
    axisTitle: 'Percent Change',
  },
  {
    value: 'MONTH_OVER_MONTH',
    label: 'Month-over-Month',
    description: 'Month-over-Month % Change',
    units: '%',
    axisTitle: 'Percent Change',
  },
  {
    value: 'PERCENT_CHANGE',
    label: 'Change since first available value',
    description: '% Change since First Available Value',
    units: '%',
    axisTitle: 'Percent Change',
  },
  {
    // ln(current) - ln(previous); null for the first point and wherever either value is not
    // positive. A dimensionless log change, so no unit.
    value: 'LOG_DIFFERENCE',
    label: 'Log difference',
    description: 'Log Difference',
    units: '',
    axisTitle: 'Log Difference',
  },
];

/**
 * Describe a transformation for titles and labels.
 * @param transformation - The transformation applied to the series.
 * @returns A human-readable description, or an empty string for NONE.
 */
export function describeTransformation(transformation: DataTransformation): string {
  return TRANSFORMATION_OPTIONS.find(o => o.value === transformation)?.description ?? '';
}

/**
 * The unit of a series' values after a transformation.
 * @param transformation - The transformation applied to the series.
 * @param seriesUnits - The series' own units, used when the transformation keeps them.
 * @returns The unit to show next to a value; empty for a dimensionless transformation.
 */
export function transformedUnits(transformation: DataTransformation, seriesUnits: string): string {
  return TRANSFORMATION_OPTIONS.find(o => o.value === transformation)?.units ?? seriesUnits;
}

/**
 * The y-axis title for a series' values after a transformation.
 * @param transformation - The transformation applied to the series.
 * @param seriesUnits - The series' own units, used when the transformation keeps them.
 * @returns The axis title.
 */
export function transformedAxisTitle(
  transformation: DataTransformation,
  seriesUnits: string
): string {
  return TRANSFORMATION_OPTIONS.find(o => o.value === transformation)?.axisTitle ?? seriesUnits;
}

/** Rough length in days of a series' period, by the prefix of its source's frequency text. */
const FREQUENCY_DAYS: readonly (readonly [string, number])[] = [
  ['daily', 1],
  ['weekly', 7],
  ['biweekly', 14],
  ['semi-annual', 182],
  ['semiannual', 182],
  ['semi annual', 182],
  ['monthly', 30],
  ['quarterly', 91],
  ['annual', 365],
  ['yearly', 365],
];

/** How far back each period-over-period transformation compares, in days. */
const COMPARISON_DAYS: Partial<Record<DataTransformation, number>> = {
  MONTH_OVER_MONTH: 30,
  QUARTER_OVER_QUARTER: 91,
  YEAR_OVER_YEAR: 365,
};

/**
 * Comparisons a frequency can't make although its period is short enough: a biweekly series'
 * points fall 84 or 98 days back, both outside the backend's 85 to 95 day quarter-over-quarter
 * window (QOQ_WINDOW in the GraphQL crate's query.rs).
 */
const UNSUPPORTED: Readonly<Record<string, readonly DataTransformation[]>> = {
  biweekly: ['QUARTER_OVER_QUARTER'],
};

/**
 * The transformations that make sense for a series observed at `frequency`. A
 * period-over-period change shorter than the series' own period has nothing to compare (a
 * quarterly series has no value a month earlier), so it isn't offered. The prefixes match the
 * backend's `SeriesFrequency::classify_raw`; an unrecognised frequency gets every option.
 * @param frequency - The series' frequency as its source states it, e.g. `Weekly, Ending Friday`.
 * @returns The options to offer, in display order.
 */
export function transformationsFor(frequency: string): readonly TransformationOption[] {
  const lower = frequency.trim().toLowerCase();
  const match = FREQUENCY_DAYS.find(([prefix]) => lower.startsWith(prefix));
  if (match === undefined) return TRANSFORMATION_OPTIONS;
  const [prefix, periodDays] = match;
  const unsupported = UNSUPPORTED[prefix] ?? [];
  return TRANSFORMATION_OPTIONS.filter(
    o => (COMPARISON_DAYS[o.value] ?? Infinity) >= periodDays && !unsupported.includes(o.value)
  );
}

/**
 * Format an observation for display after a transformation. Log differences are small
 * (0.01188 for a 1.2% rise), so they show four significant digits; everything else shows two
 * decimals with grouped thousands.
 * @param value - The value to format.
 * @param transformation - The transformation the value carries.
 * @returns E.g. `0.01188`, `5.30` or `31,702.10`.
 */
export function formatTransformedValue(value: number, transformation: DataTransformation): string {
  return transformation === 'LOG_DIFFERENCE'
    ? value.toLocaleString('en-US', { minimumSignificantDigits: 4, maximumSignificantDigits: 4 })
    : value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}
