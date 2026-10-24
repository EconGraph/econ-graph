// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

import { appVersionDefine } from './appVersion.config.ts';
import { flagDefines, flagsGuard, resolveFlagProfile } from './flags.config.ts';

// Where the dev server and `vite preview` proxy /api and /graphql. The release e2e stack
// (playwright.release.config.ts) points it at its own backend.
const backendUrl = process.env.BACKEND_URL ?? 'http://localhost:8080';

// Where they proxy /data/graphql, the data operations sent when VITE_DATA_GRAPHQL_URL is
// /data/graphql. Defaults to the same backend; set it to read a different data plane.
const dataBackendUrl = process.env.DATA_BACKEND_URL ?? backendUrl;

// Shared by the dev server and `vite preview`.
const proxy = {
  // Proxy API requests to backend
  '/api': {
    target: backendUrl,
    changeOrigin: true,
    secure: false,
  },
  // Proxy GraphQL requests. '/graphql' does not prefix-match '/data/graphql'.
  '/graphql': {
    target: backendUrl,
    changeOrigin: true,
    secure: false,
  },
  // Data operations, to the data backend's /graphql.
  '/data/graphql': {
    target: dataBackendUrl,
    changeOrigin: true,
    secure: false,
    rewrite: (path: string) => path.replace(/^\/data\/graphql/, '/graphql'),
  },
};

// https://vitejs.dev/config/
export default defineConfig(({ command }) => ({
  plugins: [
    flagsGuard(),
    react({
      // Enable React Fast Refresh
      fastRefresh: true,
      // Enable JSX runtime for React 17+ style
      jsxRuntime: 'automatic',
    }),
  ],

  // Development server configuration
  server: {
    port: 3000,
    host: true, // Allow external connections
    proxy,
  },

  // `vite preview` (the release e2e stack) proxies the same paths.
  preview: {
    proxy,
  },

  // Build configuration
  build: {
    outDir: 'dist',
    sourcemap: true,
    rolldownOptions: {
      // The silent sign-in iframe gets its own small page instead of booting the whole app.
      input: {
        main: resolve(import.meta.dirname, 'index.html'),
        silentCallback: resolve(import.meta.dirname, 'silent-callback.html'),
      },
      // Separate vendor chunks for better caching (Vite 8 bundles with Rolldown).
      output: {
        codeSplitting: {
          groups: [
            { name: 'vendor', test: /[\\/]node_modules[\\/](react|react-dom|scheduler)[\\/]/ },
            {
              name: 'mui',
              test: /[\\/]node_modules[\\/](@mui[\\/](material|icons-material)|@emotion[\\/](react|styled))[\\/]/,
            },
            {
              name: 'd3',
              test: /[\\/]node_modules[\\/](d3|d3-geo|d3-zoom|d3-scale|d3-selection)[\\/]/,
            },
            { name: 'charts', test: /[\\/]node_modules[\\/](chart\.js|react-chartjs-2)[\\/]/ },
            { name: 'router', test: /[\\/]node_modules[\\/](react-router|react-router-dom)[\\/]/ },
            { name: 'query', test: /[\\/]node_modules[\\/]@tanstack[\\/]react-query[\\/]/ },
          ],
        },
      },
    },
    // Increase chunk size warning limit for large bundles
    chunkSizeWarningLimit: 1000,
  },

  // Path resolution
  resolve: {
    alias: {
      '@': resolve(import.meta.dirname, 'src'),
    },
  },

  // CSS configuration
  css: {
    // Enable CSS modules if needed
    modules: {
      localsConvention: 'camelCase',
    },
  },

  // Environment variables
  define: {
    // Define global constants
    __DEV__: JSON.stringify(process.env.NODE_ENV === 'development'),
    ...flagDefines(resolveFlagProfile(command)),
    ...appVersionDefine(),
  },

  // Optimize dependencies
  optimizeDeps: {
    include: [
      'react',
      'react-dom',
      'react-router-dom',
      '@tanstack/react-query',
      '@mui/material',
      '@mui/icons-material',
      '@emotion/react',
      '@emotion/styled',
      'd3',
      'd3-geo',
      'd3-zoom',
      'd3-scale',
      'd3-selection',
      'chart.js',
      'react-chartjs-2',
    ],
    // Exclude problematic dependencies from pre-bundling
    exclude: ['@testing-library/react', '@testing-library/jest-dom'],
  },

  // Test configuration for Vitest
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/setupTests.vitest.ts'],
    // Exclude Playwright tests and other non-unit tests
    exclude: [
      '**/node_modules/**',
      '**/dist/**',
      '**/cypress/**',
      '**/.{idea,git,cache,output,temp}/**',
      '**/{karma,rollup,webpack,vite,vitest,jest,ava,babel,nyc,cypress,tsup,build}.config.*',
      '**/tests/e2e/**',
      '**/tests/playwright/**',
      '**/playwright.config.*',
      '**/tests/**/*.spec.ts',
      '**/tests/**/*.spec.js',
    ],
    // Module name mapping
    alias: {
      '@': resolve(import.meta.dirname, 'src'),
    },
    // Increase timeout for slow tests
    testTimeout: 30000,
    // Coverage configuration
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'src/setupTests.vitest.ts',
        'src/**/__mocks__/**',
        'src/**/__tests__/**',
        '**/*.d.ts',
        'tests/**',
      ],
    },
  },
}));
