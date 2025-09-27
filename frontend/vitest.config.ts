import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/setupTests.vitest.ts'],
    globals: true,
    deps: {
      inline: ['d3', 'd3-geo', 'd3-zoom', 'd3-scale', 'd3-scale-chromatic', 'd3-array', 'd3-selection']
    }
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src')
    }
  }
})
