/**
 * Pure helpers behind `SeriesChart`: which points are shown, in what order, and how
 * annotations become `chartjs-plugin-annotation` options. Kept free of React so other
 * features (the CSV download) can reuse exactly what the chart shows.
 */

import { color as parseColor } from 'chart.js/helpers';
import type { AnnotationOptions } from 'chartjs-plugin-annotation';
import { parseIsoDate } from '../../utils/dates';

/** One observation to plot, already transformed by the backend. */
export interface SeriesChartPoint {
  /** Observation date, `YYYY-MM-DD`. */
  date: string;
  value: number | null;
  /** Revision date of the value, `YYYY-MM-DD`, when known. */
  revisionDate?: string;
}

/** Inclusive date range of local-midnight dates; a null or invalid bound is open. */
export interface SeriesChartDateRange {
  start: Date | null;
  end: Date | null;
}

interface AnnotationBase {
  /** Id, unique within one chart's annotations; passed back to `onAnnotationClick`. */
  id: string;
  /** Short text drawn on the chart. */
  label: string;
  /** CSS colour (hex, rgb() or a name); defaults to the theme's warning colour. */
  color?: string;
}

/** A note on one observation date, drawn as a vertical line. */
export interface SeriesChartPointAnnotation extends AnnotationBase {
  kind: 'point';
  /** Observation date, `YYYY-MM-DD`. */
  date: string;
}

/** A note on a span of dates, drawn as a shaded box covering both end days. */
export interface SeriesChartRangeAnnotation extends AnnotationBase {
  kind: 'range';
  /** First date, `YYYY-MM-DD`, inclusive. */
  startDate: string;
  /** Last date, `YYYY-MM-DD`, inclusive; not before `startDate`. */
  endDate: string;
}

export type SeriesChartAnnotation = SeriesChartPointAnnotation | SeriesChartRangeAnnotation;

export const EMPTY_DATE_RANGE: SeriesChartDateRange = { start: null, end: null };

/**
 * A range bound as a timestamp.
 * @param bound - The bound.
 * @returns Its time, or undefined when it is missing or invalid (a half-typed date).
 */
export function boundTime(bound: Date | null): number | undefined {
  const time = bound?.getTime() ?? NaN;
  return Number.isNaN(time) ? undefined : time;
}

/**
 * The start of the day after an ISO date, so an inclusive end date covers its whole day.
 * @param iso - A `YYYY-MM-DD` date.
 * @returns Local midnight of the next day, as a timestamp (NaN for a bad date).
 */
function endOfDay(iso: string): number {
  const day = parseIsoDate(iso);
  return new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1).getTime();
}

/**
 * A see-through version of a colour for range shading.
 * @param cssColor - Any CSS colour.
 * @returns The colour at 15% opacity, or transparent when it can't be parsed.
 */
function translucent(cssColor: string): string {
  const parsed = parseColor(cssColor);
  return parsed.valid ? parsed.alpha(0.15).rgbString() : 'transparent';
}

/**
 * The points the chart shows: those inside the range, sorted by date.
 * @param points - All points of the series, in any order.
 * @param range - Inclusive date range; null or invalid bounds are open.
 * @returns The shown points, oldest first.
 */
export function selectShownPoints(
  points: readonly SeriesChartPoint[],
  range: SeriesChartDateRange = EMPTY_DATE_RANGE
): SeriesChartPoint[] {
  const start = boundTime(range.start) ?? -Infinity;
  const end = boundTime(range.end) ?? Infinity;
  return points
    .map(point => ({ point, time: parseIsoDate(point.date).getTime() }))
    .filter(({ time }) => !Number.isNaN(time) && time >= start && time <= end)
    .sort((a, b) => a.time - b.time)
    .map(({ point }) => point);
}

/**
 * The time span an annotation covers.
 * @param annotation - The annotation.
 * @returns `[from, to)` as timestamps, or undefined when a date is bad or the range is
 * reversed.
 */
function annotationSpan(annotation: SeriesChartAnnotation): [number, number] | undefined {
  const [from, to] =
    annotation.kind === 'point'
      ? [parseIsoDate(annotation.date).getTime(), endOfDay(annotation.date)]
      : [parseIsoDate(annotation.startDate).getTime(), endOfDay(annotation.endDate)];
  if (Number.isNaN(from) || Number.isNaN(to) || to <= from) return undefined;
  return [from, to];
}

/**
 * Whether an annotation overlaps the date range.
 * @param annotation - The annotation.
 * @param range - Inclusive date range; null or invalid bounds are open.
 * @returns True when any of its days falls inside the range; false for a bad annotation.
 */
export function annotationInRange(
  annotation: SeriesChartAnnotation,
  range: SeriesChartDateRange = EMPTY_DATE_RANGE
): boolean {
  const span = annotationSpan(annotation);
  return span !== undefined && spanInRange(span, range);
}

/**
 * Whether a `[from, to)` span overlaps an inclusive date range.
 * @param span - The span as timestamps.
 * @param range - Inclusive date range; null or invalid bounds are open.
 * @returns True when they overlap.
 */
function spanInRange(span: [number, number], range: SeriesChartDateRange): boolean {
  return (
    span[0] <= (boundTime(range.end) ?? Infinity) && span[1] > (boundTime(range.start) ?? -Infinity)
  );
}

/**
 * The dates the time axis shows: the chosen bounds, with open ones filled from the data.
 * @param range - The chosen range; null or invalid bounds are open.
 * @param shownPoints - The shown points, oldest first (from `selectShownPoints`).
 * @returns The visible range; bounds stay open only when there are no points.
 */
export function visibleRange(
  range: SeriesChartDateRange,
  shownPoints: readonly SeriesChartPoint[]
): SeriesChartDateRange {
  const first = shownPoints[0];
  const last = shownPoints[shownPoints.length - 1];
  return {
    start:
      boundTime(range.start) !== undefined ? range.start : first ? parseIsoDate(first.date) : null,
    end: boundTime(range.end) !== undefined ? range.end : last ? parseIsoDate(last.date) : null,
  };
}

/**
 * Turn annotations into `chartjs-plugin-annotation` options keyed by annotation id.
 * Points become vertical lines and ranges become shaded boxes on the time axis. They
 * never widen the axis, and boxes are cut to the range so their labels stay on screen.
 * @param annotations - The annotations to draw; ids must be unique.
 * @param range - The visible range (see `visibleRange`); only annotations overlapping it
 * are drawn.
 * @param defaultColor - Colour for annotations without one.
 * @param onClick - Called with an annotation's id, and the plugin's click event, when it is
 * clicked.
 * @returns Plugin options for `plugins.annotation.annotations`.
 */
export function buildAnnotationOptions(
  annotations: readonly SeriesChartAnnotation[],
  range: SeriesChartDateRange,
  defaultColor: string,
  onClick?: (id: string, event: unknown) => void
): Record<string, AnnotationOptions> {
  const options: Record<string, AnnotationOptions> = {};
  for (const annotation of annotations) {
    const span = annotationSpan(annotation);
    if (!span || !spanInRange(span, range)) continue;
    const color = annotation.color ?? defaultColor;
    const shared = {
      adjustScaleRange: false,
      borderColor: color,
      label: { display: true, content: annotation.label, position: 'start' as const },
      click: onClick
        ? (_context: unknown, event: unknown) => onClick(annotation.id, event)
        : undefined,
    };
    if (annotation.kind === 'point') {
      options[annotation.id] = {
        ...shared,
        type: 'line',
        xMin: span[0],
        xMax: span[0],
        borderWidth: 2,
        borderDash: [4, 4],
        hitTolerance: 4,
      };
      continue;
    }
    const xMin = Math.max(span[0], boundTime(range.start) ?? -Infinity);
    const xMax = Math.min(span[1], boundTime(range.end) ?? Infinity);
    // A range that only starts on the axis's last day would draw as a zero-width box.
    if (xMax <= xMin) continue;
    options[annotation.id] = {
      ...shared,
      type: 'box',
      xMin,
      xMax,
      backgroundColor: translucent(color),
      borderWidth: 1,
      drawTime: 'beforeDatasetsDraw',
    };
  }
  return options;
}
