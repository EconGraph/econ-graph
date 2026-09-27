/**
 * Series transformations the backend computes (the GraphQL `DataTransformation` enum).
 *
 * The browser never transforms values itself: it sends one of these to `seriesData`
 * and plots what comes back.
 */

export type DataTransformation =
  'NONE' | 'YEAR_OVER_YEAR' | 'QUARTER_OVER_QUARTER' | 'MONTH_OVER_MONTH' | 'PERCENT_CHANGE';

export interface TransformationOption {
  value: DataTransformation;
  /** Label for the selector. */
  label: string;
  /** Suffix for the chart title and the value column; empty for NONE. */
  description: string;
}

// LOG_DIFFERENCE exists in the schema but the backend returns ratio - 1 rather than a
// logarithm, so it is not offered until the backend computes it correctly.
export const TRANSFORMATION_OPTIONS: readonly TransformationOption[] = [
  { value: 'NONE', label: 'None', description: '' },
  {
    value: 'YEAR_OVER_YEAR',
    label: 'Year-over-Year',
    description: 'Year-over-Year % Change',
  },
  {
    value: 'QUARTER_OVER_QUARTER',
    label: 'Quarter-over-Quarter',
    description: 'Quarter-over-Quarter % Change',
  },
  {
    value: 'MONTH_OVER_MONTH',
    label: 'Month-over-Month',
    description: 'Month-over-Month % Change',
  },
  {
    value: 'PERCENT_CHANGE',
    label: 'Change since first observation',
    description: '% Change since First Observation',
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
