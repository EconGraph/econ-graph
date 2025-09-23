// jest-dom adds custom jest matchers for asserting on DOM nodes.
// allows you to do things like:
// expect(element).toHaveTextContent(/react/i)
// learn more: https://github.com/testing-library/jest-dom
import '@testing-library/jest-dom';

// Polyfill for Node.js environment
const { TextEncoder, TextDecoder } = require('util');
global.TextEncoder = TextEncoder;
global.TextDecoder = TextDecoder;

// Mock the hooks module globally
jest.mock('./hooks/useSeriesData', () => ({
  useDataSources: jest.fn(() => ({
    data: [
      {
        id: 'fred',
        name: 'Federal Reserve Economic Data',
        description: 'Economic data from the Federal Reserve',
        status: 'active',
        lastUpdated: '2024-12-15T10:00:00Z',
        seriesCount: 800000,
        category: 'Government',
        website: 'https://fred.stlouisfed.org',
      },
      {
        id: 'bls',
        name: 'Bureau of Labor Statistics',
        description: 'Labor market and economic statistics',
        status: 'active',
        lastUpdated: '2024-12-14T09:00:00Z',
        seriesCount: 50000,
        category: 'Government',
        website: 'https://www.bls.gov',
      },
    ],
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSeriesSearch: jest.fn(() => ({
    data: [
      {
        id: 'gdp-series-1',
        title: 'Real Gross Domestic Product',
        description: 'Real GDP in billions of chained 2017 dollars',
        sourceId: 'fred',
        frequency: 'Quarterly',
        units: 'Billions of Chained 2017 Dollars',
        lastUpdated: '2024-10-01T00:00:00Z',
        startDate: '1948-01-01T00:00:00Z',
        endDate: '2024-11-01T00:00:00Z',
        similarityScore: 0.9,
      },
      {
        id: 'unemployment-series-1',
        title: 'Unemployment Rate',
        description: 'Unemployment rate as a percentage',
        sourceId: 'fred',
        frequency: 'Monthly',
        units: 'Percent',
        lastUpdated: '2024-11-01T00:00:00Z',
        startDate: '1948-01-01T00:00:00Z',
        endDate: '2024-11-01T00:00:00Z',
        similarityScore: 0.85,
      },
    ],
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSeriesDetail: jest.fn(() => ({
    data: null,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSeriesData: jest.fn(() => ({
    data: null,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSearchSuggestions: jest.fn(() => ({
    data: [],
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useCrawlerStatus: jest.fn(() => ({
    data: null,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
}));
