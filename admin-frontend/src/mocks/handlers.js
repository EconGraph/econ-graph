/**
 * MSW handlers for Jest tests
 * Based on MSW examples: https://github.com/mswjs/examples/blob/main/examples/with-jest/
 */

import { http } from "msw";

export const handlers = [
  http.post("/graphql", ({ request }) => {
    console.log("[MSW] GraphQL request intercepted");
    return new Response(
      JSON.stringify({
        data: {
          crawlerLogs: [
            {
              id: "log1",
              timestamp: "2024-01-15T10:00:00Z",
              level: "info",
              source: "FRED",
              message: "Successfully crawled GDP data",
              duration: 2.3,
              status: "success",
            },
          ],
        },
      }),
      {
        status: 200,
        headers: {
          "Content-Type": "application/json",
        },
      },
    );
  }),
];
