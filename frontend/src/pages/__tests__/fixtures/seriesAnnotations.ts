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

/** Public annotations, in the backend's order (newest created first). */
export const gdpAnnotations = {
  annotationsForSeries: [
    {
      id: 'a1d6c0e2-1111-4c8e-9a51-0d3f2b7e8c01',
      annotationDate: '2020-04-01',
      title: 'Lockdown trough',
      description: 'Largest quarterly fall on record.',
      color: '#d32f2f',
    },
    {
      id: 'a1d6c0e2-2222-4c8e-9a51-0d3f2b7e8c02',
      annotationDate: '2020-07-01',
      title: 'Reopening rebound',
      description: null,
      color: 'not-a-colour',
    },
    {
      id: 'a1d6c0e2-3333-4c8e-9a51-0d3f2b7e8c03',
      annotationDate: '2020-01-01',
      title: 'Pre-pandemic peak',
      description: null,
      color: null,
    },
  ],
};
