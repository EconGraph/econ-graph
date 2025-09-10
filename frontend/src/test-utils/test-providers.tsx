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

  // Create a complete test theme with all required properties including transitions
  const testTheme = createTheme({
    palette: {
      mode: 'light',
      primary: { 
        main: '#1976d2',
        light: '#42a5f5',
        dark: '#1565c0',
      },
      secondary: { 
        main: '#dc004e',
        light: '#f48fb1',
        dark: '#c51162',
      },
    },
    breakpoints: {
      values: {
        xs: 0,
        sm: 600,
        md: 900,
        lg: 1200,
        xl: 1536,
      },
    },
    transitions: {
      duration: {
        shortest: 150,
        shorter: 200,
        short: 250,
        standard: 300,
        complex: 375,
        enteringScreen: 225,
        leavingScreen: 195,
      },
      easing: {
        easeInOut: 'cubic-bezier(0.4, 0, 0.2, 1)',
        easeOut: 'cubic-bezier(0.0, 0, 0.2, 1)',
        easeIn: 'cubic-bezier(0.4, 0, 1, 1)',
        sharp: 'cubic-bezier(0.4, 0, 0.6, 1)',
      },
      create: () => 'all 300ms cubic-bezier(0.4, 0, 0.2, 1) 0ms',
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
      expect(document.querySelector('[data-testid="loading"]')).not.toBeInTheDocument();
    },
    { timeout: 3000 }
  );
}
