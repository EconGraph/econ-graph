/**
 * Length limits on what a user writes into an annotation or comment, in characters. The
 * backend enforces the same numbers (ANNOTATION_TITLE_MAX_CHARS, ANNOTATION_NOTE_MAX_CHARS and
 * COMMENT_MAX_CHARS in backend/crates/econ-graph-services/src/services/collaboration_service.rs);
 * annotationLimits.test.ts fails if the two drift apart.
 */
export const ANNOTATION_TITLE_MAX_CHARS = 200;
export const ANNOTATION_NOTE_MAX_CHARS = 5_000;
export const COMMENT_MAX_CHARS = 2_000;

/** Longest annotation label drawn on the chart; longer titles are cut with an ellipsis. */
export const CHART_LABEL_MAX_CHARS = 40;

/**
 * Lets user-written text break anywhere, so one long word or URL wraps inside its box instead
 * of widening the page (ECO-402).
 */
export const wrapAnywhere = { overflowWrap: 'anywhere', wordBreak: 'break-word' } as const;

/**
 * `text` cut to at most `max` characters, ending in an ellipsis when it was cut.
 * @param text - The text to shorten.
 * @param max - The most characters to keep, ellipsis included.
 * @returns The shortened text.
 */
export function truncateLabel(text: string, max: number = CHART_LABEL_MAX_CHARS): string {
  const chars = Array.from(text);
  return chars.length <= max ? text : `${chars.slice(0, max - 1).join('')}…`;
}

/**
 * How many characters `text` has, counted as the backend counts them (Unicode code points), so
 * an emoji counts once rather than as its two UTF-16 units.
 * @param text - The text to count.
 * @returns Its length in characters.
 */
export function charCount(text: string): number {
  return Array.from(text).length;
}
