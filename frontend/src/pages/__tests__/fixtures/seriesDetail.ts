/**
 * GraphQL responses for the series page tests, in the shape the backend returns them
 * (`series(id)` and `seriesData(...)`; BigDecimal values arrive as strings).
 */

export const UNRATE_ID = '0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b';
export const EMPTY_ID = '0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5c';
export const UNKNOWN_ID = '0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5d';
export const GDP_Q_ID = '0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5e';
export const ESTAB_ID = '0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5f';

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
  // Quarterly, in whole dollars (BEA applies UNIT_MULT 6), dated at each quarter's start.
  [GDP_Q_ID]: {
    series: {
      ...unrate,
      id: GDP_Q_ID,
      externalId: 'bea_nipa/T10105.1.Q',
      title: 'Gross domestic product (quarterly)',
      source: { name: 'Bureau of Economic Analysis (BEA)' },
      frequency: 'Quarterly',
      units: 'Current Dollars',
      startDate: '2024-01-01',
      endDate: '2024-10-01',
    },
  },
  // Annual counts, dated January 1.
  [ESTAB_ID]: {
    series: {
      ...unrate,
      id: ESTAB_ID,
      externalId: 'ESTAB',
      title: 'Establishments',
      frequency: 'Annual',
      units: 'Establishments',
      startDate: '2019-01-01',
      endDate: '2022-01-01',
    },
  },
};

const node = (date: string, value: string | null, isOriginalRelease = false) => ({
  date,
  value,
  revisionDate: '2024-04-05',
  isOriginalRelease,
});

type Node = ReturnType<typeof node>;

/** A `seriesData` page, as the backend shapes it. */
export const page = (
  nodes: Node[],
  totalCount: number,
  {
    hasNextPage = false,
    endCursor = null,
  }: { hasNextPage?: boolean; endCursor?: string | null } = {}
) => ({ seriesData: { nodes, totalCount, pageInfo: { hasNextPage, endCursor } } });

/** Levels (transformation NONE). */
export const unrateLevels = page(
  [node('2024-01-01', '3.7'), node('2024-02-01', '3.9'), node('2024-03-01', '3.8', true)],
  3
);

/** Quarterly GDP levels in dollars: whole numbers of 14 digits. */
export const gdpQuarterlyLevels = page(
  [node('2024-07-01', '28511400000000'), node('2024-10-01', '28624702000000', true)],
  2
);

/** Annual establishment counts: whole numbers. */
export const estabLevels = page(
  [node('2019-01-01', '7106316'), node('2022-01-01', '7324017', true)],
  2
);

/** Year-over-year: the backend returns null where no prior-year point exists. */
export const unrateYearOverYear = page(
  [node('2024-01-01', null), node('2024-02-01', '5.4054'), node('2024-03-01', '-2.5641', true)],
  3
);

/** Log difference: null for the first point, then ln(current) - ln(previous). */
export const unrateLogDifference = page(
  [node('2024-01-01', null), node('2024-02-01', '0.052644'), node('2024-03-01', '-0.025975', true)],
  3
);

/** Month-over-month on a series too short to compute it. */
export const allNullTransformed = page([node('2024-01-01', null)], 1);

export const noObservations = page([], 0);

/**
 * Seven daily points served as three pages of at most DAILY_PAGE_SIZE, the way the backend
 * pages a long series: `endCursor` is the count of points read so far and is passed back
 * as `after`; the first page is keyed 'first'.
 */
export const DAILY_PAGE_SIZE = 3;
export const dailyPoints = [
  node('2024-01-02', '100'),
  node('2024-01-03', '101'),
  node('2024-01-04', '102'),
  node('2024-01-05', '103'),
  node('2024-01-08', '104'),
  node('2024-01-09', '105'),
  node('2024-01-10', '106', true),
];
export const dailyPages: Record<string, ReturnType<typeof page>> = {};
for (let offset = 0; offset < dailyPoints.length; offset += DAILY_PAGE_SIZE) {
  const end = Math.min(offset + DAILY_PAGE_SIZE, dailyPoints.length);
  const hasNextPage = end < dailyPoints.length;
  dailyPages[offset === 0 ? 'first' : String(offset)] = page(
    dailyPoints.slice(offset, end),
    dailyPoints.length,
    { hasNextPage, endCursor: hasNextPage ? String(end) : null }
  );
}
