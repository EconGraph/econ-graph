// REQUIREMENT: Mock Service Worker setup for API mocking in tests
// PURPOSE: Provide realistic API responses for isolated frontend testing
// This enables testing components without requiring a running backend

// Note: Polyfills are set up before this file is imported

// Import mock data (doesn't need polyfills)
import { mockSeriesData, mockDataSources, mockSearchResults, mockSuggestions } from './data';
import { loadGraphQLResponse } from './graphql-response-loader';

// Import MSW directly
import { setupServer } from 'msw/node';
import { graphql, http, HttpResponse } from 'msw';

// GraphQL endpoint
// Removed unused GRAPHQL_ENDPOINT

// Create handlers function
function createHandlers() {
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
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const { seriesId, filter, transformation } = variables;

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
        .filter(suggestion => suggestion.toLowerCase().startsWith(partialQuery.toLowerCase()))
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

    // Financial GraphQL handlers using JSON response files
    graphql.query('GetFinancialDashboard', ({ variables }: { variables: any }) => {
      const { companyId } = variables as { companyId: string };

      // Use different scenarios based on companyId for testing
      let scenario = 'success';
      if (companyId === 'invalid-company-id') {
        scenario = 'not_found';
      } else if (companyId === 'loading-company-id') {
        scenario = 'loading';
      } else if (companyId === 'partial-company-id') {
        scenario = 'partial_data';
      } else if (companyId === 'error-company-id') {
        scenario = 'error';
      }

      const response = loadGraphQLResponse('get_financial_dashboard', scenario);
      return HttpResponse.json(response, {
        status: response.errors ? 400 : 200,
      });
    }),

    graphql.query('GetFinancialStatement', ({ variables }: { variables: any }) => {
      const { statementId } = variables as { statementId: string };

      let scenario = 'success';
      if (statementId === 'invalid-statement-id') {
        scenario = 'not_found';
      } else if (statementId === 'processing-statement-id') {
        scenario = 'processing';
      } else if (statementId === 'error-statement-id') {
        scenario = 'error';
      }

      const response = loadGraphQLResponse('get_financial_statement', scenario);
      return HttpResponse.json(response, {
        status: response.errors ? 400 : 200,
      });
    }),

    graphql.query('GetFinancialRatios', ({ variables }: { variables: any }) => {
      const { statementId } = variables as { statementId: string };

      let scenario = 'success';
      if (statementId === 'empty-statement-id') {
        scenario = 'empty';
      } else if (statementId === 'partial-statement-id') {
        scenario = 'partial';
      } else if (statementId === 'error-statement-id') {
        scenario = 'error';
      }

      const response = loadGraphQLResponse('get_financial_ratios', scenario);
      return HttpResponse.json(response, {
        status: response.errors ? 400 : 200,
      });
    }),

    graphql.query('GetRatioBenchmarks', ({ variables }: { variables: any }) => {
      const { ratioName } = variables as { ratioName: string };

      let scenario = 'success';
      if (ratioName === 'unknownRatio') {
        scenario = 'not_found';
      }

      const response = loadGraphQLResponse('get_ratio_benchmarks', scenario);
      return HttpResponse.json(response, {
        status: response.errors ? 400 : 200,
      });
    }),

    graphql.query('GetRatioExplanation', ({ variables }: { variables: any }) => {
      const { ratioName } = variables as { ratioName: string };

      let scenario = 'success';
      if (ratioName === 'unknownRatio') {
        scenario = 'not_found';
      }

      const response = loadGraphQLResponse('get_ratio_explanation', scenario);
      return HttpResponse.json(response, {
        status: response.errors ? 400 : 200,
      });
    }),

    // REST API fallback handlers (intentionally minimal; no health endpoint mocking)

    // Handle GraphQL POST requests
    http.post('/graphql', async ({ request }: { request: any }) => {
      const body = await request.json();
      const { query, variables, operationName } = body;

      // Extract operation name from query
      const operationMatch = query.match(/(?:query|mutation|subscription)\s+(\w+)/);
      const extractedOperationName = operationMatch ? operationMatch[1] : operationName;

      if (process.env.MSW_DEBUG) {
        console.log('🔧 MSW GraphQL Request:', {
          query: query.substring(0, 100) + '...',
          variables,
          operationName: extractedOperationName,
        });
      }

      // Route to appropriate handler based on operation name
      if (extractedOperationName === 'GetFinancialStatement') {
        try {
          const { statementId } = variables || {};
          let scenario = 'success';
          if (statementId === 'invalid-statement-id') {
            scenario = 'not_found';
          } else if (statementId === 'processing-statement-id') {
            scenario = 'processing';
          } else if (statementId === 'error-statement-id') {
            scenario = 'error';
          }

          if (process.env.MSW_DEBUG) {
            console.log('🔧 Loading GraphQL response for get_financial_statement:', scenario);
          }

          const response = loadGraphQLResponse('get_financial_statement', scenario);

          if (process.env.MSW_DEBUG) {
            console.log('🔧 GraphQL response loaded:', response);
          }

          return HttpResponse.json(response, {
            status: response.errors ? 400 : 200,
          });
        } catch (error) {
          if (process.env.MSW_DEBUG) {
            console.error('🔧 Error in GetFinancialStatement handler:', error);
          }
          return HttpResponse.json(
            {
              data: null,
              errors: [{ message: 'Internal server error' }],
            },
            { status: 500 }
          );
        }
      }

      if (extractedOperationName === 'GetFinancialDashboard') {
        const { companyId } = variables || {};
        let scenario = 'success';
        if (companyId === 'invalid-company-id') {
          scenario = 'not_found';
        } else if (companyId === 'error-company-id') {
          scenario = 'error';
        }

        const response = loadGraphQLResponse('get_financial_dashboard', scenario);
        return HttpResponse.json(response, {
          status: response.errors ? 400 : 200,
        });
      }

      if (extractedOperationName === 'GetFinancialRatios') {
        const { statementId } = variables || {};
        let scenario = 'success';
        if (statementId === 'empty-statement-id') {
          scenario = 'empty';
        } else if (statementId === 'partial-statement-id') {
          scenario = 'partial';
        } else if (statementId === 'error-statement-id') {
          scenario = 'error';
        }

        const response = loadGraphQLResponse('get_financial_ratios', scenario);
        return HttpResponse.json(response, {
          status: response.errors ? 400 : 200,
        });
      }

      // Default fallback
      console.warn(`Unhandled GraphQL operation: ${extractedOperationName}`);
      return HttpResponse.json(
        {
          data: null,
          errors: [{ message: `Unhandled operation: ${extractedOperationName}` }],
        },
        { status: 400 }
      );
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

// Create handlers and server
const handlers = createHandlers();
const server = setupServer(...handlers);

// Configure server to not fallback to network for unhandled requests
server.use(
  http.get('*', () => {
    throw new Error('Unhandled GET request - add handler or check URL');
  }),
  http.post('*', () => {
    throw new Error('Unhandled POST request - add handler or check URL');
  }),
  http.put('*', () => {
    throw new Error('Unhandled PUT request - add handler or check URL');
  }),
  http.delete('*', () => {
    throw new Error('Unhandled DELETE request - add handler or check URL');
  })
);

// Export handlers and server
export { handlers, server };
