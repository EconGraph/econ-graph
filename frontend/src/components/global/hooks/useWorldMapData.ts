/**
 * Data for the world map: the World Development Indicators dataset, its indicator codes, and one
 * indicator's latest value per country from the `crossSection` query.
 */

import { useQuery } from '@tanstack/react-query';
import { executeGraphQL, type GraphQLResponse } from '../../../utils/graphql';

/** Code of the World Bank's World Development Indicators dataset. */
export const MAP_DATASET_CODE = 'wdi';

/** Shared code list whose values are the reference file's area keys (ISO alpha-3). */
export const COUNTRIES_CODELIST = 'countries';

/** Indicator the map shows first, when the dataset has it. */
export const DEFAULT_INDICATOR = 'NY.GDP.PCAP.CD';

export const GET_MAP_DATASETS = `
  query GetMapDatasets {
    datasets {
      id
      code
      name
      dimensions {
        name
        label
        codelist
        codes {
          code
          label
          unit
        }
      }
    }
  }
`;

export const GET_MAP_CROSS_SECTION = `
  query GetMapCrossSection(
    $datasetId: ID!
    $filter: [DimensionFilterInput!]!
    $across: String!
  ) {
    crossSection(datasetId: $datasetId, filter: $filter, across: $across, latest: true) {
      key
      seriesId
      date
      value
      area {
        name
        iso3
        isoNumeric
        kind
      }
    }
  }
`;

export const GET_MAP_SERIES_META = `
  query GetMapSeriesMeta($id: ID!) {
    series(id: $id) {
      id
      units
      frequency
    }
  }
`;

/** A coded value of a dimension. */
export interface MapDimensionCode {
  code: string;
  label: string;
  unit: string | null;
}

/** A dataset dimension with its codes. */
export interface MapDimension {
  name: string;
  label: string;
  codelist: string | null;
  codes: MapDimensionCode[];
}

/** The dataset the map reads, with the dimension it maps across and the one it picks from. */
export interface MapDataset {
  id: string;
  name: string;
  /** The country dimension, recognised by its `countries` code list. */
  areaDimension: string;
  /** The other dimension, whose codes the indicator picker offers. */
  indicatorDimension: string;
  indicators: MapDimensionCode[];
}

/** One country's latest value of the selected indicator. */
export interface MapCountryValue {
  /** The area key, ISO alpha-3. */
  key: string;
  name: string;
  /** ISO 3166-1 numeric code, which world-atlas uses as feature id; null for Kosovo. */
  isoNumeric: number | null;
  seriesId: string;
  /** Date of the value, `YYYY-MM-DD`. */
  date: string;
  /** The decimal as the API returns it, a string, so display keeps its exact digits. */
  value: string;
  /** `value` as a number, for the color scale. */
  numericValue: number;
}

/** The selected indicator across countries. */
export interface MapCrossSection {
  /** The indicator code these values are for. */
  indicator: string;
  /** Countries with a value; aggregates and countries with none are left out. */
  values: MapCountryValue[];
  /** Unit of the values: the indicator code's unit, else the series' own `units`. */
  unit: string | null;
  /** The series' frequency, e.g. `Annual`; decides how dates are shown. */
  frequency: string | null;
}

interface DatasetsData {
  datasets: Array<{
    id: string;
    code: string;
    name: string;
    dimensions: MapDimension[];
  }>;
}

interface CrossSectionData {
  crossSection: Array<{
    key: string;
    seriesId: string;
    date: string | null;
    value: string | null;
    area: {
      name: string;
      iso3: string | null;
      isoNumeric: number | null;
      kind: 'COUNTRY' | 'AGGREGATE';
    } | null;
  }>;
}

interface SeriesMetaData {
  series: { id: string; units: string | null; frequency: string } | null;
}

/**
 * Throw the first GraphQL error, so react-query reports it.
 * @param result - The GraphQL response.
 * @returns Its data.
 */
function dataOrThrow<T>(result: GraphQLResponse<T>): T {
  if (result.errors?.length) {
    throw new Error(result.errors[0].message);
  }
  if (!result.data) {
    throw new Error('The server returned no data');
  }
  return result.data;
}

/**
 * Pick the map's dataset out of the dataset list.
 * @param datasets - Every dataset.
 * @returns The WDI dataset with its area and indicator dimensions, or null when it isn't loaded
 * or doesn't have exactly those two dimensions.
 */
export function pickMapDataset(datasets: DatasetsData['datasets']): MapDataset | null {
  const dataset = datasets.find(d => d.code === MAP_DATASET_CODE);
  if (!dataset) return null;
  const area = dataset.dimensions.find(d => d.codelist === COUNTRIES_CODELIST);
  const others = dataset.dimensions.filter(d => d !== area);
  if (!area || others.length !== 1) return null;
  return {
    id: dataset.id,
    name: dataset.name,
    areaDimension: area.name,
    indicatorDimension: others[0].name,
    indicators: others[0].codes,
  };
}

/**
 * Load the map's dataset and its indicator codes.
 * @returns The react-query result; `data` is null when the dataset isn't loaded.
 */
export const useMapDataset = () =>
  useQuery(
    ['worldMap', 'dataset'],
    async ({ signal }) => {
      const result = await executeGraphQL<DatasetsData>(
        { query: GET_MAP_DATASETS, operationName: 'GetMapDatasets' },
        signal
      );
      return pickMapDataset(dataOrThrow(result).datasets);
    },
    { staleTime: 30 * 60 * 1000 }
  );

/**
 * Keep the entries the map can show: countries with a value.
 * @param entries - The cross-section, every key.
 * @returns The countries with a value, aggregates and missing values left out.
 */
export function toCountryValues(entries: CrossSectionData['crossSection']): MapCountryValue[] {
  const values: MapCountryValue[] = [];
  for (const entry of entries) {
    if (entry.area?.kind !== 'COUNTRY' || entry.value === null || entry.date === null) continue;
    const numericValue = Number(entry.value);
    if (!Number.isFinite(numericValue)) continue;
    values.push({
      key: entry.key,
      name: entry.area.name,
      isoNumeric: entry.area.isoNumeric,
      seriesId: entry.seriesId,
      date: entry.date,
      value: entry.value,
      numericValue,
    });
  }
  return values;
}

/**
 * Load the latest value of one indicator for every country.
 * @param dataset - The map's dataset; nothing loads until it is known.
 * @param indicator - The indicator code; nothing loads until one is picked.
 * @returns The react-query result.
 */
export const useMapCrossSection = (dataset: MapDataset | null | undefined, indicator: string) =>
  useQuery(
    ['worldMap', 'crossSection', dataset?.id, indicator],
    async ({ signal }): Promise<MapCrossSection> => {
      if (!dataset) throw new Error('No dataset');
      const result = await executeGraphQL<CrossSectionData>(
        {
          query: GET_MAP_CROSS_SECTION,
          operationName: 'GetMapCrossSection',
          variables: {
            datasetId: dataset.id,
            filter: [{ dimension: dataset.indicatorDimension, value: indicator }],
            across: dataset.areaDimension,
          },
        },
        signal
      );
      const values = toCountryValues(dataOrThrow(result).crossSection);

      // Every series of one indicator shares its unit and frequency, so one series answers for
      // all. The code's unit wins when the dataset declares one.
      const codeUnit = dataset.indicators.find(c => c.code === indicator)?.unit ?? null;
      let unit = codeUnit;
      let frequency: string | null = null;
      if (values.length > 0) {
        try {
          const meta = await executeGraphQL<SeriesMetaData>(
            {
              query: GET_MAP_SERIES_META,
              operationName: 'GetMapSeriesMeta',
              variables: { id: values[0].seriesId },
            },
            signal
          );
          const series = dataOrThrow(meta).series;
          unit = codeUnit ?? series?.units ?? null;
          frequency = series?.frequency ?? null;
        } catch (error) {
          // The values stand without a unit and with full dates; a cancelled query still stops.
          if (signal?.aborted) throw error;
        }
      }
      return { indicator, values, unit, frequency };
    },
    {
      enabled: Boolean(dataset && indicator),
      staleTime: 5 * 60 * 1000,
      // Keep showing the last indicator while the next loads; the result says which it is.
      keepPreviousData: true,
    }
  );
