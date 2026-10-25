import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

import { appVersionDefine } from './appVersion.config.ts';
import { flagDefines, resolveFlagProfile } from './flags.config.ts';

export default defineConfig({
  plugins: [react()],
  // Build-time flags (flags.config.ts); tests use the dev profile unless FLAGS_PROFILE is set.
  define: { ...flagDefines(resolveFlagProfile('serve')), ...appVersionDefine() },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/setupTests.integration.ts'],
    include: ['src/__tests__/integration/**/*.test.tsx'],
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
    server: {
      deps: {
        // MUI X imports Material UI v5 subpaths as directories (`@mui/material/styles`), which
        // Node's ESM resolver rejects; Vite resolves them when it processes these packages.
        inline: ['@mui/x-date-pickers', '@mui/x-internals'],
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(import.meta.dirname, './src'),
    },
  },
});
