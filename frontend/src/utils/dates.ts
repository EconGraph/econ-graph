/**
 * Date helpers for ISO calendar dates (`YYYY-MM-DD`) returned by the API.
 *
 * `new Date('2024-01-01')` is UTC midnight, which is the previous day in any timezone west
 * of UTC. Observation dates are calendar dates, so they are read as local dates instead.
 */

import { classifyFrequency } from './frequency';

/**
 * Parse an ISO calendar date as local midnight.
 * @param iso - A `YYYY-MM-DD` date, optionally followed by a time part (ignored).
 * @returns The local date, or an invalid Date when the string isn't a date.
 */
export function parseIsoDate(iso: string): Date {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
  if (!match) return new Date(NaN);
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
}

/**
 * Format an ISO calendar date for display.
 * @param iso - A `YYYY-MM-DD` date.
 * @param options - Intl options; defaults to e.g. "Jan 1, 2024".
 * @returns The formatted date.
 */
export function formatIsoDate(
  iso: string,
  options: Intl.DateTimeFormatOptions = { year: 'numeric', month: 'short', day: 'numeric' }
): string {
  return parseIsoDate(iso).toLocaleDateString('en-US', options);
}

/**
 * Format an observation's date as the period it stands for: `2026 Q2` for a quarterly series,
 * `2024 H1` for a semi-annual one and `2022` for an annual one, rather than the period's first
 * day (`Apr 1, 2026`). Other frequencies show the calendar date.
 * @param iso - The observation's `YYYY-MM-DD` date.
 * @param frequency - The series' frequency as its source states it.
 * @param options - Intl options for a calendar date; see `formatIsoDate`.
 * @returns The formatted date.
 */
export function formatObservationDate(
  iso: string,
  frequency: string,
  options?: Intl.DateTimeFormatOptions
): string {
  const date = parseIsoDate(iso);
  if (Number.isNaN(date.getTime())) return formatIsoDate(iso, options);
  const year = date.getFullYear();
  switch (classifyFrequency(frequency)?.periodDays) {
    case 365:
      return String(year);
    case 182:
      return `${year} H${date.getMonth() < 6 ? 1 : 2}`;
    case 91:
      return `${year} Q${Math.floor(date.getMonth() / 3) + 1}`;
    default:
      return formatIsoDate(iso, options);
  }
}
