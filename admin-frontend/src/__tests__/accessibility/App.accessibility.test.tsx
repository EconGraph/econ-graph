import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { auditPage } from "./audit";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import App from "../../App";
import { queryClient } from "../../lib/queryClient";

// Stub only the transport boundary. The app, providers, hooks, tabs and
// dashboard markup remain real. Unexpected requests fail the test.
beforeEach(() => {
  queryClient.clear();
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      expect(url).toBe("/graphql");
      return new Response(
        JSON.stringify({
          data: {
            crawlerStatus: { is_running: false, active_workers: 0 },
            queueStatistics: {
              total_items: 0,
              completed_items: 0,
              pending_items: 0,
              processing_items: 0,
              failed_items: 0,
              retrying_items: 0,
            },
            crawlerLogs: [],
            performanceMetrics: [],
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
    expect(await screen.findByTestId("dashboard-title")).toBeVisible();
    expect(
      screen.getByRole("tab", { name: "Dashboard", selected: true }),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: /refresh/i })).toBeVisible();
    await auditPage("dashboard");
  });
});
