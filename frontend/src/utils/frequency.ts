/**
 * Series frequencies as sources state them ("Quarterly", "Weekly, Ending Friday", "Yearly").
 */

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

/**
 * Classify a frequency by its prefix, the way the backend's `SeriesFrequency::classify_raw`
 * does.
 * @param frequency - The series' frequency as its source states it.
 * @returns The matched prefix and the period's rough length in days, or undefined for a
 *   frequency it doesn't recognise.
 */
export function classifyFrequency(
  frequency: string
): { prefix: string; periodDays: number } | undefined {
  const lower = frequency.trim().toLowerCase();
  const match = FREQUENCY_DAYS.find(([prefix]) => lower.startsWith(prefix));
  return match === undefined ? undefined : { prefix: match[0], periodDays: match[1] };
}
