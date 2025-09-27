// REQUIREMENT: Mock data for frontend component testing
// PURPOSE: Provide realistic test data that matches the GraphQL schema
// This ensures components receive properly structured data during testing

import { SeriesSearchResult } from '../../hooks/useSeriesData';

/**
 * Mock economic series data for testing
 * REQUIREMENT: Realistic series data matching backend GraphQL schema
 */
export const mockSeriesData = [
  {
    id: 'test-series-1',
    title: 'Real Gross Domestic Product',
    description: 'Inflation-adjusted measure of the value of all goods and services produced',
    externalId: 'GDPC1',
    source: {
      id: 'fred',
      name: 'Federal Reserve Economic Data',
    },
    frequency: 'Quarterly',
    units: 'Billions of Chained 2017 Dollars',
    startDate: '1947-01-01',
    endDate: '2023-12-31',
    lastUpdated: '2024-01-15T10:30:00Z',
    isActive: true,
    seasonalAdjustment: 'Seasonally Adjusted Annual Rate',
  },
  {
    id: 'test-series-2',
    title: 'Unemployment Rate',
    description: 'Percent of civilian labor force that is unemployed',
    externalId: 'UNRATE',
    source: {
      id: 'bls',
      name: 'Bureau of Labor Statistics',
    },
    frequency: 'Monthly',
    units: 'Percent',
    startDate: '1948-01-01',
    endDate: '2023-12-31',
    lastUpdated: '2024-01-05T08:15:00Z',
    isActive: true,
    seasonalAdjustment: 'Seasonally Adjusted',
  },
  {
    id: 'test-series-3',
    title: 'Consumer Price Index for All Urban Consumers',
    description: 'Measure of inflation based on consumer goods and services',
    externalId: 'CPIAUCSL',
    source: {
      id: 'bls',
      name: 'Bureau of Labor Statistics',
    },
    frequency: 'Monthly',
    units: 'Index 1982-1984=100',
    startDate: '1947-01-01',
    endDate: '2023-12-31',
    lastUpdated: '2024-01-12T09:00:00Z',
    isActive: true,
    seasonalAdjustment: 'Not Seasonally Adjusted',
  },
];

/**
 * Mock data sources for testing
 * REQUIREMENT: Data source information for dropdown and filtering components
 */
export const mockDataSources = [
  {
    id: 'fred',
    name: 'Federal Reserve Economic Data',
    description: 'Economic time series data from the Federal Reserve Bank of St. Louis',
    baseUrl: 'https://api.stlouisfed.org/fred',
    apiKeyRequired: true,
    rateLimitPerMinute: 120,
    createdAt: '2023-01-01T00:00:00Z',
    updatedAt: '2024-01-01T00:00:00Z',
  },
  {
    id: 'bls',
    name: 'Bureau of Labor Statistics',
    description: 'Labor market and economic statistics from the U.S. Department of Labor',
    baseUrl: 'https://api.bls.gov/publicAPI/v2',
    apiKeyRequired: false,
    rateLimitPerMinute: 500,
    createdAt: '2023-01-01T00:00:00Z',
    updatedAt: '2024-01-01T00:00:00Z',
  },
];

/**
 * Mock search results for full-text search testing
 * REQUIREMENT: Search results with ranking and similarity scores
 */
export const mockSearchResults: SeriesSearchResult[] = [
  {
    id: 'search-result-1',
    title: 'Real Gross Domestic Product',
    description: 'Inflation-adjusted GDP in billions of chained 2017 dollars',
    externalId: 'GDPC1',
    sourceId: 'fred',
    frequency: 'Quarterly',
    units: 'Billions of Chained 2017 Dollars',
    startDate: '1947-01-01',
    endDate: '2023-12-31',
    lastUpdated: '2024-01-15T10:30:00Z',
    isActive: true,
    rank: 0.95,
    similarityScore: 0.0,
  },
  {
    id: 'search-result-2',
    title: 'Nominal Gross Domestic Product',
    description: 'Current-dollar GDP without inflation adjustment',
    externalId: 'GDP',
    sourceId: 'fred',
    frequency: 'Quarterly',
    units: 'Billions of Dollars',
    startDate: '1947-01-01',
    endDate: '2023-12-31',
    lastUpdated: '2024-01-15T10:30:00Z',
    isActive: true,
    rank: 0.88,
    similarityScore: 0.0,
  },
  {
    id: 'search-result-3',
    title: 'Unemployment Rate',
    description: 'Percent of civilian labor force that is unemployed',
    externalId: 'UNRATE',
    sourceId: 'bls',
    frequency: 'Monthly',
    units: 'Percent',
    startDate: '1948-01-01',
    endDate: '2023-12-31',
    lastUpdated: '2024-01-05T08:15:00Z',
    isActive: true,
    rank: 0.82,
    similarityScore: 0.3, // Fuzzy match example
  },
];

/**
 * Mock search suggestions for autocomplete testing
 * REQUIREMENT: Search suggestions with different types and confidence scores
 */
export const mockSuggestions: string[] = [
  'Gross Domestic Product',
  'GDP',
  'Employment',
  'Unemployment Rate',
  'Consumer Price Index',
  'Interest Rate',
  'Federal Funds Rate',
  'Inflation Rate',
];
/**
 * Mock data points for chart testing
 * REQUIREMENT: Time series data points with various scenarios
 */
export const mockDataPoints = [
  {
    date: '2024-01-01',
    value: 100.5,
    revisionDate: '2024-01-15',
    isOriginalRelease: true,
  },
  {
    date: '2024-02-01',
    value: 101.2,
    revisionDate: '2024-02-15',
    isOriginalRelease: true,
  },
  {
    date: '2024-03-01',
    value: 102.8,
    revisionDate: '2024-03-15',
    isOriginalRelease: false,
  },
  {
    date: '2024-04-01',
    value: null, // Missing data point
    revisionDate: '2024-04-15',
    isOriginalRelease: true,
  },
  {
    date: '2024-05-01',
    value: 103.1,
    revisionDate: '2024-05-15',
    isOriginalRelease: true,
  },
];

/**
 * Mock transformed data for transformation testing
 * REQUIREMENT: Data transformation examples (YoY, QoQ, MoM)
 */
export const mockTransformedData = {
  YEAR_OVER_YEAR: [
    {
      date: '2024-01-01',
      value: 2.5, // 2.5% year-over-year growth
      revisionDate: '2024-01-15',
      isOriginalRelease: true,
    },
    {
      date: '2024-02-01',
      value: 2.8,
      revisionDate: '2024-02-15',
      isOriginalRelease: true,
    },
  ],
  QUARTER_OVER_QUARTER: [
    {
      date: '2024-01-01',
      value: 0.6, // 0.6% quarter-over-quarter growth
      revisionDate: '2024-01-15',
      isOriginalRelease: true,
    },
    {
      date: '2024-04-01',
      value: 0.8,
      revisionDate: '2024-04-15',
      isOriginalRelease: true,
    },
  ],
  MONTH_OVER_MONTH: [
    {
      date: '2024-01-01',
      value: 0.2, // 0.2% month-over-month growth
      revisionDate: '2024-01-15',
      isOriginalRelease: true,
    },
    {
      date: '2024-02-01',
      value: 0.3,
      revisionDate: '2024-02-15',
      isOriginalRelease: true,
    },
  ],
};

/**
 * Mock error responses for error handling testing
 * REQUIREMENT: Error scenarios for robust component testing
 */
export const mockErrors = {
  NETWORK_ERROR: {
    message: 'Network error: Failed to fetch',
    code: 'NETWORK_ERROR',
  },
  GRAPHQL_ERROR: {
    message: 'GraphQL error: Series not found',
    code: 'SERIES_NOT_FOUND',
    path: ['series'],
  },
  VALIDATION_ERROR: {
    message: 'Validation error: Invalid date range',
    code: 'INVALID_INPUT',
    details: {
      field: 'dateRange',
      constraint: 'start_date must be before end_date',
    },
  },
};

/**
 * Helper function to create mock data with variations
 * REQUIREMENT: Utility for generating test data with different scenarios
 */
export function createMockSeries(overrides: Partial<(typeof mockSeriesData)[0]> = {}) {
  return {
    ...mockSeriesData[0],
    ...overrides,
    id: overrides.id || `mock-series-${Date.now()}`,
  };
}

export function createMockDataPoints(count: number, baseValue = 100) {
  return Array.from({ length: count }, (_, index) => ({
    date: new Date(2024, 0, index + 1).toISOString().split('T')[0],
    value: baseValue + Math.random() * 10 - 5, // ±5 variation
    revisionDate: new Date(2024, 0, index + 15).toISOString().split('T')[0],
    isOriginalRelease: index % 3 === 0,
  }));
}

export function createMockSearchResults(query: string, count = 5) {
  return Array.from({ length: count }, (_, index) => ({
    ...mockSearchResults[0],
    id: `search-${query}-${index}`,
    title: `${query} Series ${index + 1}`,
    rank: 1 - index * 0.1, // Decreasing relevance
    similarityScore: index > 2 ? 0.3 : 0.0, // Some fuzzy matches
  }));
}
