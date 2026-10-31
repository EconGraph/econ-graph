/**
 * GraphQL client configuration for admin frontend
 *
 * Provides:
 * - Apollo Client setup with authentication
 * - Error handling and retry logic
 * - Per-operation read retry control; mutations are never replayed
 * - Request/response logging for debugging
 */

import {
  ApolloClient,
  InMemoryCache,
  createHttpLink,
  from,
  gql,
} from "@apollo/client";
import { setContext } from "@apollo/client/link/context";
import { onError } from "@apollo/client/link/error";
import { RetryLink } from "@apollo/client/link/retry";
import { CombinedGraphQLErrors, ServerError } from "@apollo/client/errors";
import { getMainDefinition } from "@apollo/client/utilities";
import { queryClient } from "../lib/queryClient";

// Apollo Client 4.3+ requires non-default `errorPolicy` defaults to be declared for type safety.
// Keep in sync with `defaultOptions` below.
/* eslint-disable @typescript-eslint/no-namespace -- Apollo declares these as namespaces; augmentation must match. */
declare module "@apollo/client" {
  namespace ApolloClient {
    namespace DeclareDefaultOptions {
      interface WatchQuery {
        errorPolicy: "all";
      }
      interface Query {
        errorPolicy: "all";
      }
      interface Mutate {
        errorPolicy: "all";
      }
    }
  }
}
/* eslint-enable @typescript-eslint/no-namespace */

// GraphQL endpoint configuration
const GRAPHQL_ENDPOINT = import.meta.env.VITE_GRAPHQL_ENDPOINT || "/graphql";

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

// Apollo 4 passes a single typed error, rather than Apollo 3's networkError fields.
const errorLink = onError(({ error }) => {
  if (CombinedGraphQLErrors.is(error)) {
    error.errors.forEach(({ message }) =>
      console.error(`[GraphQL error]: ${message}`),
    );
  } else {
    console.error(`[Network error]: ${error.message}`);
    if (ServerError.is(error)) {
      if (error.statusCode === 401) {
        clearAuthToken();
        window.location.href = "/login";
      } else if (error.statusCode === 403) {
        console.error("Access denied: Insufficient permissions");
      }
    }
  }
});

// Retry link for failed requests
const retryLink = new RetryLink({
  delay: {
    initial: 300,
    max: Infinity,
    jitter: true,
  },
  attempts: {
    max: 3,
    retryIf: (error, operation) => {
      const definition = getMainDefinition(operation.query);
      // A lost response does not establish whether a mutation committed. Never replay it.
      if (
        operation.getContext().retry === false ||
        definition.kind !== "OperationDefinition" ||
        definition.operation !== "query"
      )
        return false;
      if (CombinedGraphQLErrors.is(error)) return false;
      if (
        ServerError.is(error) &&
        error.statusCode >= 400 &&
        error.statusCode < 500
      )
        return false;
      return !!error;
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
          isRunning: {
            merge: false, // Always use server value
          },
          activeWorkers: {
            merge: false,
          },
          lastCrawl: {
            merge: false,
          },
          nextScheduledCrawl: {
            merge: false,
          },
        },
      },
      // Cache policy for queue statistics
      QueueStatistics: {
        fields: {
          totalItems: {
            merge: false,
          },
          pendingItems: {
            merge: false,
          },
          processingItems: {
            merge: false,
          },
          completedItems: {
            merge: false,
          },
          failedItems: {
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
  ...(import.meta.env.DEV && {
    connectToDevTools: true,
  }),
});

/** One shared authenticated transport for non-Apollo consumers. Polling callers can disable
 * read retries; mutations are never replayed by the link, even when retry is requested. */
export async function requestGraphQL<T>(
  query: string,
  variables?: Record<string, unknown>,
  options: { retry?: boolean } = {},
): Promise<T> {
  const document = gql(query);
  const definition = getMainDefinition(document);
  const common = {
    variables,
    errorPolicy: "none" as const,
    fetchPolicy: "no-cache" as const,
    context: { retry: options.retry !== false, queryDeduplication: false },
  };
  const result =
    definition.kind === "OperationDefinition" &&
    definition.operation === "mutation"
      ? await apolloClient.mutate({ mutation: document, ...common })
      : await apolloClient.query({ query: document, ...common });
  if (result.data == null) throw new Error("GraphQL service returned no data");
  return result.data as T;
}

// Helper function to get authentication token
export const getAuthToken = (): string | null => {
  return localStorage.getItem("auth_token");
};

// Cache identities are opaque generations, never bearer tokens or token claims. In-flight
// responses for an old generation cannot become data for the newly signed-in principal.
let sessionGeneration = 0;
let authSessionReady = true;
let sessionToken = localStorage.getItem("auth_token");
const sessionListeners = new Set<() => void>();
function rotateAuthSession(): void {
  const token = localStorage.getItem("auth_token");
  if (token === sessionToken) return;
  sessionToken = token;
  sessionGeneration += 1;
  const transition = sessionGeneration;
  authSessionReady = false;
  queryClient.clear();
  sessionListeners.forEach((listener) => listener());
  // Do not launch a new session's operation while Apollo is still cancelling the old one.
  void apolloClient.clearStore().then(
    () => {
      if (sessionGeneration === transition) {
        authSessionReady = true;
        sessionGeneration += 1;
        sessionListeners.forEach((listener) => listener());
      }
    },
    () => {
      // Keep new-session queries disabled if old normalized state could not be cleared.
      console.error("Unable to clear GraphQL authentication state");
    },
  );
}
export const getAuthSessionSnapshot = (): number => sessionGeneration;
export const isAuthSessionReady = (): boolean => authSessionReady;
window.addEventListener("storage", (event: StorageEvent) => {
  if (event.key === "auth_token" || event.key === null) rotateAuthSession();
});
export const subscribeAuthSession = (listener: () => void): (() => void) => {
  sessionListeners.add(listener);
  return () => {
    sessionListeners.delete(listener);
  };
};

export const setAuthToken = (token: string): void => {
  localStorage.setItem("auth_token", token);
  rotateAuthSession();
};

export const clearAuthToken = (): void => {
  localStorage.removeItem("auth_token");
  rotateAuthSession();
};

// Helper function to check if user is authenticated
export const isAuthenticated = (): boolean => {
  return !!getAuthToken();
};

export default apolloClient;
