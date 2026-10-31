/**
 * Wording shown with a series' source notice. The notice text itself comes from the API
 * (`DataSourceType.notice`); this adds the parts that depend on what is shown.
 */

/**
 * The statement that shown values are EconGraph's transformation, not the source's figures.
 * Sources' terms forbid presenting modified data as theirs, and CC BY requires indicating
 * changes.
 * @param transformationLabel - Such as "Year-over-Year % Change"; empty for levels.
 * @param sourceName - The source's name; empty when unknown.
 * @returns The statement, or null when the values are untransformed.
 */
export function derivedValuesNote(transformationLabel: string, sourceName: string): string | null {
  if (!transformationLabel) return null;
  const source = sourceName || 'the source';
  return `Values show the ${transformationLabel}, computed by EconGraph from data published by ${source}; they are not figures published by ${source}.`;
}

/**
 * The calendar date a series was last retrieved from its source, for notices that require it.
 * @param lastUpdated - The series' `lastUpdated` timestamp; null when unknown.
 * @returns Such as "2026-10-30" (UTC), or null when unknown or unparseable.
 */
export function retrievalDate(lastUpdated: string | null): string | null {
  if (!lastUpdated) return null;
  const date = new Date(lastUpdated);
  return Number.isNaN(date.getTime()) ? null : date.toISOString().slice(0, 10);
}
