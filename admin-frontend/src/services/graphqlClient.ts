/**
 * GraphQL client configuration for admin frontend
 *
 * Provides:
 * - Apollo Client setup with authentication
 * - Error handling and retry logic
 * - Real-time subscriptions support
 * - Request/response logging for debugging
 */

import {
  ApolloClient,
  InMemoryCache,
  createHttpLink,
  from,
} from "@apollo/client";
import { setContext } from "@apollo/client/link/context";
import { onError } from "@apollo/client/link/error";
import { RetryLink } from "@apollo/client/link/retry";

// GraphQL endpoint configuration
const GRAPHQL_ENDPOINT =
  process.env.REACT_APP_GRAPHQL_ENDPOINT || "http://localhost:8080/graphql";

// HTTP link for GraphQL requests
const httpLink = createHttpLink({
  uri: GRAPHQL_ENDPOINT,
  credentials: "include", // Include cookies for authentication
});

// Authentication link to add auth headers
const authLink = setContext((_, { headers }) => {
  // Get the authentication token from localStorage if it exists
  const token = localStorage.getItem("auth_token");

  // Return the headers to the context so httpLink can read them
  return {
    headers: {
      ...headers,
      authorization: token ? `Bearer ${token}` : "",
      "Content-Type": "application/json",
    },
  };
});

// Error handling link
const errorLink = onError(
  ({
    graphQLErrors,
    networkError,
    operation: _operation,
    forward: _forward,
  }: any) => {
    if (graphQLErrors) {
      graphQLErrors.forEach(({ message, locations, path }: any) => {
        console.error(
          `[GraphQL error]: Message: ${message}, Location: ${locations}, Path: ${path}`,
        );
      });
    }

    if (networkError) {
      console.error(`[Network error]: ${networkError}`);

      // Handle specific network errors
      if ("statusCode" in networkError) {
        switch (networkError.statusCode) {
          case 401:
            // Unauthorized - redirect to login
            localStorage.removeItem("auth_token");
            window.location.href = "/login";
            break;
          case 403:
            // Forbidden - show access denied message
            console.error("Access denied: Insufficient permissions");
            break;
          case 500:
            // Server error - show generic error message
            console.error("Server error: Please try again later");
            break;
        }
      }
    }
  },
);

// Retry link for failed requests
const retryLink = new RetryLink({
  delay: {
    initial: 300,
    max: Infinity,
    jitter: true,
  },
  attempts: {
    max: 3,
    retryIf: (error: any, _operation: any) => {
      // Retry on network errors but not on GraphQL errors
      return !!error && !(error as any).result;
    },
  },
});

// Create Apollo Client
export const apolloClient = new ApolloClient({
  link: from([errorLink, retryLink, authLink, httpLink]),
  cache: new InMemoryCache({
    typePolicies: {
      // Cache policy for crawler status - update every 30 seconds
      CrawlerStatus: {
        fields: {
          is_running: {
            merge: false, // Always use server value
          },
          active_workers: {
            merge: false,
          },
          last_crawl: {
            merge: false,
          },
          next_scheduled_crawl: {
            merge: false,
          },
        },
      },
      // Cache policy for queue statistics
      QueueStatistics: {
        fields: {
          total_items: {
            merge: false,
          },
          pending_items: {
            merge: false,
          },
          processing_items: {
            merge: false,
          },
          completed_items: {
            merge: false,
          },
          failed_items: {
            merge: false,
          },
        },
      },
    },
  }),
  defaultOptions: {
    watchQuery: {
      errorPolicy: "all", // Return both data and errors
      fetchPolicy: "cache-and-network", // Use cache but also fetch from network
    },
    query: {
      errorPolicy: "all",
      fetchPolicy: "cache-first", // Use cache first, then network
    },
    mutate: {
      errorPolicy: "all",
    },
  },
  // Enable request/response logging in development
  ...(process.env.NODE_ENV === "development" && {
    connectToDevTools: true,
  }),
});

// Helper function to get authentication token
export const getAuthToken = (): string | null => {
  return localStorage.getItem("auth_token");
};

// Helper function to set authentication token
export const setAuthToken = (token: string): void => {
  localStorage.setItem("auth_token", token);
};

// Helper function to clear authentication token
export const clearAuthToken = (): void => {
  localStorage.removeItem("auth_token");
};

// Helper function to check if user is authenticated
export const isAuthenticated = (): boolean => {
  return !!getAuthToken();
};

export default apolloClient;
