/**
 * Test isolation utilities to prevent test pollution
 * PURPOSE: Ensure each test runs in isolation without affecting others
 * This prevents race conditions and state pollution in parallel test execution
 */

// Global state that needs to be reset between tests
let originalLocalStorage: Storage;
let originalSessionStorage: Storage;
let originalConsole: {
  warn: typeof console.warn;
  error: typeof console.error;
  log: typeof console.log;
};

export const setupTestIsolation = () => {
  // Store original implementations
  originalLocalStorage = window.localStorage;
  originalSessionStorage = window.sessionStorage;
  originalConsole = {
    warn: console.warn,
    error: console.error,
    log: console.log,
  };
};

export const cleanupTestIsolation = () => {
  // Restore original implementations
  Object.defineProperty(window, 'localStorage', {
    value: originalLocalStorage,
    writable: true,
  });

  Object.defineProperty(window, 'sessionStorage', {
    value: originalSessionStorage,
    writable: true,
  });

  console.warn = originalConsole.warn;
  console.error = originalConsole.error;
  console.log = originalConsole.log;
};

export const createIsolatedLocalStorage = () => {
  const store: Record<string, string> = {};

  return {
    getItem: jest.fn((key: string) => store[key] || null),
    setItem: jest.fn((key: string, value: string) => {
      store[key] = value;
    }),
    removeItem: jest.fn((key: string) => {
      delete store[key];
    }),
    clear: jest.fn(() => {
      Object.keys(store).forEach(key => delete store[key]);
    }),
    length: 0,
    key: jest.fn(() => null),
  };
};

export const createIsolatedSessionStorage = () => {
  const store: Record<string, string> = {};

  return {
    getItem: jest.fn((key: string) => store[key] || null),
    setItem: jest.fn((key: string, value: string) => {
      store[key] = value;
    }),
    removeItem: jest.fn((key: string) => {
      delete store[key];
    }),
    clear: jest.fn(() => {
      Object.keys(store).forEach(key => delete store[key]);
    }),
    length: 0,
    key: jest.fn(() => null),
  };
};

// Global test isolation setup
beforeAll(() => {
  setupTestIsolation();
});

afterAll(() => {
  cleanupTestIsolation();
});
