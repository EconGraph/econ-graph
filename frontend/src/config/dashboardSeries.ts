/**
 * The series shown as cards on the dashboard.
 *
 * The list lives in `dashboard-series.json` so it can change without touching code. Each
 * entry names a series by its data source's name (exactly as the backend's `dataSources`
 * query returns it) and the source's own id for the series. The cards are read anonymously,
 * so every entry must come from a source the anonymous reader can read (`catalog-public` in
 * config/keycloak/econ-graph-realm.json).
 *
 * An entry may set `displayScale` to show a value stored in base units (BEA stores dollars,
 * not millions of dollars) in thousands, millions, billions or trillions, and `units` to label
 * the value as shown. Set `units` whenever `displayScale` is set: the series' own units
 * describe the stored value, not the scaled one. `displayId` names the series on the card's
 * chip when its external id isn't readable (BEA's `bea_nipa/T10105.1.Q`).
 */

import rawDashboardSeries from './dashboard-series.json';

/** Powers of ten a card can divide a value by, by name. */
export const DISPLAY_SCALES = {
  thousands: 3,
  millions: 6,
  billions: 9,
  trillions: 12,
} as const;

export type DisplayScale = keyof typeof DISPLAY_SCALES;

export interface DashboardSeriesEntry {
  /** Stable key for the card. */
  id: string;
  /** Card heading, shown even when the series is not loaded yet. */
  label: string;
  /** `data_sources.name` of the series' source, for example "Bureau of Economic Analysis (BEA)". */
  sourceName: string;
  /** Short source name shown on the card. */
  sourceLabel: string;
  /** The source's id for the series, for example "bea_nipa/T10105.1.Q". */
  externalId: string;
  /** Digits after the decimal point when the value is displayed, after any `displayScale`. */
  fractionDigits: number;
  /** Show the value divided into this scale; absent shows it as stored. */
  displayScale?: DisplayScale;
  /** Units shown under the value, in place of the series' own units. */
  units?: string;
  /**
   * How the card's chip names the series, for example "NIPA Table 1.1.5, line 1", when the
   * external id is not readable as is; absent shows the external id. Display only: the card
   * still looks the series up by `externalId`.
   */
  displayId?: string;
}

/**
 * The chip text that names an entry's series: its source and its readable id.
 * @param entry - The dashboard entry.
 * @returns For example "BEA · NIPA Table 1.1.5, line 1" or "BLS · LNS14000000".
 */
export function chipLabel(entry: DashboardSeriesEntry): string {
  return `${entry.sourceLabel} · ${entry.displayId ?? entry.externalId}`;
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
    const { displayScale, units } = record;
    if (
      displayScale !== undefined &&
      !(
        typeof displayScale === 'string' &&
        Object.prototype.hasOwnProperty.call(DISPLAY_SCALES, displayScale)
      )
    ) {
      throw new Error(
        `dashboard-series.json entry ${index} has "displayScale" other than ${Object.keys(
          DISPLAY_SCALES
        ).join(', ')}`
      );
    }
    if (units !== undefined && (typeof units !== 'string' || units.trim() === '')) {
      throw new Error(`dashboard-series.json entry ${index} has an empty "units"`);
    }
    if (displayScale !== undefined && units === undefined) {
      throw new Error(
        `dashboard-series.json entry ${index} sets "displayScale" without "units" for the scaled value`
      );
    }
    const { displayId } = record;
    if (displayId !== undefined && (typeof displayId !== 'string' || displayId.trim() === '')) {
      throw new Error(`dashboard-series.json entry ${index} has an empty "displayId"`);
    }
    seen.add(id);
    return {
      id,
      label: record.label as string,
      sourceName: record.sourceName as string,
      sourceLabel: record.sourceLabel as string,
      externalId: record.externalId as string,
      fractionDigits,
      ...(displayScale !== undefined && { displayScale: displayScale as DisplayScale }),
      ...(units !== undefined && { units: units as string }),
      ...(displayId !== undefined && { displayId: displayId as string }),
    };
  });
}

export const DASHBOARD_SERIES: DashboardSeriesEntry[] = parseDashboardSeries(rawDashboardSeries);
