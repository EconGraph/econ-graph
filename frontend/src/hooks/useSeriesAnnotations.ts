/**
 * Annotations on a series, read from the GraphQL API: public ones, plus the signed-in
 * user's own private ones.
 */

import { useQuery } from '@tanstack/react-query';
import {
  AnnotationsForSeriesResponse,
  AnnotationVisibility,
  ChartAnnotationType,
  executeGraphQL,
  QUERIES,
} from '../utils/graphql';
import { isSeriesId } from './useSeriesData';

/** One annotation as the series page shows it. */
export interface SeriesAnnotation {
  id: string;
  /** Author's user id, the identity provider's `sub` (compare with `useAuth().user.id`). */
  authorId: string;
  /** Observation date the note is about, `YYYY-MM-DD`. */
  date: string;
  title: string;
  description: string | null;
  /** Hex colour for the chart and list; left out when the stored one isn't a hex colour. */
  color?: string;
  visibility: AnnotationVisibility;
}

const HEX_COLOR = /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i;

function toSeriesAnnotation(raw: ChartAnnotationType): SeriesAnnotation {
  return {
    id: raw.id,
    authorId: raw.userId,
    date: raw.annotationDate,
    title: raw.title,
    description: raw.description,
    // The column is free text (up to 7 characters); only a hex colour reaches the chart.
    color: raw.color && HEX_COLOR.test(raw.color) ? raw.color : undefined,
    visibility: raw.visibility,
  };
}

/**
 * React-query key of a series' annotations; the annotation mutations invalidate it.
 * @param seriesId - Series id.
 * @returns The key.
 */
export const seriesAnnotationsKey = (seriesId: string) => ['seriesAnnotations', seriesId] as const;

/**
 * Load a series' annotations, newest date first.
 *
 * Signed out, the backend returns public annotations only; signed in, the request carries
 * the user's token and the backend adds their own private ones. The query cache resets when
 * the signed-in user changes, so one user's private annotations never show for another.
 * @param seriesId - Series id (UUID); anything else loads nothing.
 * @param options - `enabled` defers the request, for example until the series has loaded.
 * @param options.enabled - Whether to send the request.
 * @returns The react-query result.
 */
export function useSeriesAnnotations(seriesId: string, { enabled = true } = {}) {
  return useQuery(
    seriesAnnotationsKey(seriesId),
    async (): Promise<SeriesAnnotation[]> => {
      if (!isSeriesId(seriesId)) return [];

      const result = await executeGraphQL<AnnotationsForSeriesResponse>({
        query: QUERIES.GET_ANNOTATIONS_FOR_SERIES,
        variables: { seriesId },
      });
      return (result.data?.annotationsForSeries ?? [])
        .map(toSeriesAnnotation)
        .sort((a, b) => b.date.localeCompare(a.date));
    },
    {
      enabled: enabled && !!seriesId,
      staleTime: 5 * 60 * 1000, // 5 minutes
    }
  );
}
