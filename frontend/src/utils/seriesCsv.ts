/**
 * CSV export of the points a series chart shows, built in the browser.
 *
 * The file opens with one `label,value` row per piece of metadata (title, source, units,
 * transformation, retrieval time), then a `date,value` header and one row per point, so
 * every row has two fields. Lines end in CRLF (RFC 4180).
 */

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
  /** Points to write, in the order given. */
  points: readonly { date: string; value: number | null }[];
}

// Spreadsheets run a cell starting with one of these as a formula (CSV injection).
const FORMULA_PREFIX = /^[=+\-@\t\r]/;

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
  const lines = meta.map(([label, value]) => `${label},${csvTextField(value)}`);
  lines.push('date,value');
  for (const point of input.points) {
    // Keep the calendar date only, should the API ever add a time part.
    const date = csvField(point.date.slice(0, 10));
    // String() gives the shortest form that round-trips to the same number; spreadsheets
    // read its exponent notation too.
    lines.push(`${date},${point.value === null ? '' : String(point.value)}`);
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
