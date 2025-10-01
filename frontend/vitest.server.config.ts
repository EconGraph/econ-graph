import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    globals: true,
    testTimeout: 10000,
    // Include only server tests
    include: ['**/server/**/*.test.cjs', '**/__tests__/integration/privateChartServer.test.cjs'],
  },
});
