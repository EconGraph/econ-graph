/**
 * Series transformations the backend computes (the GraphQL `DataTransformation` enum).
 *
 * The browser never transforms values itself: it sends one of these to `seriesData`
 * and plots what comes back.
 */

import { classifyFrequency } from './frequency';

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
  const match = classifyFrequency(frequency);
  if (match === undefined) return TRANSFORMATION_OPTIONS;
  const { prefix, periodDays } = match;
  const unsupported = UNSUPPORTED[prefix] ?? [];
  return TRANSFORMATION_OPTIONS.filter(
    o => (COMPARISON_DAYS[o.value] ?? Infinity) >= periodDays && !unsupported.includes(o.value)
  );
}

/** Most decimals a level is shown with; a value given more finely is rounded to this. */
const MAX_LEVEL_FRACTION_DIGITS = 2;

/**
 * The decimals a series' levels need: as many as its most precise value carries, up to two.
 * Counts such as establishments or persons need none, so they don't show `.00`.
 * @param values - The series' levels; nulls are skipped.
 * @returns 0, 1 or 2.
 */
export function levelFractionDigits(values: readonly (number | null)[]): number {
  let digits = 0;
  for (const value of values) {
    if (value === null || !Number.isFinite(value)) continue;
    while (digits < MAX_LEVEL_FRACTION_DIGITS && Number(value.toFixed(digits)) !== value) {
      digits++;
    }
    if (digits === MAX_LEVEL_FRACTION_DIGITS) break;
  }
  return digits;
}

/**
 * Format an observation for display after a transformation. Log differences are small
 * (0.01188 for a 1.2% rise), so they show four significant digits; percent changes show two
 * decimals, and levels the decimals their series carries (`levelFractionDigits`). All group
 * thousands.
 * @param value - The value to format.
 * @param transformation - The transformation the value carries.
 * @param levelDigits - Decimals for a level (NONE); defaults to two.
 * @returns E.g. `0.01188`, `5.30`, `31,702.10` or `7,324,017`.
 */
export function formatTransformedValue(
  value: number,
  transformation: DataTransformation,
  levelDigits = MAX_LEVEL_FRACTION_DIGITS
): string {
  if (transformation === 'LOG_DIFFERENCE') {
    return value.toLocaleString('en-US', {
      minimumSignificantDigits: 4,
      maximumSignificantDigits: 4,
    });
  }
  const digits = transformation === 'NONE' ? levelDigits : 2;
  return value.toLocaleString('en-US', {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
}

const TICK_SCALES: readonly (readonly [number, string])[] = [
  [1e12, 'T'],
  [1e9, 'B'],
  [1e6, 'M'],
];

/**
 * A y-axis tick for large values, scaled to the axis' largest tick: `28.6T` rather than
 * `28,600,000,000,000`. It keeps as many decimals as the step between ticks needs, so
 * neighbouring ticks don't read the same.
 * @param value - The tick's value.
 * @param ticks - Every tick value on the axis.
 * @returns The label, or undefined when no tick reaches a million (Chart.js' own label reads
 *   fine there).
 */
export function scaledTickLabel(value: number, ticks: readonly number[]): string | undefined {
  const largest = Math.max(...ticks.map(Math.abs));
  const scale = TICK_SCALES.find(([size]) => largest >= size);
  if (scale === undefined) return undefined;
  const [size, suffix] = scale;
  const sorted = [...ticks].sort((a, b) => a - b);
  let step = Infinity;
  for (let i = 1; i < sorted.length; i++) {
    const gap = sorted[i] - sorted[i - 1];
    if (gap > 0) step = Math.min(step, gap);
  }
  const scaledStep = step / size;
  // 1e-9 keeps a step of exactly 0.1 at one decimal despite floating point.
  const digits =
    Number.isFinite(scaledStep) && scaledStep < 1
      ? Math.min(20, Math.ceil(-Math.log10(scaledStep) - 1e-9))
      : 0;
  const label = (value / size).toLocaleString('en-US', {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
  return `${label}${suffix}`;
}
