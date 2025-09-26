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
import getCrawlerStatusSuccess from "../graphql/getCrawlerStatus/success.json";
import getCrawlerStatusError from "../graphql/getCrawlerStatus/error.json";
import getCrawlerStatusLoading from "../graphql/getCrawlerStatus/loading.json";
import getQueueStatisticsSuccess from "../graphql/getQueueStatistics/success.json";
import getQueueStatisticsError from "../graphql/getQueueStatistics/error.json";
import getPerformanceMetricsSuccess from "../graphql/getPerformanceMetrics/success.json";
import getPerformanceMetricsError from "../graphql/getPerformanceMetrics/error.json";
import getSystemHealthSuccess from "../graphql/getSystemHealth/success.json";
import getSystemHealthError from "../graphql/getSystemHealth/error.json";
import getSystemHealthLoading from "../graphql/getSystemHealth/loading.json";
import getCrawlerLogsSuccess from "../graphql/getCrawlerLogs/success.json";
import getCrawlerLogsError from "../graphql/getCrawlerLogs/error.json";
import getCrawlerLogsLoading from "../graphql/getCrawlerLogs/loading.json";
import searchLogsSuccess from "../graphql/searchLogs/success.json";
import searchLogsError from "../graphql/searchLogs/error.json";
import getUsersSuccess from "../graphql/getUsers/success.json";
import getOnlineUsersSuccess from "../graphql/getOnlineUsers/success.json";
import getOnlineUsersBothUsers from "../graphql/getOnlineUsers/both-users.json";
import createUserSuccess from "../graphql/createUser/success.json";
import createUserError from "../graphql/createUser/error.json";
import updateUserSuccess from "../graphql/updateUser/success.json";
import updateUserError from "../graphql/updateUser/error.json";
import deleteUserSuccess from "../graphql/deleteUser/success.json";
import deleteUserError from "../graphql/deleteUser/error.json";

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

  GetCrawlerStatus: {
    success: getCrawlerStatusSuccess,
    error: getCrawlerStatusError,
    loading: getCrawlerStatusLoading,
  },

  GetQueueStatistics: {
    success: getQueueStatisticsSuccess,
    error: getQueueStatisticsError,
  },

  GetPerformanceMetrics: {
    success: getPerformanceMetricsSuccess,
    error: getPerformanceMetricsError,
  },

  GetSystemHealth: {
    success: getSystemHealthSuccess,
    error: getSystemHealthError,
    loading: getSystemHealthLoading,
  },
  GetCrawlerLogs: {
    success: getCrawlerLogsSuccess,
    error: getCrawlerLogsError,
    loading: getCrawlerLogsLoading,
  },
  SearchLogs: {
    success: searchLogsSuccess,
    error: searchLogsError,
  },

  GetUsers: {
    success: getUsersSuccess,
  },

  GetOnlineUsers: {
    success: getOnlineUsersSuccess,
    "both-users": getOnlineUsersBothUsers,
  },

  CreateUser: {
    success: createUserSuccess,
    error: createUserError,
  },

  UpdateUser: {
    success: updateUserSuccess,
    error: updateUserError,
  },

  DeleteUser: {
    success: deleteUserSuccess,
    error: deleteUserError,
  },
};

// Helper function to extract operation name from GraphQL query
const extractOperationName = (query: string): string => {
  // Match patterns like "query GetUsers" or "mutation UpdateUser"
  const match = query.match(/(?:query|mutation|subscription)\s+(\w+)/);
  return match ? match[1] : "Unknown";
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

export const setupSimpleMSW = async () => {
  console.log("[Simple MSW] Setting up MSW...");
  console.log("[Simple MSW] Original fetch type:", typeof global.fetch);
  originalFetch = global.fetch;

  // Use vi.spyOn for Vitest instead of jest.spyOn
  const vitestModule = await import("vitest");
  const vi = vitestModule.vi;
  vi.spyOn(global, "fetch").mockImplementation(
    (input: string | URL | Request, options?: any) => {
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
          const operationName =
            body.operationName || extractOperationName(body.query);
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
    },
  );

  console.log("[Simple MSW] MSW setup complete");
  console.log("[Simple MSW] Fetch mock applied:", typeof global.fetch);
  const vitestModule2 = await import("vitest");
  const vi2 = vitestModule2.vi;
  console.log(
    "[Simple MSW] Vitest mock function:",
    vi2.isMockFunction(global.fetch),
  );
};

export const cleanupSimpleMSW = async () => {
  // Restore original fetch using vi.restoreAllMocks()
  const vitestModule3 = await import("vitest");
  const vi3 = vitestModule3.vi;
  vi3.restoreAllMocks();
};
