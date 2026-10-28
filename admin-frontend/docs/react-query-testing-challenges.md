# Testing React Query in the admin frontend

The admin frontend uses TanStack Query v5, Vitest, jsdom and MSW. Test the initial loading state separately from the completed request: seeing an intercepted request or a loading indicator does not establish that the hook returned data or that the component rendered it.

## Use the existing setup

[Vitest configuration](../vitest.config.ts) selects jsdom and loads [setupTests.ts](../src/setupTests.ts). That setup starts the [MSW server](../src/mocks/server.ts) before tests, resets handler overrides after each test and closes the server afterward. Test files can call `server.use(...)` to override a response; they do not need to start another server.

[The test utilities](../src/test-utils.tsx) export `createTestQueryClient`, `TestWrapper` and `renderWithQueryClient`. The factory disables query and mutation retries and sets `gcTime` and `staleTime` to zero. These settings avoid retry delays and remove inactive queries promptly; active queries still use a cache. Create one client per test and keep that client stable throughout renders. The example below uses the factory directly so the wrapper closes over the same client.

Use real timers for tests that only need to await a response. The [crawler handlers](../src/mocks/handlers.js) delay responses by 50 ms, and Query schedules observer notifications through timers. With fake timers, an intercepted request can still be waiting for its handler's delay, or a completed query can be waiting to notify React. An unadvanced timer in the test itself can also prevent an awaited promise from completing. None of these observations establishes a failure in Query's promise handling.

## Await success and failure

Save this example as `admin-frontend/src/crawler-query.test.tsx`. It uses the real [crawler status hook](../src/hooks/useCrawlerData.ts) with polling disabled and the existing MSW fixture. The second test replaces the response with a GraphQL error. The global setup owns the server lifecycle; this file owns React cleanup and its query clients.

```tsx
import React from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";
import { createTestQueryClient } from "./test-utils";
import { useCrawlerStatus } from "./hooks/useCrawlerData";
import { server } from "./mocks/server";

let client: ReturnType<typeof createTestQueryClient>;

beforeEach(() => {
  vi.useRealTimers();
  client = createTestQueryClient();
});

afterEach(() => {
  cleanup();
  client.clear();
});

function Wrapper({ children }: { children: React.ReactNode }) {
  return (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

it("returns crawler status after loading", async () => {
  const { result } = renderHook(() => useCrawlerStatus(false), {
    wrapper: Wrapper,
  });

  expect(result.current.loading).toBe(true);
  await waitFor(() => expect(result.current.loading).toBe(false));
  expect(result.current.error).toBeNull();
  expect(result.current.status).toMatchObject({
    is_running: true,
    current_task: "Processing FRED data",
    active_workers: 3,
  });
});

it("exposes a GraphQL error after loading", async () => {
  server.use(
    http.post("/graphql", () =>
      HttpResponse.json({ errors: [{ message: "Crawler unavailable" }] }),
    ),
  );
  const { result } = renderHook(() => useCrawlerStatus(false), {
    wrapper: Wrapper,
  });

  await waitFor(() =>
    expect(result.current.error?.message).toBe("Crawler unavailable"),
  );
  expect(result.current.loading).toBe(false);
  expect(result.current.status).toBeUndefined();
});
```

From the repository root, use the Node version required by [package.json](../package.json), install dependencies with `npm ci` in `admin-frontend`, then run only this file:

```bash
cd admin-frontend
npm run test:run -- src/crawler-query.test.tsx
```

These assertions verify response parsing and the hook's loading, data and error values. They do not verify dashboard presentation. A component test should render the actual component with a stable client, assert its initial loading indicator, then await an element that requires resolved data with `findBy*` or `waitFor`. For example, awaiting the dashboard title and checking that loading disappeared establishes that the dashboard left loading; checking individual fields requires additional assertions. A hook error assertion does not establish that the component displays an error message.

## Choose the test boundary

| Boundary | What it can establish |
| --- | --- |
| Network interception with MSW and real hooks | Request construction, response parsing and query-driven state changes, when the test awaits and asserts those results. |
| Hook mocking with `vi.mock` | Component behavior for supplied loading, data and error values. It does not exercise HTTP requests or Query. |
| Component isolation with mocked children | The parent's rendering and interactions at that boundary. It does not establish the children's behavior or a complete data flow. |

Keep loading coverage, but give resolved-data and failure assertions their own observable outcomes. Repeating a synchronous loading assertion does not prove that a request completed or that a query remained stuck.

## Diagnose a test that stays loading

First check the request URL, method and GraphQL operation against the handler. The hook fetches `/graphql`; the fetch implementation must resolve that relative URL against the jsdom origin. A Node fetch error about parsing `/graphql` is a URL setup problem. Inspect the response body as well: the hook expects `data.crawlerStatus` and throws when the response contains `errors` or has a failing HTTP status.

Next inspect the query state through the test's client and compare it with the hook value and rendered output. Request interception, successful query state and rendered data are separate observations. Confirm that renders use the same client and that retries are disabled before interpreting an error test's timeout.

Finally check timers and assertions. If fake timers are needed to test polling, deliberately advance the handler delays, polling intervals and notification callbacks inside React's `act`, and restore real timers during cleanup. Awaiting a timer without advancing it can hang the test. Use awaited `findBy*` queries for elements that appear later or `waitFor` with an assertion that throws until the expected state arrives; declaring the test `async` alone does not wait for a response.

## References

- [TanStack Query testing guide](https://tanstack.com/query/latest/docs/framework/react/guides/testing): isolated clients, disabled retries and awaited hook results with mocked network calls.
- [Testing Library async methods](https://testing-library.com/docs/dom-testing-library/api-async/): `findBy*`, `waitFor` and their promise-based assertions.
