/**
 * CSV and JSON export of the series a search lists, built in the browser.
 *
 * Each row names the series' source and the source's own series id (`NY.GDP.PCAP.CD`, `GDP`),
 * so an exported row can be matched back to the source, and carries our id as `id`. Both
 * formats have the same fields in the same order, and a field the backend didn't send is empty
 * (null in JSON) rather than left out. CSV protects every text column, including IDs,
 * with a leading apostrophe when it could start a spreadsheet formula. JSON preserves
 * the exact source IDs; use it for programmatic matching.
 */
import { csvTextField } from './seriesCsv';

/** One listed series as it goes into an export. */
export interface SearchResultsExportRow {
  /** Source name, such as "World Bank"; missing when the source isn't known. */
  source?: string;
  /** The source's own series id. */
  externalId?: string;
  title: string;
  description?: string;
  frequency?: string;
  units?: string;
  startDate?: string;
  endDate?: string;
  lastUpdated?: string;
  /** EconGraph's id for the series. */
  id: string;
}

/** The exported fields, in file order; also the CSV header. */
export const SEARCH_RESULTS_EXPORT_COLUMNS = [
  'source',
  'externalId',
  'title',
  'description',
  'frequency',
  'units',
  'startDate',
  'endDate',
  'lastUpdated',
  'id',
] as const satisfies readonly (keyof SearchResultsExportRow)[];

/**
 * Build the CSV for listed series: a header, then one row per series. Lines end in CRLF
 * (RFC 4180), and text is escaped so a spreadsheet never runs a cell as a formula.
 * @param rows - The series, in the order listed.
 * @returns The file contents.
 */
export function buildSearchResultsCsv(rows: readonly SearchResultsExportRow[]): string {
  const lines = [SEARCH_RESULTS_EXPORT_COLUMNS.join(',')];
  for (const row of rows) {
    lines.push(
      SEARCH_RESULTS_EXPORT_COLUMNS.map(column => csvTextField(row[column] ?? '')).join(',')
    );
  }
  return `${lines.join('\r\n')}\r\n`;
}

/**
 * Build the JSON for listed series: an array of objects with every exported field.
 * @param rows - The series, in the order listed.
 * @returns The file contents, indented.
 */
export function buildSearchResultsJson(rows: readonly SearchResultsExportRow[]): string {
  const objects = rows.map(row =>
    Object.fromEntries(SEARCH_RESULTS_EXPORT_COLUMNS.map(column => [column, row[column] ?? null]))
  );
  return JSON.stringify(objects, null, 2);
}
