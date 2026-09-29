/**
 * GraphQL responses for the series page tests, in the shape the backend returns them
 * (`series(id)` and `seriesData(...)`; BigDecimal values arrive as strings).
 */

export const UNRATE_ID = '0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b';
export const EMPTY_ID = '0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5c';
export const UNKNOWN_ID = '0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5d';

const unrate = {
  id: UNRATE_ID,
  externalId: 'UNRATE',
  title: 'Unemployment Rate',
  description: 'The number of unemployed as a percentage of the labor force.',
  source: { name: 'FRED' },
  frequency: 'Monthly',
  units: 'Percent',
  seasonalAdjustment: 'Seasonally Adjusted',
  startDate: '1948-01-01',
  endDate: '2024-03-01',
  lastUpdated: '2024-04-05T12:31:00Z',
  isActive: true,
};

export const seriesResponses: Record<string, { series: typeof unrate | null }> = {
  [UNRATE_ID]: { series: unrate },
  [EMPTY_ID]: {
    series: {
      ...unrate,
      id: EMPTY_ID,
      externalId: 'NEWSERIES',
      title: 'A Series Not Crawled Yet',
      description: null as unknown as string,
      startDate: null as unknown as string,
      endDate: null as unknown as string,
      lastUpdated: null as unknown as string,
    },
  },
  [UNKNOWN_ID]: { series: null },
};

const node = (date: string, value: string | null, isOriginalRelease = false) => ({
  date,
  value,
  revisionDate: '2024-04-05',
  isOriginalRelease,
});

/** Levels (transformation NONE). */
export const unrateLevels = {
  seriesData: {
    nodes: [node('2024-01-01', '3.7'), node('2024-02-01', '3.9'), node('2024-03-01', '3.8', true)],
    totalCount: 3,
  },
};

/** Year-over-year: the backend returns null where no prior-year point exists. */
export const unrateYearOverYear = {
  seriesData: {
    nodes: [
      node('2024-01-01', null),
      node('2024-02-01', '5.4054'),
      node('2024-03-01', '-2.5641', true),
    ],
    totalCount: 3,
  },
};

/** Month-over-month on a series too short to compute it. */
export const allNullTransformed = {
  seriesData: {
    nodes: [node('2024-01-01', null)],
    totalCount: 1,
  },
};

export const noObservations = { seriesData: { nodes: [], totalCount: 0 } };
