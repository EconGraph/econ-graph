/**
 * Debug test to verify MSW is working properly
 * Uses the global MSW server from setupTests.ts
 */

import { server } from "../../mocks/server";
import { http } from "msw";

describe("MSW Debug", () => {
  it("should intercept GraphQL requests with MSW", async () => {
    console.log("Testing MSW GraphQL interception...");

    const response = await fetch("/graphql", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        query: "query GetCrawlerLogs { crawlerLogs { id message } }",
        operationName: "GetCrawlerLogs",
      }),
    });

    console.log("Response status:", response.status);
    console.log("Response ok:", response.ok);

    const data = await response.json();
    console.log("Response data:", data);

    expect(response.ok).toBe(true);
    expect(data.data).toBeDefined();
    expect(data.data.crawlerLogs).toBeDefined();
    expect(data.data.crawlerLogs[0].message).toBe(
      "Successfully crawled GDP data",
    );
  });

  it("should handle error scenarios with MSW", async () => {
    // Override the handler to return an error
    server.use(
      http.post("/graphql", () => {
        console.log("[MSW] Error scenario triggered");
        return new Response(
          JSON.stringify({
            errors: [
              {
                message: "GraphQL error occurred",
                extensions: { code: "INTERNAL_SERVER_ERROR" },
              },
            ],
          }),
          {
            status: 500,
            headers: {
              "Content-Type": "application/json",
            },
          },
        );
      }),
    );

    const response = await fetch("/graphql", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        query: "query GetCrawlerLogs { crawlerLogs { id message } }",
        operationName: "GetCrawlerLogs",
      }),
    });

    console.log("Error response status:", response.status);
    const data = await response.json();
    console.log("Error response data:", data);

    expect(response.ok).toBe(false);
    expect(data.errors).toBeDefined();
    expect(data.errors[0].message).toBe("GraphQL error occurred");
  });
});
