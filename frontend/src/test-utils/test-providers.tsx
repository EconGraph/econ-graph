// REQUIREMENT: Test providers for React component testing
// PURPOSE: Provide necessary context providers for isolated component testing
// This ensures components have access to routing, theming, and data fetching contexts

import React from 'react';
import { QueryClient, QueryClientProvider } from 'react-query';
import { BrowserRouter } from 'react-router-dom';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { CssBaseline } from '@mui/material';
import { render } from '@testing-library/react';

interface TestProvidersProps {
  children: React.ReactNode;
  queryClient?: QueryClient;
}

/**
 * Test wrapper component that provides all necessary contexts
 * REQUIREMENT: Isolated testing environment with all required providers
 */
export function TestProviders({ children, queryClient }: TestProvidersProps) {
  // Create a fresh QueryClient for each test to avoid cache interference
  const testQueryClient =
    queryClient ||
    new QueryClient({
      defaultOptions: {
        queries: {
          retry: false, // Disable retries in tests
          cacheTime: 0, // Disable caching in tests
          staleTime: 0,
        },
        mutations: {
          retry: false,
        },
      },
    });

  // Create a simple test theme
  const testTheme = createTheme({
    palette: {
      mode: 'light',
    },
  });

  return (
    <QueryClientProvider client={testQueryClient}>
      <BrowserRouter>
        <ThemeProvider theme={testTheme}>
          <CssBaseline />
          {children}
        </ThemeProvider>
      </BrowserRouter>
    </QueryClientProvider>
  );
}

/**
 * Custom render function that includes all providers
 * REQUIREMENT: Simplified testing setup with automatic provider wrapping
 */
export function renderWithProviders(
  ui: React.ReactElement,
  options: {
    queryClient?: QueryClient;
    initialEntries?: string[];
  } = {}
) {
  const { queryClient, ...renderOptions } = options;

  function Wrapper({ children }: { children: React.ReactNode }) {
    return <TestProviders queryClient={queryClient}>{children}</TestProviders>;
  }

  return {
    ...render(ui, { wrapper: Wrapper, ...renderOptions }),
    queryClient: queryClient || new QueryClient(),
  };
}

// Re-export testing utilities
export * from '@testing-library/react';
// Note: renderWithProviders is available as a separate export to avoid conflicts

/**
 * Create a mock QueryClient with custom configuration
 * REQUIREMENT: Configurable query client for different test scenarios
 */
export function createMockQueryClient(overrides: Partial<QueryClient> = {}) {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        cacheTime: 0,
        staleTime: 0,
      },
      mutations: {
        retry: false,
      },
    },
    ...overrides,
  });
}

/**
 * Wait for React Query to finish loading
 * REQUIREMENT: Utility for waiting for async data fetching in tests
 */
export async function waitForLoadingToFinish() {
  const { waitFor } = await import('@testing-library/react');
  await waitFor(
    () => {
      // eslint-disable-next-line testing-library/no-node-access
      expect(document.querySelector('[data-testid="loading"]')).not.toBeInTheDocument();
    },
    { timeout: 3000 }
  );
}
