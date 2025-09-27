// Vitest setup file for MSW integration
// This file sets up the test environment for Vitest with MSW support

import '@testing-library/jest-dom';
import { beforeAll, afterEach, afterAll, vi } from 'vitest';
import { setupSimpleMSW, cleanupSimpleMSW } from './test-utils/mocks/simpleServer';

// Start MSW server for all tests
beforeAll(async () => {
  await setupSimpleMSW();
});
afterEach(async () => {
  // Reset to success scenario after each test
  const { setMockScenario, MockScenarios } = await import('./test-utils/mocks/simpleServer');
  setMockScenario(MockScenarios.SUCCESS);
});
afterAll(async () => {
  await cleanupSimpleMSW();
  // Force cleanup of any hanging processes
  if (typeof process !== 'undefined') {
    // Clear any pending timeouts/intervals
    const highestTimeoutId = setTimeout(() => {
      // Empty function for cleanup
    }, 0);
    for (let i = 0; i < Number(highestTimeoutId); i++) {
      clearTimeout(i);
    }

    const highestIntervalId = setInterval(() => {
      // Empty function for cleanup
    }, 0);
    for (let i = 0; i < Number(highestIntervalId); i++) {
      clearInterval(i);
    }

    // Force exit if needed
    setTimeout(() => {
      if (process.exit) {
        process.exit(0);
      }
    }, 100);
  }
});

// Polyfill for Node.js environment
import { TextEncoder, TextDecoder } from 'util';
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore - Node.js TextEncoder/TextDecoder types differ from browser types
global.TextEncoder = TextEncoder;
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore - Node.js TextEncoder/TextDecoder types differ from browser types
global.TextDecoder = TextDecoder;

// Mock localStorage
const localStorageMock = {
  getItem: () => null,
  setItem: () => {
    // Mock implementation
  },
  removeItem: () => {
    // Mock implementation
  },
  clear: () => {
    // Mock implementation
  },
  key: () => null,
  length: 0,
};
(global as any).localStorage = localStorageMock;
Object.defineProperty(window, 'localStorage', {
  value: localStorageMock,
});

// Mock Chart.js and related modules
vi.mock('chart.js', () => ({
  Chart: {
    register: vi.fn(),
  },
  registerables: [],
  CategoryScale: vi.fn(),
  LinearScale: vi.fn(),
  PointElement: vi.fn(),
  LineElement: vi.fn(),
  BarElement: vi.fn(),
  ArcElement: vi.fn(),
  Title: vi.fn(),
  Tooltip: vi.fn(),
  Legend: vi.fn(),
  Filler: vi.fn(),
}));

vi.mock('chartjs-adapter-date-fns', () => ({}));

vi.mock('react-chartjs-2', () => ({
  Line: vi.fn(() => 'Mock Chart'),
  Bar: vi.fn(() => 'Mock Chart'),
  Pie: vi.fn(() => 'Mock Chart'),
}));

// Mock chartjs-plugin-annotation to avoid plugin runtime issues in tests
vi.mock('chartjs-plugin-annotation', () => ({
  __esModule: true,
  default: { id: 'annotation', beforeDraw: vi.fn(), afterDraw: vi.fn() },
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

vi.mock('./hooks/useSeriesData', () => ({
  useDataSources: vi.fn().mockImplementation(() => ({
    data: mockDataSources,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSeriesSearch: vi.fn().mockImplementation(() => ({
    data: mockSearchResults,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSeriesDetail: vi.fn().mockImplementation(() => ({
    data: null,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSeriesData: vi.fn().mockImplementation(() => ({
    data: null,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useSearchSuggestions: vi.fn().mockImplementation(() => ({
    data: [],
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
  useCrawlerStatus: vi.fn().mockImplementation(() => ({
    data: null,
    isLoading: false,
    error: null,
    isError: false,
    isSuccess: true,
  })),
}));

// Mock D3 modules
vi.mock('d3-geo', () => ({
  geoPath: vi.fn(() => ({
    projection: vi.fn(),
    pointRadius: vi.fn(),
  })),
  geoNaturalEarth1: vi.fn(() => ({
    scale: vi.fn().mockReturnThis(),
    translate: vi.fn().mockReturnThis(),
    center: vi.fn().mockReturnThis(),
  })),
  geoMercator: vi.fn(() => ({
    scale: vi.fn().mockReturnThis(),
    translate: vi.fn().mockReturnThis(),
    center: vi.fn().mockReturnThis(),
  })),
  geoOrthographic: vi.fn(() => ({
    scale: vi.fn().mockReturnThis(),
    translate: vi.fn().mockReturnThis(),
    center: vi.fn().mockReturnThis(),
  })),
}));

vi.mock('d3-zoom', () => ({
  zoom: vi.fn(() => ({
    scaleExtent: vi.fn().mockReturnThis(),
    on: vi.fn().mockReturnThis(),
  })),
}));

// Mock react-router-dom
vi.mock('react-router-dom', () => ({
  BrowserRouter: ({ children }: { children: React.ReactNode }) => children,
  Routes: ({ children }: { children: React.ReactNode }) => children,
  Route: ({ children }: { children: React.ReactNode }) => children,
  useNavigate: () => vi.fn(),
  useLocation: () => ({ pathname: '/test', search: '', hash: '', state: null }),
  useParams: () => ({}),
  useSearchParams: () => [new URLSearchParams(), vi.fn()],
  Link: ({ children, to, ...props }: any) => {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const React = require('react');
    return React.createElement('a', { href: to, ...props }, children);
  },
  NavLink: ({ children, to, ...props }: any) => {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const React = require('react');
    return React.createElement('a', { href: to, ...props }, children);
  },
}));
