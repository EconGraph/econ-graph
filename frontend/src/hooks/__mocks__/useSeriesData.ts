/**
 * Mock implementation of useSeriesData hooks for testing
 * PURPOSE: Provide predictable test data and behavior for components
 */

import { vi } from 'vitest';

// Mock data for testing
export const mockDataSources = [
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
];

export const mockSearchResults = [
  {
    id: 'gdp-series-1',
    title: 'Real Gross Domestic Product',
    description: 'Real GDP in billions of chained 2017 dollars',
    sourceId: 'fred',
    frequency: 'Quarterly',
    units: 'Billions of Chained 2017 Dollars',
    lastUpdated: '2024-12-01T00:00:00Z',
    startDate: '1947-01-01T00:00:00Z',
    endDate: '2024-10-01T00:00:00Z',
    similarityScore: 0.95,
  },
  {
    id: 'unemployment-series-1',
    title: 'Unemployment Rate',
    description: 'Unemployment rate as a percentage of the labor force',
    sourceId: 'fred',
    frequency: 'Monthly',
    units: 'Percent',
    lastUpdated: '2024-11-01T00:00:00Z',
    startDate: '1948-01-01T00:00:00Z',
    endDate: '2024-11-01T00:00:00Z',
    similarityScore: 0.9,
  },
];

// Mock hook implementations
export const useDataSources = vi.fn(() => ({
  data: mockDataSources,
  isLoading: false,
  error: null,
  isError: false,
  isSuccess: true,
}));

export const useSeriesSearch = vi.fn(() => ({
  data: mockSearchResults,
  isLoading: false,
  error: null,
  isError: false,
  isSuccess: true,
}));

export const useSeriesDetail = vi.fn(() => ({
  data: mockSearchResults[0],
  isLoading: false,
  error: null,
  isError: false,
  isSuccess: true,
}));

export const useSeriesData = vi.fn(() => ({
  data: {
    series: mockSearchResults[0],
    dataPoints: [
      { date: '2024-10-01', value: 27360.0 },
      { date: '2024-07-01', value: 27280.5 },
      { date: '2024-04-01', value: 27180.2 },
    ],
  },
  isLoading: false,
  error: null,
  isError: false,
  isSuccess: true,
}));

export const useSearchSuggestions = vi.fn(() => ({
  data: ['GDP', 'unemployment', 'inflation', 'federal funds rate'],
  isLoading: false,
  error: null,
  isError: false,
  isSuccess: true,
}));

export const useCrawlerStatus = vi.fn(() => ({
  data: {
    status: 'running',
    lastRun: '2024-12-15T10:00:00Z',
    nextRun: '2024-12-15T11:00:00Z',
    sources: [
      { id: 'fred', status: 'active', lastUpdate: '2024-12-15T10:00:00Z' },
      { id: 'bls', status: 'active', lastUpdate: '2024-12-14T09:00:00Z' },
    ],
  },
  isLoading: false,
  error: null,
  isError: false,
  isSuccess: true,
}));
