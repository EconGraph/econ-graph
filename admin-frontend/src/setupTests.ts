/**
 * Test setup file for admin frontend
 *
 * Configures:
 * - Testing environment
 * - Mock implementations
 * - Global test utilities
 * - Accessibility testing helpers
 */

import "@testing-library/jest-dom";

// Mock window.matchMedia for Material-UI components
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: jest.fn().mockImplementation((query) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: jest.fn(), // deprecated
    removeListener: jest.fn(), // deprecated
    addEventListener: jest.fn(),
    removeEventListener: jest.fn(),
    dispatchEvent: jest.fn(),
  })),
});

// Mock IntersectionObserver
global.IntersectionObserver = class IntersectionObserver {
  constructor() {}
  disconnect() {}
  observe() {}
  unobserve() {}
} as any;

// Mock ResizeObserver
global.ResizeObserver = class ResizeObserver {
  constructor() {}
  disconnect() {}
  observe() {}
  unobserve() {}
};

// Mock WebSocket for real-time features
global.WebSocket = class WebSocket {
  constructor() {}
  close() {}
  send() {}
  addEventListener() {}
  removeEventListener() {}
  dispatchEvent() {}
} as any;

// Mock console methods to avoid noise in tests
const originalConsoleError = console.error;
const originalConsoleWarn = console.warn;
const originalConsoleLog = console.log;

beforeAll(() => {
  console.error = jest.fn();
  console.warn = jest.fn();
  console.log = jest.fn();
});

afterAll(() => {
  console.error = originalConsoleError;
  console.warn = originalConsoleWarn;
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
