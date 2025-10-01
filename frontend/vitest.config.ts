import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/setupTests.vitest.ts'],
    globals: true,
    forceRerunTriggers: ['**/package.json/**', '**/vitest.config.*/**', '**/vite.config.*/**'],
    pool: 'forks',
    poolOptions: {
      forks: {
        singleFork: true,
        isolate: false, // Share memory between tests for faster execution
      },
    },
    // Global timeout for entire test run
    timeout: 15000, // 15 seconds max for all tests
    // Force exit after tests complete
    teardownTimeout: 200,
    testTimeout: 3000,
    hookTimeout: 3000,
    bail: 0, // Don't bail on first failure
    // Force process exit after tests complete to prevent hanging
    forceExit: true,
    exclude: [
      '**/node_modules/**',
      '**/dist/**',
      '**/cypress/**',
      '**/.{idea,git,cache,output,temp}/**',
      '**/{karma,rollup,webpack,vite,vitest,jest,ava,babel,nyc,cypress,tsup,build}.config.*',
      '**/server/**/*.test.cjs',
      '**/__tests__/integration/privateChartServer.test.cjs',
      '**/__tests__/integration/**/*.test.tsx',
      // Ignore unit-level e2e-style tests for now
      '**/__tests__/e2e-*.test.tsx',
      '**/tests/**/*.spec.ts',
      '**/tests/**/*.spec.tsx',
    ],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'src/setupTests.vitest.ts',
        'src/test-utils/',
        '**/*.d.ts',
        '**/*.config.*',
        '**/coverage/**',
        '**/dist/**',
        '**/build/**',
        '**/playwright-report/**',
        '**/test-results/**',
      ],
      thresholds: {
        global: {
          branches: 70,
          functions: 70,
          lines: 70,
          statements: 70,
        },
      },
    },
    deps: {
      inline: [
        'd3',
        'd3-geo',
        'd3-zoom',
        'd3-scale',
        'd3-scale-chromatic',
        'd3-array',
        'd3-selection',
        '@tanstack/react-query',
      ],
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
