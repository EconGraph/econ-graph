/**
 * The series shown as cards on the dashboard.
 *
 * The list lives in `dashboard-series.json` so it can change without touching code. Each
 * entry names a series by its data source's name (exactly as the backend's `dataSources`
 * query returns it) and the source's own id for the series.
 */

import rawDashboardSeries from './dashboard-series.json';

export interface DashboardSeriesEntry {
  /** Stable key for the card. */
  id: string;
  /** Card heading, shown even when the series is not loaded yet. */
  label: string;
  /** `data_sources.name` of the series' source, for example "Federal Reserve Economic Data (FRED)". */
  sourceName: string;
  /** Short source name shown on the card. */
  sourceLabel: string;
  /** The source's id for the series, for example "GDP". */
  externalId: string;
  /** Digits after the decimal point when the value is displayed. */
  fractionDigits: number;
}

/**
 * Check that a parsed dashboard series file has the expected shape.
 * @param value - The parsed JSON.
 * @returns The entries, typed.
 * @throws {Error} Naming the first entry or field that is wrong.
 */
export function parseDashboardSeries(value: unknown): DashboardSeriesEntry[] {
  if (!Array.isArray(value)) {
    throw new Error('dashboard-series.json must be an array');
  }
  const seen = new Set<string>();
  return value.map((entry, index) => {
    if (typeof entry !== 'object' || entry === null) {
      throw new Error(`dashboard-series.json entry ${index} must be an object`);
    }
    const record = entry as Record<string, unknown>;
    for (const field of ['id', 'label', 'sourceName', 'sourceLabel', 'externalId']) {
      if (typeof record[field] !== 'string' || record[field] === '') {
        throw new Error(`dashboard-series.json entry ${index} needs a non-empty "${field}"`);
      }
    }
    const { fractionDigits } = record;
    if (
      typeof fractionDigits !== 'number' ||
      !Number.isInteger(fractionDigits) ||
      fractionDigits < 0 ||
      fractionDigits > 10
    ) {
      throw new Error(
        `dashboard-series.json entry ${index} needs "fractionDigits" as an integer from 0 to 10`
      );
    }
    const id = record.id as string;
    if (seen.has(id)) {
      throw new Error(`dashboard-series.json has "${id}" more than once`);
    }
    seen.add(id);
    return {
      id,
      label: record.label as string,
      sourceName: record.sourceName as string,
      sourceLabel: record.sourceLabel as string,
      externalId: record.externalId as string,
      fractionDigits,
    };
  });
}

export const DASHBOARD_SERIES: DashboardSeriesEntry[] = parseDashboardSeries(rawDashboardSeries);
