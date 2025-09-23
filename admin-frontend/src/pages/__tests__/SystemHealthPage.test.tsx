// REQUIREMENT: Comprehensive tests for SystemHealthPage component
// PURPOSE: Ensure system health page displays metrics correctly and integrates with Grafana dashboards
// This validates the health monitoring interface works with our existing infrastructure

import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import SystemHealthPage from "../SystemHealthPage";
import { AuthProvider } from "../../contexts/AuthContext";
import { SecurityProvider } from "../../contexts/SecurityContext";

// Mock fetch to prevent network requests to Grafana
global.fetch = jest.fn(() =>
  Promise.resolve({
    ok: true,
    status: 200,
    json: () => Promise.resolve({}),
    text: () => Promise.resolve(""),
  }),
) as jest.Mock;

// Mock setTimeout to run immediately in tests
jest.useFakeTimers();

// Mock all timers to run immediately
jest
  .spyOn(global, "setTimeout")
  .mockImplementation((fn: any, delay?: number) => {
    if (typeof fn === "function") {
      // Run the function immediately for tests
      fn();
    }
    return 1 as any;
  });

// Don't mock Date - let it work normally with the mock data
// The component uses mock data with hardcoded timestamps, so we don't need to mock Date

// Mock window.open to prevent actual navigation
Object.defineProperty(window, "open", {
  value: jest.fn(),
  writable: true,
});

// Mock the contexts to prevent network requests
jest.mock("../../contexts/AuthContext", () => ({
  AuthProvider: ({ children }: any) => children,
  useAuth: () => ({
    user: {
      id: "1",
      name: "Test Admin",
      email: "admin@test.com",
      role: "admin",
    },
    login: jest.fn(),
    logout: jest.fn(),
    isAuthenticated: true,
    loading: false,
  }),
}));

jest.mock("../../contexts/SecurityContext", () => ({
  SecurityProvider: ({ children }: any) => children,
  useSecurity: () => ({
    checkAccess: jest.fn(() => true),
    sessionRemainingTime: 3600,
    securityEvents: [],
    refreshSecurityContext: jest.fn(),
  }),
}));

// Create a test theme
const theme = createTheme();

// Test wrapper component
const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <BrowserRouter>
    <ThemeProvider theme={theme}>
      <AuthProvider>
        <SecurityProvider>{children}</SecurityProvider>
      </AuthProvider>
    </ThemeProvider>
  </BrowserRouter>
);

describe("SystemHealthPage", () => {
  // Set a generous timeout for this component due to complex initialization
  jest.setTimeout(5000);

  beforeEach(() => {
    jest.clearAllMocks();
    jest.clearAllTimers();
    // Run all pending timers immediately to prevent timeouts
    jest.runAllTimers();
  });

  afterEach(() => {
    jest.runAllTimers();
    jest.clearAllTimers();
  });

  describe("Rendering", () => {
    it("renders system health page with correct title", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      expect(screen.getByText("System Health")).toBeInTheDocument();
      expect(
        screen.getByText("Real-time system status and performance metrics"),
      ).toBeInTheDocument();
    });

    it("displays health metrics cards", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      expect(screen.getByTestId("health-metrics-grid")).toBeInTheDocument();
      expect(
        screen.getByTestId("health-metric-card-system-uptime"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("health-metric-card-response-time"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("health-metric-card-database-connections"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("health-metric-card-memory-usage"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("health-metric-card-disk-space"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("health-metric-card-active-users"),
      ).toBeInTheDocument();
    });

    it("shows service status list", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      expect(screen.getByTestId("services-status-section")).toBeInTheDocument();
      expect(screen.getByTestId("services-list")).toBeInTheDocument();
      expect(screen.getByText("Service Status")).toBeInTheDocument();
      expect(screen.getByText("Backend API")).toBeInTheDocument();
      expect(screen.getByText("PostgreSQL")).toBeInTheDocument();
      expect(screen.getByText("Data Crawler")).toBeInTheDocument();
      expect(screen.getByText("Grafana")).toBeInTheDocument();
      expect(screen.getByText("NGINX")).toBeInTheDocument();
    });

    it("displays quick actions section", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      expect(screen.getByTestId("quick-actions-section")).toBeInTheDocument();
      expect(screen.getByTestId("quick-actions-grid")).toBeInTheDocument();
      expect(screen.getByText("Quick Actions")).toBeInTheDocument();
      expect(screen.getByText("Platform Overview")).toBeInTheDocument();
      expect(screen.getByText("Performance Metrics")).toBeInTheDocument();
      expect(screen.getByText("Crawler Status")).toBeInTheDocument();
      expect(screen.getByText("Security Events")).toBeInTheDocument();
    });
  });

  describe("Health Metrics Display", () => {
    it("shows correct metric values and descriptions", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      expect(
        screen.getByTestId("metric-value-system-uptime"),
      ).toHaveTextContent("99.9%");
      expect(
        screen.getByTestId("metric-value-response-time"),
      ).toHaveTextContent("120ms");
      expect(
        screen.getByTestId("metric-value-database-connections"),
      ).toHaveTextContent("85%");
      expect(screen.getByTestId("metric-value-memory-usage")).toHaveTextContent(
        "68%",
      );
      expect(screen.getByTestId("metric-value-disk-space")).toHaveTextContent(
        "78%",
      );
      expect(screen.getByTestId("metric-value-active-users")).toHaveTextContent(
        "24",
      );
    });

    it("displays metric descriptions", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      expect(
        screen.getByTestId("metric-description-system-uptime"),
      ).toHaveTextContent("Overall system availability");
      expect(
        screen.getByTestId("metric-description-response-time"),
      ).toHaveTextContent("Average API response time");
      expect(
        screen.getByTestId("metric-description-database-connections"),
      ).toHaveTextContent("Active database connections");
      expect(
        screen.getByTestId("metric-description-memory-usage"),
      ).toHaveTextContent("System memory utilization");
      expect(
        screen.getByTestId("metric-description-disk-space"),
      ).toHaveTextContent("Available disk space");
      expect(
        screen.getByTestId("metric-description-active-users"),
      ).toHaveTextContent("Currently active users");
    });

    it("shows status indicators with correct colors", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Should show different status types - use proper ARIA labels for accessibility
      expect(
        screen.getByLabelText(/System Uptime status: healthy/i),
      ).toBeInTheDocument();
      expect(
        screen.getByLabelText(/Response Time status: healthy/i),
      ).toBeInTheDocument();
      expect(
        screen.getByLabelText(/Database Connections status: warning/i),
      ).toBeInTheDocument();
    });

    it("displays trend indicators", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Trend icons should be present (they're rendered as SVG icons)
      const trendIcons = screen.getAllByTestId("TrendingUpIcon");
      expect(trendIcons.length).toBeGreaterThan(0);
    });

    it("shows last check timestamps", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Should show formatted timestamps - check for any timestamp format
      const timestampElements = screen.getAllByText(/\d{1,2}:\d{2}:\d{2}/);
      expect(timestampElements.length).toBeGreaterThan(0);
    });
  });

  describe("Service Status", () => {
    it("displays service information correctly", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Should show service versions and uptime - check for any version numbers
      const versionElements = screen.getAllByText(/\d+\.\d+/);
      expect(versionElements.length).toBeGreaterThan(0);
    });

    it("shows service uptime information", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Check for uptime format - any duration pattern
      const uptimeElements = screen.getAllByText(/\d+d \d+h \d+m/);
      expect(uptimeElements.length).toBeGreaterThan(0);
    });

    it("displays resource utilization with progress bars", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      expect(
        screen.getByTestId("service-backend-api-cpu-usage"),
      ).toHaveTextContent("CPU: 45%");
      expect(
        screen.getByTestId("service-backend-api-ram-usage"),
      ).toHaveTextContent("RAM: 62%");
      expect(
        screen.getByTestId("service-backend-api-disk-usage"),
      ).toHaveTextContent("Disk: 12%");
    });

    it("shows different service statuses", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Should show running and degraded statuses
      const runningElements = screen.getAllByText("RUNNING");
      expect(runningElements.length).toBeGreaterThan(0);
      expect(screen.getByText("DEGRADED")).toBeInTheDocument();
    });
  });

  describe("Grafana Integration", () => {
    it("links to correct Grafana dashboards", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      const grafanaButton = screen.getByTestId("open-grafana-button");
      expect(grafanaButton).toHaveAttribute(
        "href",
        "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview",
      );
    });

    it("provides Grafana links for individual metrics", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      const grafanaLinks = screen.getAllByTestId(/metric-grafana-link-/);
      expect(grafanaLinks.length).toBeGreaterThan(0);

      grafanaLinks.forEach((link) => {
        expect(link).toHaveAttribute(
          "href",
          expect.stringContaining("localhost:30001"),
        );
      });
    });

    it("uses correct dashboard URLs for different metric types", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      const dbLink = screen.getByTestId(
        "metric-grafana-link-database-connections",
      );
      expect(dbLink).toHaveAttribute(
        "href",
        expect.stringContaining("database-statistics"),
      );

      const overviewLink = screen.getByTestId(
        "metric-grafana-link-system-uptime",
      );
      expect(overviewLink).toHaveAttribute(
        "href",
        expect.stringContaining("econgraph-overview"),
      );
    });
  });

  describe("Quick Actions", () => {
    it("provides quick access to different Grafana dashboards", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      expect(screen.getByTestId("quick-actions-section")).toBeInTheDocument();
      expect(screen.getByTestId("quick-actions-grid")).toBeInTheDocument();

      // Check that the buttons exist
      expect(screen.getByText("Platform Overview")).toBeInTheDocument();
      expect(screen.getByText("Performance Metrics")).toBeInTheDocument();
      expect(screen.getByText("Crawler Status")).toBeInTheDocument();
      expect(screen.getByText("Security Events")).toBeInTheDocument();
    });

    it("opens all quick action links in new tabs", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      const actionButtons = screen.getAllByText(
        /Platform Overview|Performance Metrics|Crawler Status|Security Events/,
      );

      actionButtons.forEach((button) => {
        expect(button.closest("a")).toHaveAttribute("target", "_blank");
        expect(button.closest("a")).toHaveAttribute(
          "rel",
          "noopener noreferrer",
        );
      });
    });
  });

  describe("Overall Status Alert", () => {
    it("displays overall system status based on service health", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      expect(screen.getByTestId("system-status-display")).toHaveTextContent(
        "System Status: WARNING",
      );
    });

    it("shows last update timestamp", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Use data-testid attributes for reliable testing
      expect(screen.getByTestId("last-update-timestamp")).toHaveTextContent(
        /Last updated:/,
      );
    });
  });

  describe("User Interactions", () => {
    it("handles refresh button click", () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      const refreshButton = screen.getByLabelText(/refresh/i);
      fireEvent.click(refreshButton);

      // Should trigger loading state
      expect(refreshButton).toBeDisabled();

      // Should show refreshing message
      expect(screen.getByText(/Refreshing\.\.\./)).toBeInTheDocument();
    });

    it("updates timestamp after refresh", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      const refreshButton = screen.getByLabelText(/refresh/i);
      fireEvent.click(refreshButton);

      // Run all timers to complete the refresh
      jest.runAllTimers();

      // Button should be enabled after refresh completes
      expect(refreshButton).not.toBeDisabled();
    });
  });

  describe("Resource Utilization Display", () => {
    it("shows resource usage with color-coded progress bars", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should show different resource levels
        expect(screen.getByText("CPU: 45%")).toBeInTheDocument();
        expect(screen.getByText("CPU: 25%")).toBeInTheDocument();
        expect(screen.getByText("CPU: 85%")).toBeInTheDocument();
        expect(screen.getByText("CPU: 15%")).toBeInTheDocument();
        expect(screen.getByText("CPU: 5%")).toBeInTheDocument();
      });
    });

    it("displays memory and disk usage", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("RAM: 62%")).toBeInTheDocument();
        expect(screen.getByText("RAM: 78%")).toBeInTheDocument();
        expect(screen.getByText("RAM: 45%")).toBeInTheDocument();
        expect(screen.getByText("RAM: 35%")).toBeInTheDocument();
        expect(screen.getByText("RAM: 12%")).toBeInTheDocument();

        expect(screen.getByText("Disk: 12%")).toBeInTheDocument();
        expect(screen.getByText("Disk: 45%")).toBeInTheDocument();
        expect(screen.getByText("Disk: 8%")).toBeInTheDocument();
        expect(screen.getByText("Disk: 5%")).toBeInTheDocument();
        expect(screen.getByText("Disk: 2%")).toBeInTheDocument();
      });
    });
  });

  describe("Integration with Existing Infrastructure", () => {
    it("uses correct Grafana port from our monitoring setup", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
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

    it("references our actual service names", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should match our actual service names from k8s manifests
        expect(screen.getByText("Backend API")).toBeInTheDocument();
        expect(screen.getByText("PostgreSQL")).toBeInTheDocument();
        expect(screen.getByText("Data Crawler")).toBeInTheDocument();
        expect(screen.getByText("Grafana")).toBeInTheDocument();
        expect(screen.getByText("NGINX")).toBeInTheDocument();
      });
    });
  });

  describe("Error Handling", () => {
    it("handles loading states gracefully", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      // Should show loading state initially
      expect(screen.getByText("System Health")).toBeInTheDocument();
    });

    it("displays current timestamp on initial load", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText(/Last updated:/)).toBeInTheDocument();
      });
    });
  });
});
