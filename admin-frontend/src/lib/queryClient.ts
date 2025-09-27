/**
 * React Query Client Configuration
 *
 * Optimized for admin UI with focus on developer experience and debugging.
 * Bundle size is not a concern since this is for employees, not customers.
 */

import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Admin UI can afford longer stale times since data doesn't change as frequently
      staleTime: 5 * 60 * 1000, // 5 minutes
      gcTime: 10 * 60 * 1000, // 10 minutes

      // Retry logic for admin operations
      retry: (failureCount, error) => {
        // Don't retry on 4xx errors (client errors)
        if (
          error instanceof Error &&
          "status" in error &&
          typeof error.status === "number" &&
          error.status >= 400 &&
          error.status < 500
        ) {
          return false;
        }
        // Retry up to 3 times for server errors
        return failureCount < 3;
      },

      // Background refetch for admin dashboards
      refetchOnWindowFocus: true,
      refetchOnReconnect: true,
    },
    mutations: {
      // Retry mutations once for admin operations
      retry: 1,
    },
  },
});

// Enable React Query DevTools in development
if (process.env.NODE_ENV === "development") {
  // DevTools will be imported and used in the main App component
}
