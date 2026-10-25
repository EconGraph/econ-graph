/**
 * Mock implementation of useSeriesData hooks for testing
 * PURPOSE: Provide predictable test data and behavior for components.
 */

import { vi } from 'vitest';

import type { DataSource } from '../useSeriesData';

// Mock data for testing
export const mockDataSources: DataSource[] = [
  {
    id: 'fred',
    name: 'Federal Reserve Economic Data',
    description: 'Economic data from the Federal Reserve',
    baseUrl: 'https://fred.stlouisfed.org',
    apiKeyRequired: true,
    rateLimitPerMinute: 120,
    seriesCount: 800000,
    createdAt: '2024-01-01T00:00:00Z',
    updatedAt: '2024-12-15T10:00:00Z',
  },
  {
    id: 'bls',
    name: 'Bureau of Labor Statistics',
    description: 'Labor market and economic statistics',
    baseUrl: 'https://www.bls.gov',
    apiKeyRequired: false,
    rateLimitPerMinute: 500,
    seriesCount: 50000,
    createdAt: '2024-01-01T00:00:00Z',
    updatedAt: '2024-12-14T09:00:00Z',
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
  isPending: false,
  error: null,
  isError: false,
  isSuccess: true,
}));

export const useSeriesSearch = vi.fn(() => ({
  data: mockSearchResults,
  isLoading: false,
  isPending: false,
  error: null,
  isError: false,
  isSuccess: true,
}));

export const useSeriesBrowse = vi.fn(() => ({
  data: undefined,
  isPending: true,
  isLoading: false,
  isFetching: false,
  error: null,
}));

export const useSeriesDetail = vi.fn(() => ({
  data: {
    id: mockSearchResults[0].id,
    externalId: 'GDPC1',
    title: mockSearchResults[0].title,
    description: mockSearchResults[0].description,
    source: { name: 'FRED' },
    frequency: mockSearchResults[0].frequency,
    units: mockSearchResults[0].units,
    seasonalAdjustment: null,
    startDate: '1947-01-01',
    endDate: '2024-10-01',
    lastUpdated: mockSearchResults[0].lastUpdated,
    isActive: true,
  },
  isLoading: false,
  isPending: false,
  error: null,
  isError: false,
  isSuccess: true,
}));

export const useSeriesData = vi.fn(() => ({
  data: {
    transformation: 'NONE',
    points: [
      { date: '2024-04-01', value: 27180.2, revisionDate: '2024-10-30', isOriginalRelease: false },
      { date: '2024-07-01', value: 27280.5, revisionDate: '2024-10-30', isOriginalRelease: false },
      { date: '2024-10-01', value: 27360.0, revisionDate: '2024-10-30', isOriginalRelease: true },
    ],
  },
  isLoading: false,
  isPending: false,
  error: null,
  isError: false,
  isSuccess: true,
}));

export const useSearchSuggestions = vi.fn(() => ({
  data: ['GDP', 'unemployment', 'inflation', 'federal funds rate'],
  isLoading: false,
  isPending: false,
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
  isPending: false,
  error: null,
  isError: false,
  isSuccess: true,
}));
