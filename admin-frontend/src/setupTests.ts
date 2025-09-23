/**
 * Test setup file for admin frontend
 *
 * Configures:
 * - Testing environment with comprehensive mocking
 * - Material-UI component testing support
 * - GraphQL and Apollo Client mocking
 * - Portal and dialog testing utilities
 * - Accessibility testing helpers
 * - Test isolation and cleanup
 */

import "@testing-library/jest-dom";
import { configure } from "@testing-library/react";

// Configure React Testing Library for admin frontend
configure({
  asyncUtilTimeout: 10000, // Increase timeout for CI environment
  testIdAttribute: "data-testid",
});

// Set Jest timeout for CI environment
if (process.env.CI) {
  jest.setTimeout(30000); // 30 seconds for CI
} else {
  jest.setTimeout(10000); // 10 seconds for local
}

// Mock Apollo Client for GraphQL testing
jest.mock("@apollo/client", () => ({
  gql: jest.fn().mockReturnValue({}),
  useQuery: jest.fn(() => ({
    data: null,
    loading: false,
    error: null,
    refetch: jest.fn(),
  })),
  useMutation: jest.fn(() => [
    jest.fn(),
    {
      data: null,
      loading: false,
      error: null,
    },
  ]),
  ApolloClient: jest.fn(),
  InMemoryCache: jest.fn(),
  ApolloProvider: ({ children }: any) => children,
}));

// Mock GraphQL hooks for crawler admin functionality
jest.mock("./hooks/useCrawlerData", () => ({
  useCrawlerData: jest.fn(() => ({
    status: {
      data: {
        is_running: true,
        active_workers: 5,
        last_crawl: new Date().toISOString(),
        next_scheduled_crawl: new Date(Date.now() + 60000).toISOString(),
      },
      loading: false,
      error: null,
      refresh: jest.fn(),
    },
    queueStats: {
      data: {
        total_items: 1000,
        pending_items: 25,
        processing_items: 10,
        completed_items: 950,
        failed_items: 15,
      },
      loading: false,
      error: null,
      refresh: jest.fn(),
    },
    performance: {
      data: {
        avg_processing_time: 2.5,
        success_rate: 0.95,
        throughput_per_hour: 100,
      },
      loading: false,
      error: null,
      refresh: jest.fn(),
    },
    loading: false,
    error: null,
    refreshAll: jest.fn(),
  })),
}));

jest.mock("./hooks/useCrawlerConfig", () => ({
  useCrawlerConfig: jest.fn(() => ({
    config: {
      data: {
        enabled: true,
        maintenance_mode: false,
        max_workers: 10,
        queue_size_limit: 1000,
        default_timeout: 30,
        default_retry_attempts: 3,
        schedule_frequency: "hourly",
      },
      loading: false,
      error: null,
      refresh: jest.fn(),
    },
    dataSources: {
      data: [
        {
          id: "1",
          name: "FRED",
          description: "Federal Reserve Economic Data",
          enabled: true,
          priority: 1,
          rate_limit: 100,
          retry_attempts: 3,
          timeout: 30,
          health_status: "healthy",
        },
        {
          id: "2",
          name: "BLS",
          description: "Bureau of Labor Statistics",
          enabled: true,
          priority: 2,
          rate_limit: 50,
          retry_attempts: 3,
          timeout: 30,
          health_status: "healthy",
        },
      ],
      loading: false,
      error: null,
      refresh: jest.fn(),
    },
    loading: false,
    error: null,
    refreshAll: jest.fn(),
  })),
}));

jest.mock("./hooks/useCrawlerLogs", () => ({
  useCrawlerLogs: jest.fn(() => ({
    logs: {
      data: [
        {
          id: "1",
          timestamp: new Date().toISOString(),
          level: "INFO",
          message: "Crawler started successfully",
          source: "crawler",
          component: "main",
        },
        {
          id: "2",
          timestamp: new Date(Date.now() - 60000).toISOString(),
          level: "WARN",
          message: "Rate limit approaching for FRED",
          source: "crawler",
          component: "rate_limiter",
        },
      ],
      loading: false,
      error: null,
      refresh: jest.fn(),
    },
    performance: {
      data: {
        avg_processing_time: 2.5,
        success_rate: 0.95,
        throughput_per_hour: 100,
      },
      loading: false,
      error: null,
      refresh: jest.fn(),
    },
    systemHealth: {
      data: {
        cpu_usage: 45.2,
        memory_usage: 67.8,
        disk_usage: 23.1,
        network_latency: 12.5,
      },
      loading: false,
      error: null,
      refresh: jest.fn(),
    },
    logStats: {
      data: {
        total_logs: 10000,
        error_count: 50,
        warning_count: 200,
        info_count: 9750,
      },
      loading: false,
      error: null,
      refresh: jest.fn(),
    },
    loading: false,
    error: null,
    refreshAll: jest.fn(),
  })),
}));

// Mock window.matchMedia for Material-UI components
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: jest.fn().mockImplementation((query) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: jest.fn(), // Deprecated
    removeListener: jest.fn(), // Deprecated
    addEventListener: jest.fn(),
    removeEventListener: jest.fn(),
    dispatchEvent: jest.fn(),
  })),
});

// Mock useMediaQuery hook to prevent theme.breakpoints errors
jest.mock("@mui/material/useMediaQuery", () => {
  return jest.fn(() => false);
});

// Mock IntersectionObserver for lazy loading components
global.IntersectionObserver = jest.fn().mockImplementation(() => ({
  observe: jest.fn(),
  unobserve: jest.fn(),
  disconnect: jest.fn(),
}));

// Mock ResizeObserver
global.ResizeObserver = jest.fn().mockImplementation(() => ({
  observe: jest.fn(),
  unobserve: jest.fn(),
  disconnect: jest.fn(),
}));

// Mock WebSocket for real-time features
global.WebSocket = jest.fn().mockImplementation(() => ({
  close: jest.fn(),
  send: jest.fn(),
  addEventListener: jest.fn(),
  removeEventListener: jest.fn(),
  dispatchEvent: jest.fn(),
})) as any;

// Create isolated localStorage mock for each test
const createLocalStorageMock = () => ({
  getItem: jest.fn(),
  setItem: jest.fn(),
  removeItem: jest.fn(),
  clear: jest.fn(),
  length: 0,
  key: jest.fn(),
});

// Create isolated sessionStorage mock for each test
const createSessionStorageMock = () => ({
  getItem: jest.fn(),
  setItem: jest.fn(),
  removeItem: jest.fn(),
  clear: jest.fn(),
  length: 0,
  key: jest.fn(),
});

// Global storage mocks that will be reset for each test
let localStorageMock = createLocalStorageMock();
let sessionStorageMock = createSessionStorageMock();

Object.defineProperty(window, "localStorage", {
  value: localStorageMock,
  writable: true,
});

Object.defineProperty(window, "sessionStorage", {
  value: sessionStorageMock,
  writable: true,
});

// Suppress console warnings during tests
const originalConsoleWarn = console.warn;
const originalConsoleError = console.error;
const originalConsoleLog = console.log;

beforeEach(() => {
  // Reset localStorage mock for each test to prevent state pollution
  localStorageMock = createLocalStorageMock();
  Object.defineProperty(window, "localStorage", {
    value: localStorageMock,
    writable: true,
  });

  // Reset sessionStorage mock for each test
  sessionStorageMock = createSessionStorageMock();
  Object.defineProperty(window, "sessionStorage", {
    value: sessionStorageMock,
    writable: true,
  });

  // Clear all mocks to prevent test pollution
  jest.clearAllMocks();

  // Suppress console warnings during tests
  console.warn = jest.fn();
  console.error = jest.fn();
  console.log = jest.fn();
});

afterEach(() => {
  // Restore original console methods
  console.warn = originalConsoleWarn;
  console.error = originalConsoleError;
  console.log = originalConsoleLog;
});

// Global test utilities
declare global {
  namespace jest {
    interface Matchers<R> {
      toBeInTheDocument(): R;
      toHaveValue(value: any): R;
      toHaveFocus(): R;
      toHaveAttribute(attr: string, value?: any): R;
    }
  }
}
