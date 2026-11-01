import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { auditPage } from "./audit";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import App from "../../App";
import { queryClient } from "../../lib/queryClient";
import { OVERVIEW } from "../../hooks/useCrawlerAdmin";

// Stub only the transport boundary. The app, providers, hooks, tabs and
// dashboard markup remain real. Unexpected requests fail the test.
beforeEach(() => {
  queryClient.clear();
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, options: RequestInit) => {
      expect(url).toBe("/graphql");
      expect(
        JSON.parse(String(options.body)).query.replace(/\s|__typename/g, ""),
      ).toBe(OVERVIEW.replace(/\s/g, ""));
      return new Response(
        JSON.stringify({
          data: {
            crawlerStatus: {
              isRunning: false,
              activeWorkers: 0,
              lastCrawl: null,
              nextScheduledCrawl: null,
              sources: [],
            },
            queueStatistics: {
              totalItems: 0,
              completedItems: 0,
              pendingItems: 0,
              processingItems: 0,
              failedItems: 0,
              retryingItems: 0,
            },
            crawlerSources: [],
          },
        }),
        { headers: { "Content-Type": "application/json" } },
      );
    }),
  );
});
afterEach(() => {
  queryClient.clear();
  vi.unstubAllGlobals();
});

describe("App accessibility", () => {
  it("audits the loaded crawler dashboard and navigation", async () => {
    render(
      <ThemeProvider theme={createTheme()}>
        <App />
      </ThemeProvider>,
    );
    expect(
      await screen.findByRole("table", { name: "Queue counts" }),
    ).toBeVisible();
    expect(
      screen.getByRole("heading", { name: "Crawler queue" }),
    ).toBeVisible();
    expect(
      screen.getByRole("tab", { name: "Dashboard", selected: true }),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: /refresh/i })).toBeVisible();
    await auditPage("dashboard");
  });
});
