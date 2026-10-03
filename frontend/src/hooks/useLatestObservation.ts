/**
 * Look up a series by its source and external id, with its newest observation.
 */

import { useQuery } from '@tanstack/react-query';
import { executeGraphQL } from '../utils/graphql';

export const GET_SERIES_LATEST_OBSERVATION = `
  query GetSeriesLatestObservation($sourceName: String!, $externalId: String!) {
    seriesByExternalId(sourceName: $sourceName, externalId: $externalId) {
      id
      units
      latestObservation {
        date
        value
        revisionDate
      }
    }
  }
`;

export interface LatestObservation {
  /** Observation date, `YYYY-MM-DD`. */
  date: string;
  /** Decimal as a string; null when the source published the date without a number. */
  value: string | null;
  /** Publication date of the revision the value comes from, `YYYY-MM-DD`. */
  revisionDate: string;
}

export interface SeriesWithLatestObservation {
  id: string;
  units: string | null;
  /** Null when the series has no data points. */
  latestObservation: LatestObservation | null;
}

interface GetSeriesLatestObservationData {
  seriesByExternalId: SeriesWithLatestObservation | null;
}

/**
 * Fetch a series by source name and external id, with its latest observation.
 * @param sourceName - The source's `data_sources.name`, for example "Federal Reserve Economic Data (FRED)".
 * @param externalId - The source's id for the series, for example "GDP".
 * @returns The react-query result; `data` is null when no such series exists.
 */
export const useLatestObservation = (sourceName: string, externalId: string) => {
  return useQuery(
    ['seriesLatestObservation', sourceName, externalId],
    async (): Promise<SeriesWithLatestObservation | null> => {
      const result = await executeGraphQL<GetSeriesLatestObservationData>({
        query: GET_SERIES_LATEST_OBSERVATION,
        variables: { sourceName, externalId },
        operationName: 'GetSeriesLatestObservation',
      });
      return result.data?.seriesByExternalId ?? null;
    },
    {
      staleTime: 5 * 60 * 1000, // 5 minutes
    }
  );
};
