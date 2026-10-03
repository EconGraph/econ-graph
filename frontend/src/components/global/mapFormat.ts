/**
 * How the world map shows values, dates and countries without data.
 */

import { formatIsoDate } from '../../utils/dates';
import { formatDecimalString } from '../../utils/formatDecimal';

/** Fill of a country without a value. */
export const NO_DATA_FILL = '#d0d0d0';

/**
 * Format a value for display: grouped thousands, at most two decimals, no trailing zeros.
 * @param value - The decimal as the API returns it, e.g. `"82769.4"`.
 * @returns E.g. `82,769.4`; the input unchanged when it isn't a decimal.
 */
export function formatMapValue(value: string): string {
  const formatted = formatDecimalString(value, 2);
  if (formatted === null) return value;
  return formatted.includes('.') ? formatted.replace(/\.?0+$/, '') : formatted;
}

/**
 * Format a value's date for its series' frequency. Annual, quarterly and monthly values are
 * dated on the first day of their period, so they show the period rather than that day.
 * @param date - `YYYY-MM-DD`.
 * @param frequency - The series' frequency, e.g. `Annual`; null when unknown.
 * @returns E.g. `2023`, `2024 Q2`, `Mar 2024` or `Mar 15, 2024`.
 */
export function formatMapDate(date: string, frequency: string | null): string {
  switch (frequency?.toLowerCase()) {
    case 'annual':
      return date.slice(0, 4);
    case 'quarterly':
      return `${date.slice(0, 4)} Q${Math.floor((Number(date.slice(5, 7)) - 1) / 3) + 1}`;
    case 'monthly':
      return formatIsoDate(date, { year: 'numeric', month: 'short' });
    default:
      return formatIsoDate(date);
  }
}
