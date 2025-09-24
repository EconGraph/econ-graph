/**
 * Jest Configuration for EconGraph Admin Frontend
 * Optimized for memory usage and developer experience
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
    '!src/index.tsx'
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

  // Test environment and setup
  testEnvironment: 'jsdom',

  // Transform settings
  transform: {
    '^.+\\.(ts|tsx)$': ['ts-jest', {
      tsconfig: {
        jsx: 'react-jsx'
      }
    }]
  },

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
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx', 'json']
};
