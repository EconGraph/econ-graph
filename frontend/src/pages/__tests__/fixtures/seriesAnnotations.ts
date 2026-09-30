/**
 * GraphQL responses for the series page's annotation tests, in the shape the backend
 * returns them (camelCase fields, dates as `YYYY-MM-DD`).
 */

export const GDP_ID = '0b9f5a3e-6c1d-4e2a-9f0b-7a1c2d3e4f50';

export const gdpDetail = {
  series: {
    id: GDP_ID,
    externalId: 'GDPC1',
    title: 'Real Gross Domestic Product',
    description: 'Inflation-adjusted value of goods and services produced in the US.',
    source: { name: 'Federal Reserve Economic Data' },
    frequency: 'Quarterly',
    units: 'Billions of Chained 2017 Dollars',
    seasonalAdjustment: 'Seasonally Adjusted Annual Rate',
    startDate: '2019-10-01',
    endDate: '2020-07-01',
    lastUpdated: '2020-10-29T12:00:00Z',
    isActive: true,
  },
};

export const gdpLevels = {
  seriesData: {
    nodes: [
      { date: '2019-10-01', value: '19202.3', revisionDate: '2020-01-30', isOriginalRelease: true },
      { date: '2020-01-01', value: '18951.9', revisionDate: '2020-04-29', isOriginalRelease: true },
      { date: '2020-04-01', value: '17258.2', revisionDate: '2020-07-30', isOriginalRelease: true },
      { date: '2020-07-01', value: '18560.8', revisionDate: '2020-10-29', isOriginalRelease: true },
    ],
    totalCount: 4,
  },
};

/** Signed-in user who authors `gdpAnnotations`' first and last annotations. */
export const ALICE = 'b7c1e2d3-aaaa-4f00-8000-00000000a11c';
/** Another signed-in user. */
export const BOB = 'b7c1e2d3-bbbb-4f00-8000-00000000b0b0';

/** Public annotations, in the backend's order (newest created first). */
export const gdpAnnotations = {
  annotationsForSeries: [
    {
      id: 'a1d6c0e2-1111-4c8e-9a51-0d3f2b7e8c01',
      userId: ALICE,
      annotationDate: '2020-04-01',
      title: 'Lockdown trough',
      description: 'Largest quarterly fall on record.',
      color: '#d32f2f',
      visibility: 'PUBLIC',
    },
    {
      id: 'a1d6c0e2-2222-4c8e-9a51-0d3f2b7e8c02',
      userId: BOB,
      annotationDate: '2020-07-01',
      title: 'Reopening rebound',
      description: null,
      color: 'not-a-colour',
      visibility: 'PUBLIC',
    },
    {
      id: 'a1d6c0e2-3333-4c8e-9a51-0d3f2b7e8c03',
      userId: ALICE,
      annotationDate: '2020-01-01',
      title: 'Pre-pandemic peak',
      description: null,
      color: null,
      visibility: 'PUBLIC',
    },
  ],
};

/**
 * What the backend returns to ALICE: the public annotations plus her own private one. Anyone
 * else (BOB, or a signed-out visitor) gets `gdpAnnotations`.
 */
export const gdpAnnotationsForAlice = {
  annotationsForSeries: [
    {
      id: 'a1d6c0e2-4444-4c8e-9a51-0d3f2b7e8c04',
      userId: ALICE,
      annotationDate: '2019-10-01',
      title: 'My draft note',
      description: 'Check the revision.',
      color: null,
      visibility: 'PRIVATE',
    },
    ...gdpAnnotations.annotationsForSeries,
  ],
};

/** Comments on the lockdown annotation, in the backend's shape and order (oldest first). */
export const lockdownComments = {
  commentsForAnnotation: [
    {
      id: 'c0de0000-0001-4000-8000-000000000001',
      annotationId: 'a1d6c0e2-1111-4c8e-9a51-0d3f2b7e8c01',
      userId: BOB,
      content: 'Services drove most of it.',
      createdAt: '2020-08-01T09:00:00Z',
    },
    {
      id: 'c0de0000-0002-4000-8000-000000000002',
      annotationId: 'a1d6c0e2-1111-4c8e-9a51-0d3f2b7e8c01',
      userId: ALICE,
      content: 'Revised down in the annual update.',
      createdAt: '2020-08-02T09:00:00Z',
    },
  ],
};
