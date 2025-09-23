// jest-dom adds custom jest matchers for asserting on DOM nodes.
// allows you to do things like:
// expect(element).toHaveTextContent(/react/i)
// learn more: https://github.com/testing-library/jest-dom
import '@testing-library/jest-dom';

// Polyfill for Node.js environment
const { TextEncoder, TextDecoder } = require('util');
global.TextEncoder = TextEncoder;
global.TextDecoder = TextDecoder;

// Mock localStorage
const localStorageMock: Storage = {
  getItem: jest.fn(),
  setItem: jest.fn(),
  removeItem: jest.fn(),
  clear: jest.fn(),
  key: jest.fn(),
  length: 0,
};
(global as any).localStorage = localStorageMock;
Object.defineProperty(window, 'localStorage', {
  value: localStorageMock,
});

// Mock Chart.js and related modules
jest.mock('chart.js', () => ({
  Chart: {
    register: jest.fn(),
  },
  registerables: [],
}));

jest.mock('chartjs-adapter-date-fns', () => ({}));

jest.mock('react-chartjs-2', () => ({
  Line: jest.fn(() => 'Mock Chart'),
  Bar: jest.fn(() => 'Mock Chart'),
  Pie: jest.fn(() => 'Mock Chart'),
}));

// Mock chartjs-plugin-annotation to avoid plugin runtime issues in tests
jest.mock('chartjs-plugin-annotation', () => ({
  __esModule: true,
  default: { id: 'annotation', beforeDraw: jest.fn(), afterDraw: jest.fn() },
}));

// Mock the hooks module globally
const mockDataSources = [
  {
    id: 'fred',
    name: 'Federal Reserve Economic Data',
    description: 'Economic data from the Federal Reserve',
    base_url: 'https://fred.stlouisfed.org',
    api_key_required: false,
    rate_limit_per_minute: 120,
    series_count: 800000,
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-12-15T10:00:00Z',
  },
  {
    id: 'bls',
    name: 'Bureau of Labor Statistics',
    description: 'Labor market and economic statistics',
    base_url: 'https://www.bls.gov',
    api_key_required: true,
    rate_limit_per_minute: 60,
    series_count: 50000,
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-12-14T09:00:00Z',
  },
  {
    id: 'census',
    name: 'U.S. Census Bureau',
    description: 'Demographic and economic data',
    base_url: 'https://www.census.gov',
    api_key_required: false,
    rate_limit_per_minute: 100,
    series_count: 25000,
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-12-13T08:00:00Z',
  },
  {
    id: 'worldbank',
    name: 'World Bank Open Data',
    description: 'Global development indicators',
    base_url: 'https://data.worldbank.org',
    api_key_required: false,
    rate_limit_per_minute: 80,
    series_count: 15000,
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-12-12T07:00:00Z',
  },
];

const mockSearchResults = [
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
];

jest.mock('./hooks/useSeriesData', () => ({
  useDataSources: jest.fn().mockImplementation(() => ({
    data: mockDataSources,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSeriesSearch: jest.fn().mockImplementation(() => ({
    data: mockSearchResults,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSeriesDetail: jest.fn().mockImplementation(() => ({
    data: null,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSeriesData: jest.fn().mockImplementation(() => ({
    data: null,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSearchSuggestions: jest.fn().mockImplementation(() => ({
    data: [],
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useCrawlerStatus: jest.fn().mockImplementation(() => ({
    data: null,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
}));

// Mock D3 modules
jest.mock('d3-geo', () => ({
  geoPath: jest.fn(() => ({
    projection: jest.fn(),
    pointRadius: jest.fn(),
  })),
  geoNaturalEarth1: jest.fn(() => ({
    scale: jest.fn().mockReturnThis(),
    translate: jest.fn().mockReturnThis(),
    center: jest.fn().mockReturnThis(),
  })),
  geoMercator: jest.fn(() => ({
    scale: jest.fn().mockReturnThis(),
    translate: jest.fn().mockReturnThis(),
    center: jest.fn().mockReturnThis(),
  })),
  geoOrthographic: jest.fn(() => ({
    scale: jest.fn().mockReturnThis(),
    translate: jest.fn().mockReturnThis(),
    center: jest.fn().mockReturnThis(),
  })),
}));

jest.mock('d3-zoom', () => ({
  zoom: jest.fn(() => ({
    scaleExtent: jest.fn().mockReturnThis(),
    on: jest.fn().mockReturnThis(),
  })),
}));

// Global test setup
beforeEach(() => {
  // Clear all mocks
  jest.clearAllMocks();

  // Reset localStorage mock
  (global as any).localStorage.clear();
  (global as any).localStorage.getItem.mockReturnValue(null);
  (global as any).localStorage.setItem.mockImplementation(() => {});
  (global as any).localStorage.removeItem.mockImplementation(() => {});

  // Reset any global state
  delete (global as any).__TEST_STATE__;
});
