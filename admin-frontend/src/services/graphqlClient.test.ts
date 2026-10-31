import { vi } from "vitest";
import { requestGraphQL } from "./graphqlClient";

afterEach(() => {
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
