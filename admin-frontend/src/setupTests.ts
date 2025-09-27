// Vitest setup file for MSW integration
import { beforeAll, afterEach, afterAll, vi } from "vitest";
import "@testing-library/jest-dom";

// MSW setup - based on MSW examples
import "whatwg-fetch";

// Lazy load MSW server to avoid polyfill issues
let server: any;

beforeAll(async () => {
  const { server: mswServer } = await import("./mocks/server");
  server = mswServer;
  server.listen();
});

// Reset any request handlers that we may add during the tests,
// so they don't affect other tests.
afterEach(() => {
  if (server) {
    server.resetHandlers();
  }
});

// Clean up after the tests are finished.
afterAll(() => {
  if (server) {
    server.close();
  }
});

// Mock window.matchMedia
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: vi.fn().mockImplementation((query) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(), // Deprecated
    removeListener: vi.fn(), // Deprecated
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

// Mock ResizeObserver - must be done before any imports
class MockResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
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
global.IntersectionObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  unobserve: vi.fn(),
  disconnect: vi.fn(),
}));

// Mock crypto.randomUUID and crypto.random
Object.defineProperty(global, "crypto", {
  value: {
    randomUUID: vi.fn(() => "mock-uuid-1234"),
    random: vi.fn(() => new Uint8Array([1, 2, 3, 4, 5])),
  },
});

// Mock BroadcastChannel for MSW
class MockBroadcastChannel {
  constructor(_name: string) {}
  postMessage = vi.fn();
  addEventListener = vi.fn();
  removeEventListener = vi.fn();
  close = vi.fn();
}
global.BroadcastChannel = MockBroadcastChannel as any;

// Mock window.open
Object.defineProperty(window, "open", {
  writable: true,
  value: vi.fn(),
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

// MSW server lifecycle is managed by jest.setup.js

// Clean up after each test - but don't clear all mocks as it breaks MSW
afterEach(() => {
  // Don't clear all mocks - this breaks MSW fetch interception
  // vi.clearAllMocks();

  // Only clear timers if they were mocked in the test
  if (vi.isMockFunction(setTimeout)) {
    vi.clearAllTimers();
  }
});
