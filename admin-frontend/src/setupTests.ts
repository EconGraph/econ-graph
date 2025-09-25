// jest-dom adds custom jest matchers for asserting on DOM nodes.
// allows you to do things like:
// expect(element).toHaveTextContent(/react/i)
// learn more: https://github.com/testing-library/jest-dom
import "@testing-library/jest-dom";

// Mock timers globally to prevent resource leaks in all tests
jest.useFakeTimers();

// Mock setTimeout to run immediately in tests
jest
  .spyOn(global, "setTimeout")
  .mockImplementation((fn: any, delay?: number) => {
    if (typeof fn === "function") {
      fn();
    }
    return 1 as any;
  });

// Mock setInterval to prevent resource leaks
jest
  .spyOn(global, "setInterval")
  .mockImplementation((fn: any, delay?: number) => {
    if (typeof fn === "function") {
      fn();
    }
    return 1 as any;
  });

// Mock clearTimeout and clearInterval to prevent errors
jest.spyOn(global, "clearTimeout").mockImplementation(() => {});

jest.spyOn(global, "clearInterval").mockImplementation(() => {});

// Mock the contexts globally to prevent resource leaks
jest.mock("./contexts/AuthContext", () => ({
  AuthProvider: ({ children }: any) => children,
  useAuth: () => ({
    user: {
      id: "test-user",
      username: "Test Admin",
      role: "super_admin",
      sessionExpiry: new Date(Date.now() + 3600000).toISOString(),
    },
    isAuthenticated: true,
    login: jest.fn(),
    logout: jest.fn(),
    refreshSession: jest.fn(),
    extendSession: jest.fn(),
  }),
}));

jest.mock("./contexts/SecurityContext", () => ({
  SecurityProvider: ({ children }: any) => children,
  useSecurity: () => ({
    checkAccess: jest.fn(() => true),
    logSecurityEvent: jest.fn(),
    securityEvents: [],
    sessionRemainingTime: 3600, // 60 minutes in seconds
  }),
}));

// Set timeout for CI
if (process.env.CI) {
  jest.setTimeout(15000);
} else {
  jest.setTimeout(5000);
}

// Mock fetch globally
global.fetch = jest.fn(() =>
  Promise.resolve({
    ok: true,
    status: 200,
    json: () => Promise.resolve({}),
    text: () => Promise.resolve(""),
  }),
) as jest.Mock;

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

// Clean up after each test
afterEach(() => {
  jest.clearAllMocks();
  jest.clearAllTimers();
});
