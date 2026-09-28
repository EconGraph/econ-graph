/**
 * Public annotations on a series, read from the GraphQL API.
 */

import { useQuery } from '@tanstack/react-query';
import {
  AnnotationsForSeriesResponse,
  ChartAnnotationType,
  executeGraphQL,
  QUERIES,
} from '../utils/graphql';
import { isSeriesId } from './useSeriesData';

/** One annotation as the series page shows it. */
export interface SeriesAnnotation {
  id: string;
  /** Observation date the note is about, `YYYY-MM-DD`. */
  date: string;
  title: string;
  description: string | null;
  /** Hex colour for the chart and list; left out when the stored one isn't a hex colour. */
  color?: string;
}

const HEX_COLOR = /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i;

function toSeriesAnnotation(raw: ChartAnnotationType): SeriesAnnotation {
  return {
    id: raw.id,
    date: raw.annotationDate,
    title: raw.title,
    description: raw.description,
    // The column is free text (up to 7 characters); only a hex colour reaches the chart.
    color: raw.color && HEX_COLOR.test(raw.color) ? raw.color : undefined,
  };
}

/**
 * Load a series' public annotations, newest date first.
 *
 * The request carries no user, so the backend returns public annotations only. Signed-in
 * views (private annotations, editing, comments) build on this in the auth area.
 * @param seriesId - Series id (UUID); anything else loads nothing.
 * @param options - `enabled` defers the request, for example until the series has loaded.
 * @param options.enabled - Whether to send the request.
 * @returns The react-query result.
 */
export function useSeriesAnnotations(seriesId: string, { enabled = true } = {}) {
  return useQuery(
    ['seriesAnnotations', seriesId],
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
