/**
 * Date helpers for ISO calendar dates (`YYYY-MM-DD`) returned by the API.
 *
 * `new Date('2024-01-01')` is UTC midnight, which is the previous day in any timezone west
 * of UTC. Observation dates are calendar dates, so they are read as local dates instead.
 */

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
