/**
 * MSW Server Setup for Testing
 *
 * This creates a proper MSW server that can intercept GraphQL requests
 * and return mock responses from our directory-based structure.
 */

import { setupServer } from "msw/node";
import { http, HttpResponse } from "msw";

// Import GraphQL mock responses from directory structure
import getCrawlerConfigSuccess from "../graphql/getCrawlerConfig/success.json";
import getCrawlerConfigError from "../graphql/getCrawlerConfig/error.json";
import getCrawlerConfigLoading from "../graphql/getCrawlerConfig/loading.json";
import getDataSourcesSuccess from "../graphql/getDataSources/success.json";
import getDataSourcesError from "../graphql/getDataSources/error.json";
import getDataSourcesEmpty from "../graphql/getDataSources/empty.json";
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

// Track current scenario for dynamic responses
let currentScenario = "success";

export const setMockScenario = (scenario: string) => {
  currentScenario = scenario;
  console.log(`[MSW Server] Set scenario to: ${scenario}`);
};

export const resetMockScenario = () => {
  currentScenario = "success";
  console.log(`[MSW Server] Reset scenario to: ${currentScenario}`);
};

// Define handlers for GraphQL endpoints
const handlers = [
  // GraphQL endpoint handler
  http.post("/graphql", async ({ request }) => {
    console.log(`[MSW Server] ===== GRAPHQL REQUEST INTERCEPTED =====`);

    try {
      const body = (await request.json()) as any;
      const { operationName, variables } = body;

      console.log(`[MSW Server] Operation: ${operationName}`);
      console.log(`[MSW Server] Variables:`, variables);
      console.log(`[MSW Server] Current scenario: ${currentScenario}`);

      let response;

      switch (operationName) {
        case "GetCrawlerConfig":
          switch (currentScenario) {
            case "error":
              response = getCrawlerConfigError;
              break;
            case "loading":
              response = getCrawlerConfigLoading;
              break;
            default:
              response = getCrawlerConfigSuccess;
          }
          break;

        case "GetDataSources":
          switch (currentScenario) {
            case "error":
              response = getDataSourcesError;
              break;
            case "empty":
              response = getDataSourcesEmpty;
              break;
            default:
              response = getDataSourcesSuccess;
          }
          break;

        case "GetCrawlerStatus":
          switch (currentScenario) {
            case "error":
              response = getCrawlerStatusError;
              break;
            case "loading":
              response = getCrawlerStatusLoading;
              break;
            default:
              response = getCrawlerStatusSuccess;
          }
          break;

        case "GetQueueStatistics":
          switch (currentScenario) {
            case "error":
              response = getQueueStatisticsError;
              break;
            default:
              response = getQueueStatisticsSuccess;
          }
          break;

        case "GetPerformanceMetrics":
          switch (currentScenario) {
            case "error":
              response = getPerformanceMetricsError;
              break;
            default:
              response = getPerformanceMetricsSuccess;
          }
          break;

        case "GetSystemHealth":
          switch (currentScenario) {
            case "error":
              response = getSystemHealthError;
              break;
            case "loading":
              response = getSystemHealthLoading;
              break;
            default:
              response = getSystemHealthSuccess;
          }
          break;

        case "GetCrawlerLogs":
          switch (currentScenario) {
            case "error":
              response = getCrawlerLogsError;
              break;
            case "loading":
              response = getCrawlerLogsLoading;
              break;
            default:
              response = getCrawlerLogsSuccess;
          }
          break;

        case "SearchLogs":
          switch (currentScenario) {
            case "error":
              response = searchLogsError;
              break;
            default:
              response = searchLogsSuccess;
          }
          break;

        default:
          console.error(`[MSW Server] Unknown operation: ${operationName}`);
          response = {
            data: null,
            errors: [{ message: `Unknown operation: ${operationName}` }],
          };
      }

      console.log(`[MSW Server] Returning response:`, response);
      console.log(`[MSW Server] ==========================================`);

      return HttpResponse.json(response);
    } catch (error) {
      console.error("[MSW Server] Error processing GraphQL request:", error);
      return HttpResponse.json(
        { data: null, errors: [{ message: "Failed to process request" }] },
        { status: 400 },
      );
    }
  }),
];

// Create and export the MSW server
export const server = setupServer(...handlers);
