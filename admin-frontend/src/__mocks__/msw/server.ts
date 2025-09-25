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
      const body = await request.json() as any;
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
