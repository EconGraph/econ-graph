import { vi } from "vitest";
import {
  clearAuthToken,
  getAuthToken,
  isAuthSessionReady,
  requestGraphQL,
  setAuthToken,
} from "./graphqlClient";

afterEach(async () => {
  clearAuthToken();
  await vi.waitFor(() => expect(isAuthSessionReady()).toBe(true));
  vi.unstubAllGlobals();
  localStorage.clear();
});

it("does not replay shared-client mutations even without a caller retry override", async () => {
  const request = vi.fn().mockRejectedValue(new Error("Lost response"));
  vi.stubGlobal("fetch", request);
  await expect(
    requestGraphQL(
      'mutation { triggerCrawl(input:{source:"FRED"}) {enqueuedCount} }',
    ),
  ).rejects.toThrow("Lost response");
  expect(request).toHaveBeenCalledOnce();
});

it("permits polling callers to disable read retries", async () => {
  const request = vi
    .fn()
    .mockResolvedValue(new Response("unavailable", { status: 503 }));
  vi.stubGlobal("fetch", request);
  await expect(
    requestGraphQL("query {crawlerStatus{activeWorkers}}", undefined, {
      retry: false,
    }),
  ).rejects.toThrow("503");
  expect(request).toHaveBeenCalledOnce();
});

it.each([false, true])(
  "does not sign out a new session after an old mutation returns 401 (reused token: %s)",
  async (reuseToken) => {
    const location = { href: "/admin/" };
    vi.stubGlobal("window", { location });
    let finish!: (response: Response) => void;
    const request = vi.fn().mockImplementation(
      () =>
        new Promise<Response>((resolve) => {
          finish = resolve;
        }),
    );
    vi.stubGlobal("fetch", request);
    setAuthToken("operator-a");
    await vi.waitFor(() => expect(isAuthSessionReady()).toBe(true));
    const mutation = requestGraphQL(
      'mutation { triggerCrawl(input:{source:"FRED"}) {enqueuedCount} }',
    );
    const rejected = expect(mutation).rejects.toThrow("401");
    await vi.waitFor(() => expect(request).toHaveBeenCalledOnce());
    expect(request.mock.calls[0][1].headers.authorization).toBe(
      "Bearer operator-a",
    );
    setAuthToken("operator-b");
    await vi.waitFor(() => expect(isAuthSessionReady()).toBe(true));
    if (reuseToken) {
      setAuthToken("operator-a");
      await vi.waitFor(() => expect(isAuthSessionReady()).toBe(true));
    }
    finish(new Response("unauthorized", { status: 401 }));
    await rejected;
    expect(getAuthToken()).toBe(reuseToken ? "operator-a" : "operator-b");
    expect(location.href).toBe("/admin/");
    expect(request).toHaveBeenCalledOnce();
  },
);

it("signs out the current session when its mutation returns 401", async () => {
  const location = { href: "/admin/" };
  vi.stubGlobal("window", { location });
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(new Response("unauthorized", { status: 401 })),
  );
  setAuthToken("current-operator");
  await vi.waitFor(() => expect(isAuthSessionReady()).toBe(true));
  await expect(
    requestGraphQL(
      'mutation { triggerCrawl(input:{source:"FRED"}) {enqueuedCount} }',
    ),
  ).rejects.toThrow("401");
  expect(getAuthToken()).toBeNull();
  expect(location.href).toBe("/login");
});
