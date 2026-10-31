import { vi } from "vitest";
import { crawlerRequest } from "./useCrawlerAdmin";

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});
it("uses the configured authenticated contract without silently dropping GraphQL errors", async () => {
  localStorage.setItem("auth_token", "verified-access-token");
  const request = vi
    .fn()
    .mockResolvedValue(
      new Response(
        JSON.stringify({ errors: [{ message: "Insufficient permissions" }] }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );
  vi.stubGlobal("fetch", request);
  await expect(crawlerRequest("query {crawlerJobs{id}}")).rejects.toThrow(
    "Insufficient permissions",
  );
  const [endpoint, options] = request.mock.calls[0];
  expect(endpoint).toBe("/graphql");
  expect(options.credentials).toBe("include");
  expect(options.headers.authorization).toBe("Bearer verified-access-token");
  expect(JSON.parse(options.body).query).toContain("crawlerJobs");
});
it("clears expired credentials on HTTP 401 and refuses missing GraphQL data", async () => {
  localStorage.setItem("auth_token", "expired-token");
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(
      new Response("{}", {
        status: 401,
        headers: { "Content-Type": "application/json" },
      }),
    ),
  );
  await expect(crawlerRequest("query {crawlerJobs{id}}")).rejects.toThrow(
    "401",
  );
  expect(localStorage.getItem("auth_token")).toBeNull();
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(
      new Response("{}", {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    ),
  );
  await expect(crawlerRequest("query {crawlerJobs{id}}")).rejects.toThrow();
});

it("does not turn a malformed overview into fabricated zero counts", async () => {
  const { createElement } = await import("react");
  const { renderHook, waitFor } = await import("@testing-library/react");
  const { QueryClient, QueryClientProvider } =
    await import("@tanstack/react-query");
  const { useCrawlerOverview } = await import("./useCrawlerAdmin");
  const client = new QueryClient();
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ data: {} }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    ),
  );
  const hook = renderHook(() => useCrawlerOverview(), {
    wrapper: ({ children }) =>
      createElement(QueryClientProvider, { client }, children),
  });
  await waitFor(() =>
    expect(hook.result.current.error?.message).toContain(
      "no valid queue snapshot",
    ),
  );
  expect(hook.result.current.data).toBeUndefined();
  hook.unmount();
  client.clear();
});

it("never retries an enqueue after an ambiguous transport failure", async () => {
  const { createElement } = await import("react");
  const { renderHook, act } = await import("@testing-library/react");
  const { QueryClient, QueryClientProvider } =
    await import("@tanstack/react-query");
  const { useCrawlerActions } = await import("./useCrawlerAdmin");
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: 1 } },
  });
  const request = vi.fn().mockRejectedValue(new Error("Connection lost"));
  vi.stubGlobal("fetch", request);
  const hook = renderHook(() => useCrawlerActions(), {
    wrapper: ({ children }) =>
      createElement(QueryClientProvider, { client }, children),
  });
  await act(async () => {
    await expect(
      hook.result.current.enqueue.mutateAsync({
        source: "FRED",
        seriesIds: ["GDP"],
        priority: 5,
      }),
    ).rejects.toThrow("Connection lost");
  });
  expect(request).toHaveBeenCalledOnce();
  hook.unmount();
  client.clear();
});

it("never renders a previous operator's cached overview after changing identity", async () => {
  const { createElement } = await import("react");
  const { render, screen, waitFor, act } =
    await import("@testing-library/react");
  const { QueryClientProvider } = await import("@tanstack/react-query");
  const { queryClient } = await import("../lib/queryClient");
  const { setAuthToken, clearAuthToken } =
    await import("../services/graphqlClient");
  const { useCrawlerOverview } = await import("./useCrawlerAdmin");
  const snapshot = {
    crawlerStatus: { activeWorkers: 23 },
    queueStatistics: {
      pendingItems: 1,
      processingItems: 0,
      completedItems: 0,
      failedItems: 0,
      retryingItems: 0,
    },
    crawlerSources: [],
  };
  let deny: ((response: Response) => void) | undefined;
  const request = vi
    .fn()
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ data: snapshot }), {
        headers: { "Content-Type": "application/json" },
      }),
    )
    .mockImplementationOnce(
      () =>
        new Promise<Response>((resolve) => {
          deny = resolve;
        }),
    );
  vi.stubGlobal("fetch", request);
  setAuthToken("operator-a-token");
  function OverviewProbe() {
    const overview = useCrawlerOverview();
    return createElement(
      "p",
      null,
      overview.data
        ? `workers:${overview.data.crawlerStatus.activeWorkers}`
        : overview.error?.message || "no data",
    );
  }
  const view = render(
    createElement(
      QueryClientProvider,
      { client: queryClient },
      createElement(OverviewProbe),
    ),
  );
  await waitFor(() =>
    expect(screen.getByText("workers:23")).toBeInTheDocument(),
  );
  await act(async () => {
    setAuthToken("denied-operator-token");
  });
  expect(screen.queryByText("workers:23")).not.toBeInTheDocument();
  await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
  expect(
    JSON.stringify(
      queryClient
        .getQueryCache()
        .getAll()
        .map((query) => query.queryKey),
    ),
  ).not.toContain("operator-token");
  await act(async () => {
    deny!(
      new Response(
        JSON.stringify({ errors: [{ message: "Insufficient permissions" }] }),
        { headers: { "Content-Type": "application/json" } },
      ),
    );
  });
  await waitFor(() =>
    expect(screen.getByText("Insufficient permissions")).toBeInTheDocument(),
  );
  expect(screen.queryByText("workers:23")).not.toBeInTheDocument();
  view.unmount();
  clearAuthToken();
  queryClient.clear();
});

it("drops a previous operator's mutation result when the session changes", async () => {
  const { createElement } = await import("react");
  const { renderHook, act, waitFor } = await import("@testing-library/react");
  const { QueryClient, QueryClientProvider } =
    await import("@tanstack/react-query");
  const { setAuthToken, clearAuthToken, isAuthSessionReady } =
    await import("../services/graphqlClient");
  const { useCrawlerActions } = await import("./useCrawlerAdmin");
  setAuthToken("operator-a");
  await waitFor(() => expect(isAuthSessionReady()).toBe(true));
  const client = new QueryClient();
  vi.stubGlobal(
    "fetch",
    vi
      .fn()
      .mockResolvedValue(
        new Response(
          JSON.stringify({ data: { triggerCrawl: { enqueuedCount: 9 } } }),
          { headers: { "Content-Type": "application/json" } },
        ),
      ),
  );
  const hook = renderHook(() => useCrawlerActions(), {
    wrapper: ({ children }) =>
      createElement(QueryClientProvider, { client }, children),
  });
  await act(async () => {
    await hook.result.current.enqueue.mutateAsync({
      source: "FRED",
      seriesIds: ["GDP"],
      priority: 5,
    });
  });
  await waitFor(() =>
    expect(hook.result.current.enqueue.data?.triggerCrawl.enqueuedCount).toBe(
      9,
    ),
  );
  await act(async () => {
    setAuthToken("operator-b");
  });
  expect(hook.result.current.enqueue.data).toBeUndefined();
  expect(hook.result.current.enqueue.error).toBeNull();
  hook.unmount();
  clearAuthToken();
  client.clear();
});
