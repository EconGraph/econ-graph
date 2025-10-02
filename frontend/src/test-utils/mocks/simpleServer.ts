/**
 * Simplified MSW server setup to avoid import issues.
 *
 * This version uses dynamic imports and simpler setup to avoid
 * conflicts with other fetch mocks in the test suite.
 */

// Debug flag - set to true to enable MSW debug output
const MSW_DEBUG = process.env.MSW_DEBUG === 'true' || false;

// Import GraphQL mock responses
import { loadGraphQLResponse } from './graphql-response-loader';

// Mock scenarios for testing different states
export enum MockScenarios {
  SUCCESS = 'success',
  ERROR = 'error',
  LOADING = 'loading',
  EMPTY = 'empty',
  NOT_FOUND = 'not_found',
}

// Current scenario - can be changed during tests
let currentScenario = MockScenarios.SUCCESS;

export const setMockScenario = (scenario: MockScenarios) => {
  currentScenario = scenario;
  if (MSW_DEBUG) {
    console.log(`[Simple MSW] Scenario changed to: ${scenario}`);
  }
};

// Extract operation name from GraphQL query
const extractOperationName = (query: string): string => {
  const match = query.match(/(?:query|mutation|subscription)\s+(\w+)/);
  return match ? match[1] : 'UnknownOperation';
};

// Get mock response based on operation name, scenario, and variables
const getMockResponse = (operationName: string, scenario: MockScenarios, variables: any = {}) => {
  if (MSW_DEBUG) {
    console.log(`[Simple MSW] Getting response for ${operationName}/${scenario}`, variables);
  }

  try {
    // Map operation names to response files
    const operationMap: Record<string, string> = {
      GetFinancialStatement: 'get_financial_statement',
      GetFinancialDashboard: 'get_financial_dashboard',
      GetFinancialRatios: 'get_financial_ratios',
      GetFinancialAlerts: 'get_financial_alerts',
      GetRatioBenchmarks: 'get_ratio_benchmarks',
      GetSeriesDetail: 'get_series_detail',
      SearchSeriesFulltext: 'search_series_fulltext',
      GetSeriesData: 'get_series_data',
      GetPeerCompanies: 'get_peer_companies',
    };

    const responseFile = operationMap[operationName];
    if (!responseFile) {
      if (MSW_DEBUG) {
        console.log(`[Simple MSW] Unknown operation: ${operationName}`);
      }
      return {
        data: null,
        errors: [{ message: `Unknown operation: ${operationName}` }],
      };
    }

    // Determine scenario based on specific statement IDs for testing
    let finalScenario = scenario;
    if (variables.statementId) {
      if (variables.statementId === 'error-statement-id') {
        finalScenario = MockScenarios.ERROR;
      } else if (variables.statementId === 'empty-statement-id') {
        finalScenario = MockScenarios.EMPTY;
      } else if (variables.statementId === 'loading-statement-id') {
        finalScenario = MockScenarios.LOADING;
      } else if (variables.statementId === 'not-found-statement-id') {
        finalScenario = MockScenarios.NOT_FOUND;
      }
    }

    const response = loadGraphQLResponse(responseFile, finalScenario);

    // Normalize shapes for specific operations to satisfy all tests
    if (operationName === 'GetFinancialAlerts' && response?.data) {
      const companyAlerts = (response.data as any)?.company?.alerts;
      if (companyAlerts && !(response.data as any).financialAlerts) {
        (response.data as any).financialAlerts = companyAlerts;
      }
    }

    if (MSW_DEBUG) {
      console.log(`[Simple MSW] Found response for ${operationName}/${finalScenario}`);
    }
    return response;
  } catch (error) {
    if (MSW_DEBUG) {
      console.log(`[Simple MSW] Error loading response for ${operationName}/${scenario}:`, error);
    }
    return {
      data: null,
      errors: [{ message: `Error loading response: ${error}` }],
    };
  }
};

// Mock fetch to intercept GraphQL requests
// Store original fetch to properly restore it
let originalFetch: typeof global.fetch;

export const setupSimpleMSW = async () => {
  if (MSW_DEBUG) {
    console.log('[Simple MSW] Setting up MSW...');
    console.log('[Simple MSW] Original fetch type:', typeof global.fetch);
  }
  originalFetch = global.fetch;

  // Use vi.spyOn for Vitest instead of jest.spyOn
  const vitestModule = await import('vitest');
  const vi = vitestModule.vi;
  vi.spyOn(global, 'fetch').mockImplementation((input: string | URL | any, options?: any) => {
    const url = typeof input === 'string' ? input : input.toString();
    if (MSW_DEBUG) {
      console.log(`[Simple MSW] ===== FETCH INTERCEPTED =====`);
      console.log(`[Simple MSW] URL: "${url}"`);
      console.log(`[Simple MSW] URL type: ${typeof input}`);
      console.log(`[Simple MSW] Method: ${options?.method || 'GET'}`);
      console.log(`[Simple MSW] Headers:`, options?.headers);
      console.log(`[Simple MSW] Body:`, options?.body);
      console.log(`[Simple MSW] ============================`);
    }

    // Intercept GraphQL requests - be more flexible with URL matching
    if (
      url.includes('graphql') ||
      url === '/graphql' ||
      url.endsWith('/graphql') ||
      url.includes('/graphql')
    ) {
      if (MSW_DEBUG) console.log(`[Simple MSW] *** GRAPHQL REQUEST DETECTED ***`);
      try {
        const body = JSON.parse(options.body);
        const operationName = body.operationName || extractOperationName(body.query);
        const variables = body.variables || {};

        if (MSW_DEBUG) {
          console.log(`[Simple MSW] Operation: ${operationName}`);
          console.log(`[Simple MSW] Variables:`, variables);
          console.log(`[Simple MSW] Current scenario: ${currentScenario}`);
        }

        const response = getMockResponse(operationName, currentScenario, variables);

        if (MSW_DEBUG) {
          console.log(`[Simple MSW] Returning mock response:`, response);
        }

        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve(response),
          status: 200,
          statusText: 'OK',
        } as any);
      } catch (error) {
        console.error('[Simple MSW] Error parsing GraphQL request:', error);
        console.error('[Simple MSW] Request body:', options.body);
        return originalFetch(input, options);
      }
    }

    // For non-GraphQL requests, pass through to original fetch
    if (MSW_DEBUG) console.log(`[Simple MSW] Passing through request: ${url}`);
    return originalFetch(input, options);
  });

  if (MSW_DEBUG) {
    console.log('[Simple MSW] MSW setup complete');
    console.log('[Simple MSW] Fetch mock applied:', typeof global.fetch);
    const vitestModule2 = await import('vitest');
    const vi2 = vitestModule2.vi;
    console.log('[Simple MSW] Vitest mock function:', vi2.isMockFunction(global.fetch));
  }
};

export const cleanupSimpleMSW = async () => {
  // Restore original fetch using vi.restoreAllMocks()
  const vitestModule3 = await import('vitest');
  const vi3 = vitestModule3.vi;
  vi3.restoreAllMocks();

  // Clear any remaining fetch mocks
  if (originalFetch) {
    global.fetch = originalFetch;
  }

  // Clear any remaining handlers
  if (typeof global !== 'undefined' && global.fetch) {
    // Reset fetch to original implementation
    global.fetch = originalFetch || fetch;
  }

  // Clear any remaining timers that might be related to MSW
  for (let i = 1; i < 1000; i++) {
    clearTimeout(i);
    clearInterval(i);
  }

  if (MSW_DEBUG) {
    console.log('[Simple MSW] Cleanup completed');
  }
};
