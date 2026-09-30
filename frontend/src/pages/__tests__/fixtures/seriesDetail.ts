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
