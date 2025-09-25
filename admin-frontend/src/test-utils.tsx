/**
 * Test utilities for React Query testing
 *
 * Provides a wrapper component that includes QueryClientProvider
 * for tests that need to test React Query functionality.
 */

import React from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, RenderOptions } from "@testing-library/react";

// Create a test QueryClient with disabled retries and caching
const createTestQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        gcTime: 0,
        staleTime: 0,
      },
      mutations: {
        retry: false,
      },
    },
  });

// Test wrapper component
const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const queryClient = createTestQueryClient();

  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
};

// Custom render function that includes QueryClientProvider
const renderWithQueryClient = (
  ui: React.ReactElement,
  options?: Omit<RenderOptions, "wrapper">,
) => render(ui, { wrapper: TestWrapper, ...options });

export { createTestQueryClient, TestWrapper, renderWithQueryClient };
