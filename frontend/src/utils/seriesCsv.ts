/**
 * CSV export of the points a series chart shows, built in the browser.
 *
 * The file opens with one `label,value` row per piece of metadata (title, source, units,
 * transformation, retrieval time, and the source's required citation and notice), then a
 * `date,value` header and one row per point, so every row has two fields. Lines end in CRLF
 * (RFC 4180).
 */

import type { SourceNotice } from '../hooks/useSeriesData';
import { derivedValuesNote, retrievalDate } from './sourceNotice';

/** What goes into a series CSV. */
export interface SeriesCsvInput {
  title: string;
  /** Source name; empty when unknown. */
  source: string;
  /** Units; empty when unknown. */
  units: string;
  /** Human-readable transformation, such as "Year-over-Year % Change"; empty for levels. */
  transformation: string;
  /** When the values were fetched from the API; null when unknown. */
  retrievedAt: Date | null;
  /** The source's required notice; null or absent for a source without one. */
  notice?: SourceNotice | null;
  /** When EconGraph last retrieved the series from its source (`lastUpdated`); null when unknown. */
  fetchedFromSource?: string | null;
  /**
   * Points to write, in the order given. `exactValue`, the API's decimal string, is written
   * when present so the file keeps every digit; otherwise `value`.
   */
  points: readonly { date: string; value: number | null; exactValue?: string }[];
}

// A plain decimal number, optionally in exponent notation: what the API sends for a value.
const DECIMAL = /^-?\d+(\.\d+)?([eE][+-]?\d+)?$/;

/**
 * The CSV text of one value.
 * @param point - The point.
 * @param point.value - Its number; null for no value.
 * @param point.exactValue - The API's decimal string for it, when known.
 * @returns Its exact decimal string when it has a well-formed one, else the shortest string
 * that round-trips to its number; empty for null.
 */
export function csvValue(point: { value: number | null; exactValue?: string }): string {
  if (point.value === null) return '';
  return point.exactValue !== undefined && DECIMAL.test(point.exactValue)
    ? point.exactValue
    : String(point.value);
}

// Spreadsheets run a cell starting with one of these as a formula (CSV injection).
const FORMULA_PREFIX = /^[=+\-@\t\r\n]/;

/**
 * Quote a field when it holds a comma, quote or line break, doubling inner quotes.
 * @param field - The raw field.
 * @returns The field as it goes into the file.
 */
export function csvField(field: string): string {
  return /[",\r\n]/.test(field) ? `"${field.replace(/"/g, '""')}"` : field;
}

/**
 * Escape free text (series metadata) so a spreadsheet shows it rather than evaluates it.
 * @param text - Text from the source, such as a title.
 * @returns The field, prefixed with `'` when it would start a formula, then quoted if needed.
 */
export function csvTextField(text: string): string {
  return csvField(FORMULA_PREFIX.test(text) ? `'${text}` : text);
}

/**
 * Build the CSV for a series.
 * @param input - Metadata and points; see `SeriesCsvInput`.
 * @returns The file contents. A null value is an empty field.
 */
export function buildSeriesCsv(input: SeriesCsvInput): string {
  const meta: [string, string][] = [
    ['Title', input.title],
    ['Source', input.source],
    ['Units', input.units],
    ['Transformation', input.transformation || 'None'],
    ['Retrieved at', input.retrievedAt?.toISOString() ?? ''],
  ];
  const derived = derivedValuesNote(input.transformation, input.source);
  if (derived) meta.push(['Derived values', derived]);
  const notice = input.notice;
  if (notice) {
    if (notice.citation) meta.push(['Citation', notice.citation]);
    meta.push(['Notice', notice.notice]);
    if (notice.retrievalDateRequired) {
      meta.push(['Retrieved from source', retrievalDate(input.fetchedFromSource ?? null) ?? '']);
    }
    meta.push(['Terms of use', notice.termsUrl]);
  }
  const lines = meta.map(([label, value]) => `${label},${csvTextField(value)}`);
  lines.push('date,value');
  for (const point of input.points) {
    // Keep the calendar date only, should the API ever add a time part.
    const date = csvField(point.date.slice(0, 10));
    // The API's exact decimal when known; otherwise String(), the shortest form that
    // round-trips to the same number. Spreadsheets read exponent notation too.
    lines.push(`${date},${csvValue(point)}`);
  }
  return `${lines.join('\r\n')}\r\n`;
}

/**
 * A file name for a series CSV, safe on every OS.
 * @param seriesKey - Identifier for the series, such as its external id.
 * @param transformation - The transformation code (`NONE` for levels).
 * @returns The name, such as `GDPC1.csv` or `GDPC1-year_over_year.csv`.
 */
export function seriesCsvFileName(seriesKey: string, transformation: string): string {
  const base = seriesKey.replace(/[^A-Za-z0-9._-]+/g, '_').replace(/^[._]+/, '') || 'series';
  return transformation === 'NONE' ? `${base}.csv` : `${base}-${transformation.toLowerCase()}.csv`;
}

// Some browsers start the download after the click handler returns, so the URL must live on.
export const REVOKE_DELAY_MS = 10_000;

/**
 * Save text as a file through a temporary link.
 * @param fileName - Name the browser suggests.
 * @param contents - CSV text.
 */
export function downloadCsv(fileName: string, contents: string): void {
  // The byte-order mark makes Excel read the file as UTF-8.
  const blob = new Blob(['\uFEFF', contents], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = fileName;
  document.body.appendChild(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), REVOKE_DELAY_MS);
}
