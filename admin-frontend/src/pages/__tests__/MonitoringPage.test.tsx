// REQUIREMENT: Comprehensive tests for MonitoringPage component
// PURPOSE: Ensure monitoring page integrates correctly with Grafana dashboards and displays metrics
// This validates the monitoring interface works with our existing Grafana infrastructure

import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import MonitoringPage from "../MonitoringPage";

import { vi } from "vitest";

// Set timeout for all tests in this file due to performance characteristics
// TODO: Optimize MonitoringPage component performance to reduce test timeouts
vi.setConfig({ testTimeout: 60000 });

// Mock the contexts to prevent resource leaks
vi.mock("../../contexts/AuthContext", () => ({
  AuthProvider: ({ children }: any) => children,
  useAuth: () => ({
    user: {
      id: "test-user",
      username: "admin",
      role: "super_admin",
      sessionExpiry: new Date(Date.now() + 3600000).toISOString(),
    },
    isAuthenticated: true,
    login: vi.fn(),
    logout: vi.fn(),
    refreshSession: vi.fn(),
    extendSession: vi.fn(),
  }),
}));

vi.mock("../../contexts/SecurityContext", () => ({
  SecurityProvider: ({ children }: any) => children,
  useSecurity: () => ({
    checkAccess: vi.fn(() => true),
    logSecurityEvent: vi.fn(),
    securityEvents: [],
    sessionRemainingTime: 3661, // 61 minutes and 1 second in seconds
  }),
}));

// Mock React Query to prevent "No QueryClient set" errors
vi.mock("@tanstack/react-query", () => ({
  QueryClientProvider: ({ children }: any) => children,
  useQuery: vi.fn(() => ({
    data: undefined,
    isLoading: false,
    error: null,
    refetch: vi.fn(),
  })),
  useMutation: vi.fn(() => ({
    mutate: vi.fn(),
    mutateAsync: vi.fn(),
    isLoading: false,
    error: null,
  })),
  useQueryClient: vi.fn(() => ({
    invalidateQueries: vi.fn(),
    setQueryData: vi.fn(),
  })),
}));

// Mock timers to prevent resource leaks
vi.useFakeTimers();

// Mock setTimeout to run immediately in tests
vi.spyOn(global, "setTimeout").mockImplementation((fn: any, delay?: number) => {
  if (typeof fn === "function") {
    fn();
  }
  return 1 as any;
});

// Mock setInterval to prevent resource leaks
vi.spyOn(global, "setInterval").mockImplementation(
  (fn: any, delay?: number) => {
    if (typeof fn === "function") {
      fn();
    }
    return 1 as any;
  },
);

// Create a test theme
const theme = createTheme();

// Test wrapper component
const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <BrowserRouter>
    <ThemeProvider theme={theme}>{children}</ThemeProvider>
  </BrowserRouter>
);

describe("MonitoringPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Rendering", () => {
    it("renders monitoring page with correct title", () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      // First check if the main content renders
      expect(screen.getByTestId("monitoring-content")).toBeInTheDocument();

      // Then check specific content
      expect(screen.getByText("System Monitoring")).toBeInTheDocument();
      expect(
        screen.getByText("Grafana dashboards and system metrics"),
      ).toBeInTheDocument();
    });

    it("displays Grafana dashboards from our existing infrastructure", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(
          screen.getByText("EconGraph Platform Overview"),
        ).toBeInTheDocument();
        expect(screen.getByText("Database Statistics")).toBeInTheDocument();
        expect(screen.getByText("Crawler Status")).toBeInTheDocument();
      });
    });

    it("shows correct Grafana URLs for our dashboards", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const grafanaButton = screen.getByText("Open Grafana");
        expect(grafanaButton.closest("a")).toHaveAttribute(
          "href",
          "http://localhost:30001",
        );
      });
    });

    it("renders system status overview cards", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("Overall Status")).toBeInTheDocument();
        expect(screen.getByText("Active Alerts")).toBeInTheDocument();
        expect(screen.getByText("Total Series")).toBeInTheDocument();
        expect(screen.getByText("Active Crawlers")).toBeInTheDocument();
      });
    });

    it("displays service status indicators", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("Service Status")).toBeInTheDocument();
        expect(screen.getByText("BACKEND")).toBeInTheDocument();
        expect(screen.getByText("DATABASE")).toBeInTheDocument();
        expect(screen.getByText("CRAWLER")).toBeInTheDocument();
        expect(screen.getByText("GRAFANA")).toBeInTheDocument();
      });
    });
  });

  describe("Dashboard Integration", () => {
    it("shows dashboard descriptions from our Grafana setup", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(
          screen.getByText(/High-level system monitoring and health overview/),
        ).toBeInTheDocument();
        expect(
          screen.getByText(
            /Comprehensive PostgreSQL monitoring for time series data/,
          ),
        ).toBeInTheDocument();
        expect(
          screen.getByText(
            /Data crawler monitoring and queue processing analysis/,
          ),
        ).toBeInTheDocument();
      });
    });

    it("displays correct dashboard URLs for our infrastructure", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      const overviewButtons = screen.getAllByText("Open Dashboard");
      expect(overviewButtons.length).toBeGreaterThan(0);
      expect(overviewButtons[0].closest("a")).toHaveAttribute(
        "href",
        expect.stringContaining("localhost:30001"),
      );
    });

    it("shows embedded view buttons for Grafana panels", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const embedButtons = screen.getAllByText("Embed View");
        expect(embedButtons).toHaveLength(3); // One for each dashboard
        embedButtons.forEach((button) => {
          expect(button.closest("a")).toHaveAttribute(
            "href",
            expect.stringContaining("d-solo"),
          );
        });
      });
    });
  });

  describe("Tab Navigation", () => {
    it("switches between dashboard overview and embedded views tabs", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(
          screen.getByText("Available Grafana Dashboards"),
        ).toBeInTheDocument();
      });

      // Switch to embedded views tab
      fireEvent.click(screen.getByText("Embedded Views"));

      await waitFor(() => {
        expect(
          screen.getByText("Embedded Dashboard Views"),
        ).toBeInTheDocument();
      });

      // Switch to quick metrics tab
      fireEvent.click(screen.getByText("Quick Metrics"));

      await waitFor(() => {
        expect(screen.getByText("Quick System Metrics")).toBeInTheDocument();
      });
    });

    it("shows embedded view placeholder with Grafana integration note", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      fireEvent.click(screen.getByText("Embedded Views"));

      await waitFor(() => {
        expect(
          screen.getByText(/These are embedded views from Grafana dashboards/),
        ).toBeInTheDocument();
        expect(
          screen.getByText(/Click the fullscreen icon to open in Grafana/),
        ).toBeInTheDocument();
      });
    });
  });

  describe("Status Indicators", () => {
    it("displays correct status colors and icons", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should show healthy status indicators
        const statusChips = screen.getAllByText("HEALTHY");
        expect(statusChips.length).toBeGreaterThan(0);
      });
    });

    it("shows active alerts count", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("2")).toBeInTheDocument(); // Active alerts count
        expect(screen.getByText("Active Alerts")).toBeInTheDocument();
      });
    });

    // TODO: Fix network calls to Grafana causing test timeouts
    // Issue: Tests are making actual network requests to localhost:30001
    it.skip("displays service status with appropriate indicators", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      // Should show different statuses for different services
      const healthyElements = screen.getAllByText("HEALTHY");
      expect(healthyElements.length).toBeGreaterThan(0);
      // The component shows service names, not status text
      expect(screen.getByText("CRAWLER")).toBeInTheDocument();
    });
  });

  describe("User Interactions", () => {
    it("handles refresh button click", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      const refreshButton = screen.getByLabelText(/refresh/i);
      fireEvent.click(refreshButton);

      // Should trigger loading state
      await waitFor(() => {
        expect(refreshButton).toBeDisabled();
      });
    });

    it("opens Grafana in new tab when button is clicked", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const grafanaButton = screen.getByText("Open Grafana");
        expect(grafanaButton.closest("a")).toHaveAttribute("target", "_blank");
        expect(grafanaButton.closest("a")).toHaveAttribute(
          "rel",
          "noopener noreferrer",
        );
      });
    });

    it("opens individual dashboards in new tabs", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const dashboardButtons = screen.getAllByText("Open Dashboard");
        dashboardButtons.forEach((button) => {
          expect(button.closest("a")).toHaveAttribute("target", "_blank");
          expect(button.closest("a")).toHaveAttribute(
            "rel",
            "noopener noreferrer",
          );
        });
      });
    });
  });

  describe("Metrics Display", () => {
    it("shows aggregated metrics from all dashboards", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should show totals from all dashboards
        expect(screen.getByText("Total Series")).toBeInTheDocument();
        expect(screen.getByText("Active Crawlers")).toBeInTheDocument();
      });
    });

    it("displays uptime information", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const uptimeChips = screen.getAllByText(/99\.9%|100%/);
        expect(uptimeChips.length).toBeGreaterThan(0);
      });
    });

    it("shows last update timestamps", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const updateTexts = screen.getAllByText(/Last Update:/);
        expect(updateTexts.length).toBeGreaterThan(0);
      });
    });
  });

  describe("Error Handling", () => {
    it("handles loading states gracefully", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      // Should show loading state initially
      expect(screen.getByText("System Monitoring")).toBeInTheDocument();
    });

    it("displays embedded view placeholders when Grafana is unavailable", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      fireEvent.click(screen.getByText("Embedded Views"));

      await waitFor(() => {
        expect(screen.getAllByText("Embedded Grafana Dashboard")).toHaveLength(
          3,
        );
      });
    });
  });

  describe("Integration with Existing Infrastructure", () => {
    // TODO: Fix network calls to Grafana causing test timeouts
    // Issue: Tests are making actual network requests to localhost:30001
    it.skip("uses correct Grafana port (30001) from our monitoring setup", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const links = screen.getAllByRole("link");
        const grafanaLinks = links.filter((link) =>
          link.getAttribute("href")?.includes("localhost:30001"),
        );
        expect(grafanaLinks.length).toBeGreaterThan(0);
      });
    });

    it("references our actual dashboard IDs", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const links = screen.getAllByRole("link");
        const dashboardLinks = links.filter((link) => {
          const href = link.getAttribute("href");
          return (
            href?.includes("econgraph-overview") ||
            href?.includes("database-statistics") ||
            href?.includes("crawler-status")
          );
        });
        expect(dashboardLinks.length).toBeGreaterThan(0);
      });
    });

    it("matches our Grafana dashboard refresh rates", async () => {
      render(
        <TestWrapper>
          <MonitoringPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Verify we're using the correct time ranges from our dashboard configs
        const links = screen.getAllByRole("link");
        const timeRangeLinks = links.filter((link) => {
          const href = link.getAttribute("href");
          return (
            href?.includes("from=now-1h") ||
            href?.includes("from=now-6h") ||
            href?.includes("from=now-2h")
          );
        });
        expect(timeRangeLinks.length).toBeGreaterThan(0);
      });
    });
  });
});
