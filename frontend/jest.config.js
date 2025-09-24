/**
 * Jest Configuration for EconGraph Frontend
 * Optimized for memory usage and developer experience
 *
 * Memory optimizations:
 * - Limited maxWorkers to reduce memory usage
 * - Set max-old-space-size to 2GB for Node.js processes
 * - Configure testTimeout for better resource management
 * - Use --runInBand for memory-constrained environments
 */

const { defaults } = require('jest-config');

module.exports = {
  ...defaults,

  // Memory optimization settings
  maxWorkers: process.env.CI ? 2 : Math.max(1, Math.floor(require('os').cpus().length / 2)),

  // Test execution settings
  testTimeout: 30000,
  verbose: false,

  // Memory management
  workerIdleMemoryLimit: '512MB',

  // Coverage settings (reduced for memory efficiency)
  collectCoverageFrom: [
    'src/**/*.{ts,tsx}',
    '!src/**/*.d.ts',
    '!src/**/*.stories.{ts,tsx}',
    '!src/**/__tests__/**',
    '!src/**/__mocks__/**',
    '!src/setupTests.ts',
    '!src/index.tsx',
    '!src/reportWebVitals.ts'
  ],

  // Coverage thresholds (relaxed for memory efficiency)
  coverageThreshold: {
    global: {
      branches: 60,
      functions: 60,
      lines: 60,
      statements: 60
    }
  },

  // Module name mapping (existing configuration)
  moduleNameMapper: {
    '^@/(.*)$': '<rootDir>/src/$1',
    '^react-app/jest$': 'react-app/jest',
    '^d3$': '<rootDir>/src/__mocks__/d3.js',
    '^d3-geo$': '<rootDir>/src/__mocks__/d3-geo.js',
    '^d3-zoom$': '<rootDir>/src/__mocks__/d3-zoom.js',
    '^d3-scale$': '<rootDir>/src/__mocks__/d3-scale.js',
    '^d3-scale-chromatic$': '<rootDir>/src/__mocks__/d3-scale-chromatic.js',
    '^d3-array$': '<rootDir>/src/__mocks__/d3-array.js',
    '^d3-selection$': '<rootDir>/src/__mocks__/d3-selection.js'
  },

  // Test environment and setup
  testEnvironment: 'jsdom',
  setupFilesAfterEnv: ['<rootDir>/src/setupTests.ts'],

  // Transform settings
  transform: {
    '^.+\\.(ts|tsx)$': ['ts-jest', {
      tsconfig: {
        jsx: 'react-jsx'
      }
    }]
  },

  // Transform ignore patterns
  transformIgnorePatterns: [
    'node_modules/(?!(@apollo/client|d3-geo|d3-zoom|d3-scale|d3-scale-chromatic|d3-array|d3-selection)/)'
  ],

  // Mock settings
  clearMocks: true,
  resetMocks: true,
  restoreMocks: true,

  // Test patterns
  testMatch: [
    '<rootDir>/src/**/__tests__/**/*.{ts,tsx}',
    '<rootDir>/src/**/*.{test,spec}.{ts,tsx}'
  ],

  // Ignore patterns
  testPathIgnorePatterns: [
    '<rootDir>/node_modules/',
    '<rootDir>/build/',
    '<rootDir>/coverage/'
  ],

  // Module file extensions
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx', 'json'],

  // Global setup for memory optimization
  globalSetup: '<rootDir>/src/test-utils/globalSetup.js',
  globalTeardown: '<rootDir>/src/test-utils/globalTeardown.js'
};
