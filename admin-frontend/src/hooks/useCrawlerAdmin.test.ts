import { vi } from "vitest";
import { crawlerRequest } from "./useCrawlerAdmin";

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});
it("uses the configured authenticated contract without silently dropping GraphQL errors", async () => {
  localStorage.setItem("auth_token", "verified-access-token");
  const request = vi.fn().mockResolvedValue({
    ok: true,
    json: async () => ({ errors: [{ message: "Insufficient permissions" }] }),
  });
  vi.stubGlobal("fetch", request);
  await expect(crawlerRequest("query {crawlerJobs{id}}")).rejects.toThrow(
    "Insufficient permissions",
  );
  const [endpoint, options] = request.mock.calls[0];
  expect(endpoint).toBe("/graphql");
  expect(options.credentials).toBe("include");
  expect(options.headers.Authorization).toBe("Bearer verified-access-token");
  expect(JSON.parse(options.body).query).toContain("crawlerJobs");
});
it("rejects transport failures and missing data", async () => {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false, status: 401 }));
  await expect(crawlerRequest("query {crawlerJobs{id}}")).rejects.toThrow(
    "401",
  );
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({ ok: true, json: async () => ({}) }),
  );
  await expect(crawlerRequest("query {crawlerJobs{id}}")).rejects.toThrow(
    "no data",
  );
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
    vi.fn().mockResolvedValue({ ok: true, json: async () => ({ data: {} }) }),
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
