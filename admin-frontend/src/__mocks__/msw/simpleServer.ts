/**
 * Simplified MSW server setup to avoid import issues
 *
 * This version uses dynamic imports and simpler setup to avoid
 * TextEncoder issues in the Jest environment.
 */

// Import GraphQL mock responses from directory structure
import getCrawlerConfigSuccess from "../graphql/getCrawlerConfig/success.json";
import getCrawlerConfigError from "../graphql/getCrawlerConfig/error.json";
import getCrawlerConfigLoading from "../graphql/getCrawlerConfig/loading.json";
import getDataSourcesSuccess from "../graphql/getDataSources/success.json";
import getDataSourcesError from "../graphql/getDataSources/error.json";
import getDataSourcesEmpty from "../graphql/getDataSources/empty.json";
import updateCrawlerConfigSuccess from "../graphql/updateCrawlerConfig/success.json";
import updateCrawlerConfigError from "../graphql/updateCrawlerConfig/error.json";
import updateDataSourceSuccess from "../graphql/updateDataSource/success.json";
import updateDataSourceError from "../graphql/updateDataSource/error.json";
import testDataSourceConnectionSuccess from "../graphql/testDataSourceConnection/success.json";
import testDataSourceConnectionError from "../graphql/testDataSourceConnection/error.json";

// Mock scenarios for testing different states
export enum MockScenarios {
  SUCCESS = "success",
  ERROR = "error",
  LOADING = "loading",
  UNAUTHORIZED = "unauthorized",
}

// Simple mock responses using imported GraphQL mock files
export const mockResponses = {
  GetCrawlerConfig: {
    success: getCrawlerConfigSuccess,
    error: getCrawlerConfigError,
    loading: getCrawlerConfigLoading,
  },

  GetDataSources: {
    success: getDataSourcesSuccess,
    error: getDataSourcesError,
    empty: getDataSourcesEmpty,
  },

  UpdateCrawlerConfig: {
    success: updateCrawlerConfigSuccess,
    error: updateCrawlerConfigError,
  },

  UpdateDataSource: {
    success: updateDataSourceSuccess,
    error: updateDataSourceError,
  },

  TestDataSourceConnection: {
    success: testDataSourceConnectionSuccess,
    error: testDataSourceConnectionError,
  },
};

// Simple scenario management
let currentScenario = "default";

export const setMockScenario = (scenario: string) => {
  currentScenario = scenario;
  console.log(`[Simple MSW] Set mock scenario to: ${scenario}`);
};

export const resetMockScenario = () => {
  currentScenario = "default";
  console.log(`[Simple MSW] Reset mock scenario to default`);
};

// Simple GraphQL response handler
export const getMockResponse = (
  operationName: string,
  scenario: string = currentScenario,
) => {
  console.log(
    `[Simple MSW] Looking for operation: ${operationName} with scenario: ${scenario}`,
  );
  console.log(`[Simple MSW] Available operations:`, Object.keys(mockResponses));

  const responses = mockResponses[operationName as keyof typeof mockResponses];
  if (!responses) {
    console.error(`[Simple MSW] Unknown operation: ${operationName}`);
    return {
      data: null,
      errors: [{ message: `Unknown operation: ${operationName}` }],
    };
  }

  console.log(
    `[Simple MSW] Available scenarios for ${operationName}:`,
    Object.keys(responses),
  );

  const response = responses[scenario as keyof typeof responses];
  if (!response) {
    console.error(
      `[Simple MSW] Unknown scenario: ${scenario} for operation: ${operationName}`,
    );
    return {
      data: null,
      errors: [{ message: `Unknown scenario: ${scenario}` }],
    };
  }

  console.log(`[Simple MSW] Found response for ${operationName}/${scenario}`);
  return response;
};

// Mock fetch to intercept GraphQL requests
// Store original fetch to properly restore it
let originalFetch: typeof global.fetch;

export const setupSimpleMSW = () => {
  console.log("[Simple MSW] Setting up MSW...");
  console.log("[Simple MSW] Original fetch type:", typeof global.fetch);
  originalFetch = global.fetch;

  // Use jest.spyOn instead of direct assignment to ensure proper mocking
  jest
    .spyOn(global, "fetch")
    .mockImplementation((input: string | URL | Request, options?: any) => {
      const url = typeof input === "string" ? input : input.toString();
      console.log(`[Simple MSW] ===== FETCH INTERCEPTED =====`);
      console.log(`[Simple MSW] URL: "${url}"`);
      console.log(`[Simple MSW] Method: ${options?.method || "GET"}`);
      console.log(`[Simple MSW] Headers:`, options?.headers);
      console.log(`[Simple MSW] Body:`, options?.body);
      console.log(`[Simple MSW] ============================`);

      // Intercept GraphQL requests - be more flexible with URL matching
      if (
        url.includes("graphql") ||
        url === "/graphql" ||
        url.endsWith("/graphql")
      ) {
        console.log(`[Simple MSW] *** GRAPHQL REQUEST DETECTED ***`);
        try {
          const body = JSON.parse(options.body);
          const operationName = body.operationName;
          const variables = body.variables || {};

          console.log(`[Simple MSW] Operation: ${operationName}`);
          console.log(`[Simple MSW] Variables:`, variables);
          console.log(`[Simple MSW] Current scenario: ${currentScenario}`);

          const response = getMockResponse(operationName, currentScenario);

          console.log(`[Simple MSW] Returning mock response:`, response);

          return Promise.resolve({
            ok: true,
            json: () => Promise.resolve(response),
            status: 200,
            statusText: "OK",
          } as Response);
        } catch (error) {
          console.error("[Simple MSW] Error parsing GraphQL request:", error);
          console.error("[Simple MSW] Request body:", options.body);
          return originalFetch(url, options);
        }
      }

      // For non-GraphQL requests, pass through to original fetch
      console.log(`[Simple MSW] Passing through request: ${url}`);
      return originalFetch(url, options);
    });

  console.log("[Simple MSW] MSW setup complete");
  console.log("[Simple MSW] Fetch mock applied:", typeof global.fetch);
  console.log(
    "[Simple MSW] Jest mock function:",
    jest.isMockFunction(global.fetch),
  );
};

export const cleanupSimpleMSW = () => {
  // Restore original fetch using jest.restoreAllMocks()
  jest.restoreAllMocks();
};
