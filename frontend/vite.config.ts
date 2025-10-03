// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
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
    proxy: {
      // Proxy API requests to backend
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        secure: false,
      },
      // Proxy GraphQL requests
      '/graphql': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        secure: false,
      },
    },
  },

  // Build configuration
  build: {
    outDir: 'dist',
    sourcemap: true,
    // Optimize chunks for better caching
    rollupOptions: {
      output: {
        manualChunks: {
          // Separate vendor chunks for better caching
          vendor: ['react', 'react-dom'],
          mui: ['@mui/material', '@mui/icons-material', '@emotion/react', '@emotion/styled'],
          d3: ['d3', 'd3-geo', 'd3-zoom', 'd3-scale', 'd3-selection'],
          charts: ['chart.js', 'react-chartjs-2'],
          router: ['react-router-dom'],
          query: ['@tanstack/react-query'],
        },
      },
    },
    // Increase chunk size warning limit for large bundles
    chunkSizeWarningLimit: 1000,
  },

  // Path resolution
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
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
      '@': resolve(__dirname, 'src'),
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
});
