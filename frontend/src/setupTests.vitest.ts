// Vitest setup file for MSW integration
// This file sets up the test environment for Vitest with MSW support

import '@testing-library/jest-dom';
import { vi, beforeAll, afterEach, afterAll } from 'vitest';
import { setupServer } from 'msw/node';
import { http, HttpResponse } from 'msw';
import { loadGraphQLResponse } from './test-utils/mocks/graphql-response-loader';

// MSW server for mocking API calls
const server = setupServer(
  // GraphQL endpoint handler
  http.post('/graphql', ({ request }) => {
    return request.json().then((body: any) => {
      console.log('MSW GraphQL Request:', body.query);
      console.log('MSW GraphQL Variables:', body.variables);
      
      // Handle different GraphQL queries based on the query string
      if (body.query && body.query.includes('GetFinancialDashboard')) {
        // Financial dashboard query
        const response = loadGraphQLResponse('get_financial_dashboard', 'success');
        return HttpResponse.json(response);
      } else if (body.query && body.query.includes('GetFinancialRatios')) {
        // Financial ratios query
        console.log('MSW returning financial ratios data');
        console.log('MSW GetFinancialRatios query variables:', body.variables);
        
        // Handle empty data scenarios
        if (body.variables?.statementId === 'error-statement-id' || body.variables?.statementId === 'empty-statement-id') {
          const response = loadGraphQLResponse('get_financial_ratios', 'empty');
          return HttpResponse.json(response);
        }
        
        const response = loadGraphQLResponse('get_financial_ratios', 'success');
        return HttpResponse.json(response);
      } else {
        // Default financial statement query
        const response = loadGraphQLResponse('get_financial_statement', 'success');
        return HttpResponse.json(response);
      }
    });
  })
);

// Start server before all tests
beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));

// Reset handlers after each test `important for test isolation`
afterEach(() => server.resetHandlers());

// Clean up after all tests
afterAll(() => server.close());

// Mock console methods to reduce noise in test output
global.console = {
  ...console,
  // Uncomment to ignore a specific log level
  log: vi.fn(),
  debug: vi.fn(),
  info: vi.fn(),
  warn: vi.fn(),
  error: vi.fn(),
};

// Mock window.matchMedia
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation(query => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(), // deprecated
    removeListener: vi.fn(), // deprecated
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

// Mock ResizeObserver
global.ResizeObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  unobserve: vi.fn(),
  disconnect: vi.fn(),
}));

// Mock IntersectionObserver
global.IntersectionObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  unobserve: vi.fn(),
  disconnect: vi.fn(),
}));

// Mock URL.createObjectURL
global.URL.createObjectURL = vi.fn(() => 'mocked-url');

// Mock fetch for tests that don't use MSW
global.fetch = vi.fn();

// Mock react-query hooks
vi.mock('@/hooks/useSeriesSearch', () => ({
  useSeriesSearch: vi.fn().mockImplementation(() => ({
    data: [],
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