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
