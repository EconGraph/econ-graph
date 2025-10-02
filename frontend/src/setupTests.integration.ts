// Integration test setup - minimal mocking for real component testing
import '@testing-library/jest-dom';
import React from 'react';

// Only mock the essential modules that are needed for integration tests
// DO NOT mock @tanstack/react-query - we want real HTTP requests with MSW

// Mock Chart.js to avoid canvas issues in tests
import { vi } from 'vitest';

vi.mock('chart.js', () => ({
  Chart: vi.fn(),
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

// Mock useAuth hook for integration tests
vi.mock('@/hooks/useAuth', () => ({
  useAuth: vi.fn(() => ({
    user: {
      id: 'test-user-1',
      email: 'test@example.com',
      name: 'Test User',
      role: 'user',
    },
    isAuthenticated: true,
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
    register: vi.fn(),
  })),
}));

// Mock useDataSources and useSeriesSearch hooks
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
];

const mockSearchResults = {
  results: [
    {
      id: 'gdp-series-1',
      title: 'Gross Domestic Product',
      description: 'Real GDP in billions of chained 2017 dollars',
      source: 'fred',
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
      description: 'Unemployment rate as a percentage of the labor force',
      source: 'bls',
      frequency: 'Monthly',
      units: 'Percent',
      lastUpdated: '2024-11-01T00:00:00Z',
      startDate: '1948-01-01T00:00:00Z',
      endDate: '2024-11-01T00:00:00Z',
      similarityScore: 0.85,
    },
  ],
  totalCount: 2,
  page: 1,
  pageSize: 10,
};

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

// Mock react-router-dom
vi.mock('react-router-dom', () => ({
  useNavigate: vi.fn(() => vi.fn()),
  useLocation: vi.fn(() => ({ pathname: '/', search: '', hash: '', state: null })),
  useParams: vi.fn(() => ({})),
  Link: vi.fn(({ children, ...props }) => React.createElement('a', props, children)),
  NavLink: vi.fn(({ children, ...props }) => React.createElement('a', props, children)),
  Routes: vi.fn(({ children }) => children),
  Route: vi.fn(({ children }) => children),
  BrowserRouter: vi.fn(({ children }) => children),
  MemoryRouter: vi.fn(({ children }) => children),
}));
