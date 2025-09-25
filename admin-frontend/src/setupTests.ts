// jest-dom adds custom jest matchers for asserting on DOM nodes.
// allows you to do things like:
// expect(element).toHaveTextContent(/react/i)
// learn more: https://github.com/testing-library/jest-dom
import "@testing-library/jest-dom";

// Polyfills for MSW in Node.js environment
import { TextEncoder, TextDecoder } from "util";
import { TransformStream as NodeTransformStream } from "stream/web";

global.TextEncoder = TextEncoder;
global.TextDecoder = TextDecoder as any;
global.TransformStream = NodeTransformStream as any;

// Note: Timer mocking is now handled per-test to avoid interfering with async operations
// Individual tests can use jest.useFakeTimers() if needed for specific timer testing

// Note: Context mocks are handled per-test-file to avoid conflicts

// Note: React Query mocks are handled per-test-file to avoid conflicts

// Set timeout for CI
if (process.env.CI) {
  jest.setTimeout(15000);
} else {
  jest.setTimeout(5000);
}

// MSW setup is now handled per-test to avoid conflicts
// Individual tests should call setupSimpleMSW() and cleanupSimpleMSW() as needed

// Fetch mocking is now handled by MSW

// Mock window.matchMedia
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

// Mock ResizeObserver - must be done before any imports
class MockResizeObserver {
  observe = jest.fn();
  unobserve = jest.fn();
  disconnect = jest.fn();
}

// Mock ResizeObserver globally
global.ResizeObserver = MockResizeObserver as any;

// Also mock ResizeObserver on window for compatibility
Object.defineProperty(window, "ResizeObserver", {
  writable: true,
  value: MockResizeObserver,
});

// Mock ResizeObserver on globalThis as well
Object.defineProperty(globalThis, "ResizeObserver", {
  writable: true,
  value: MockResizeObserver,
});

// Mock IntersectionObserver
global.IntersectionObserver = jest.fn().mockImplementation(() => ({
  observe: jest.fn(),
  unobserve: jest.fn(),
  disconnect: jest.fn(),
}));

// Mock crypto.randomUUID
Object.defineProperty(global, "crypto", {
  value: {
    randomUUID: jest.fn(() => "mock-uuid-1234"),
  },
});

// Mock window.open
Object.defineProperty(window, "open", {
  writable: true,
  value: jest.fn(),
});

// Mock console methods to reduce noise in tests
const originalConsoleError = console.error;
const originalConsoleWarn = console.warn;

beforeAll(() => {
  console.error = (...args: any[]) => {
    if (
      typeof args[0] === "string" &&
      (args[0].includes("Warning: ReactDOM.render is no longer supported") ||
        args[0].includes("Warning: validateDOMNesting") ||
        args[0].includes("Warning: React does not recognize"))
    ) {
      return;
    }
    originalConsoleError.call(console, ...args);
  };

  console.warn = (...args: any[]) => {
    if (
      typeof args[0] === "string" &&
      (args[0].includes("Warning: ReactDOM.render is no longer supported") ||
        args[0].includes("Warning: validateDOMNesting") ||
        args[0].includes("Warning: React does not recognize"))
    ) {
      return;
    }
    originalConsoleWarn.call(console, ...args);
  };
});

afterAll(() => {
  console.error = originalConsoleError;
  console.warn = originalConsoleWarn;
});

// Clean up after each test - but don't clear all mocks as it breaks MSW
afterEach(() => {
  // Don't clear all mocks - this breaks MSW fetch interception
  // jest.clearAllMocks();

  // Only clear timers if they were mocked in the test
  if (jest.isMockFunction(setTimeout)) {
    jest.clearAllTimers();
  }
});
