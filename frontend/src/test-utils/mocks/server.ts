// REQUIREMENT: Mock Service Worker setup for API mocking in tests
// PURPOSE: Provide realistic API responses for isolated frontend testing
// This enables testing components without requiring a running backend

// Note: Polyfills are set up before this file is imported

// Import mock data (doesn't need polyfills)
import { mockSeriesData, mockDataSources, mockSearchResults, mockSuggestions } from './data';

// Lazy import MSW to ensure polyfills are loaded first
let setupServer: any, graphql: any, http: any, HttpResponse: any;

function ensureMSWImported() {
  if (!setupServer) {
    const msw = require('msw/node');
    const mswCore = require('msw');
    setupServer = msw.setupServer;
    graphql = mswCore.graphql;
    http = mswCore.http;
    HttpResponse = mswCore.HttpResponse;
  }
}

// GraphQL endpoint
// Removed unused GRAPHQL_ENDPOINT

// Create handlers function that ensures MSW is imported
function createHandlers() {
  ensureMSWImported();

  return [
    // GraphQL handlers
    graphql.query('GetSeriesDetail', ({ variables }: { variables: any }) => {
      // REQUIREMENT: Mock series detail query for component testing
      const { id } = variables as { id: string };
      const series = mockSeriesData.find(s => s.id === id);

      if (!series) {
        return HttpResponse.json({
          data: { series: null },
          errors: [{ message: 'Series not found' }],
        });
      }

      return HttpResponse.json({
        data: { series },
      });
    }),

    graphql.query('GetSeriesData', ({ variables }: { variables: any }) => {
      // REQUIREMENT: Mock series data points for chart testing
      // Destructured but unused: seriesId, filter, transformation
      variables as any;

      // Generate mock data points based on parameters
      const dataPoints = Array.from({ length: 12 }, (_, index) => ({
        date: `2024-${String(index + 1).padStart(2, '0')}-01`,
        value: Math.random() * 100 + 50, // Random values between 50-150
        revisionDate: `2024-${String(index + 1).padStart(2, '0')}-15`,
        isOriginalRelease: index % 3 === 0, // Every third point is original
      }));

      return HttpResponse.json({
        data: {
          seriesData: {
            nodes: dataPoints,
            totalCount: dataPoints.length,
            pageInfo: {
              hasNextPage: false,
              hasPreviousPage: false,
              startCursor: null,
              endCursor: null,
            },
          },
        },
      });
    }),

    graphql.query('SearchSeriesFulltext', ({ variables }: { variables: any }) => {
      // REQUIREMENT: Mock full-text search for search component testing
      const { params } = variables as any;
      const { query, limit = 50 } = params;

      // Filter mock results based on query
      const filteredResults = mockSearchResults
        .filter(
          result =>
            result.title.toLowerCase().includes(query.toLowerCase()) ||
            result.description?.toLowerCase().includes(query.toLowerCase())
        )
        .slice(0, limit);

      return HttpResponse.json({
        data: {
          searchSeries: filteredResults,
        },
      });
    }),

    graphql.query('GetSearchSuggestions', ({ variables }: { variables: any }) => {
      // REQUIREMENT: Mock search suggestions for autocomplete testing
      const { partialQuery, limit = 10 } = variables as any;

      const filteredSuggestions = mockSuggestions
        .filter(suggestion =>
          suggestion.suggestion.toLowerCase().startsWith(partialQuery.toLowerCase())
        )
        .slice(0, limit);

      return HttpResponse.json({
        data: {
          searchSuggestions: filteredSuggestions,
        },
      });
    }),

    graphql.query('GetDataSources', () => {
      // REQUIREMENT: Mock data sources for dropdown testing
      return HttpResponse.json({
        data: {
          dataSources: mockDataSources,
        },
      });
    }),

    graphql.query('GetCrawlerStatus', () => {
      // REQUIREMENT: Mock crawler status for monitoring component testing
      return HttpResponse.json({
        data: {
          crawlerStatus: {
            isRunning: true,
            lastRunAt: new Date().toISOString(),
            nextRunAt: new Date(Date.now() + 60000).toISOString(),
            totalJobs: 150,
            completedJobs: 120,
            failedJobs: 5,
            pendingJobs: 25,
          },
          queueStatistics: {
            totalItems: 1000,
            pendingItems: 25,
            processingItems: 3,
            completedItems: 950,
            failedItems: 22,
          },
        },
      });
    }),

    // REST API fallback handlers
    http.get('/api/health', () => {
      // REQUIREMENT: Mock health check endpoint
      return HttpResponse.json({
        status: 'healthy',
        timestamp: new Date().toISOString(),
      });
    }),

    // Handle unmatched GraphQL requests
    graphql.operation(({ operationName }: { operationName: any }) => {
      console.warn(`Unhandled GraphQL operation: ${operationName}`);
      return HttpResponse.json({
        data: null,
        errors: [{ message: `Unhandled operation: ${operationName}` }],
      });
    }),
  ];
}

// Export handlers and server as getters to ensure lazy loading
export let handlers: any, server: any;

// Initialize handlers and server when first accessed
function initializeMSW() {
  if (!handlers) {
    handlers = createHandlers();
  }
  if (!server) {
    ensureMSWImported();
    server = setupServer(...handlers);
  }
}

// Initialize immediately for Vitest compatibility
initializeMSW();

// Export initialization function for setupTests.ts
export function getMSWServer() {
  initializeMSW();
  return server;
}
