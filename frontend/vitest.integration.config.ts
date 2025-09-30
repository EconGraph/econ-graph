import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/setupTests.integration.ts'],
    include: ['src/__tests__/integration/**/*.test.tsx'],
    deps: {
      inline: ['d3', 'd3-geo', 'd3-zoom', 'd3-scale', 'd3-scale-chromatic', 'd3-array', 'd3-selection', '@tanstack/react-query']
    }
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
