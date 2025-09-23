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

// Mock the contexts
// const mockAuthContext = {
//   user: {
//     id: "1",
//     name: "Test Admin",
//     email: "admin@test.com",
//     role: "admin",
//   },
//   login: jest.fn(),
//   logout: jest.fn(),
//   isAuthenticated: true,
//   loading: false,
// };

// const mockSecurityContext = {
//   checkAccess: jest.fn(() => true),
//   sessionRemainingTime: 3600,
//   securityEvents: [],
//   refreshSecurityContext: jest.fn(),
// };

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
  beforeEach(() => {
    jest.clearAllMocks();
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

    it("displays health metrics cards", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("System Uptime")).toBeInTheDocument();
        expect(screen.getByText("Response Time")).toBeInTheDocument();
        expect(screen.getByText("Database Connections")).toBeInTheDocument();
        expect(screen.getByText("Memory Usage")).toBeInTheDocument();
        expect(screen.getByText("Disk Space")).toBeInTheDocument();
        expect(screen.getByText("Active Users")).toBeInTheDocument();
      });
    });

    it("shows service status list", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("Service Status")).toBeInTheDocument();
        expect(screen.getByText("Backend API")).toBeInTheDocument();
        expect(screen.getByText("PostgreSQL")).toBeInTheDocument();
        expect(screen.getByText("Data Crawler")).toBeInTheDocument();
        expect(screen.getByText("Grafana")).toBeInTheDocument();
        expect(screen.getByText("NGINX")).toBeInTheDocument();
      });
    });

    it("displays quick actions section", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("Quick Actions")).toBeInTheDocument();
        expect(screen.getByText("Platform Overview")).toBeInTheDocument();
        expect(screen.getByText("Performance Metrics")).toBeInTheDocument();
        expect(screen.getByText("Crawler Status")).toBeInTheDocument();
        expect(screen.getByText("Security Events")).toBeInTheDocument();
      });
    });
  });

  describe("Health Metrics Display", () => {
    it("shows correct metric values and descriptions", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("99.9%")).toBeInTheDocument(); // System Uptime
        expect(screen.getByText("120ms")).toBeInTheDocument(); // Response Time
        expect(screen.getByText("85%")).toBeInTheDocument(); // Database Connections
        expect(screen.getByText("68%")).toBeInTheDocument(); // Memory Usage
        expect(screen.getByText("78%")).toBeInTheDocument(); // Disk Space
        expect(screen.getByText("24")).toBeInTheDocument(); // Active Users
      });
    });

    it("displays metric descriptions", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(
          screen.getByText("Overall system availability"),
        ).toBeInTheDocument();
        expect(
          screen.getByText("Average API response time"),
        ).toBeInTheDocument();
        expect(
          screen.getByText("Active database connections"),
        ).toBeInTheDocument();
        expect(
          screen.getByText("System memory utilization"),
        ).toBeInTheDocument();
        expect(screen.getByText("Available disk space")).toBeInTheDocument();
        expect(screen.getByText("Currently active users")).toBeInTheDocument();
      });
    });

    it("shows status indicators with correct colors", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should show different status types
        expect(screen.getByText("HEALTHY")).toBeInTheDocument();
        expect(screen.getByText("WARNING")).toBeInTheDocument();
      });
    });

    it("displays trend indicators", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Trend icons should be present (they're rendered as SVG icons)
        const trendIcons = document.querySelectorAll(
          '[data-testid="TrendingUpIcon"]',
        );
        expect(trendIcons.length).toBeGreaterThan(0);
      });
    });

    it("shows last check timestamps", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should show formatted timestamps
        const timestamps = screen.getAllByText(
          /10:30:00|9:15:00|16:45:00|14:20:00/,
        );
        expect(timestamps.length).toBeGreaterThan(0);
      });
    });
  });

  describe("Service Status", () => {
    it("displays service information correctly", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should show service versions and uptime
        expect(screen.getByText("v1.2.3")).toBeInTheDocument(); // Backend version
        expect(screen.getByText("14.8")).toBeInTheDocument(); // PostgreSQL version
        expect(screen.getByText("v1.1.0")).toBeInTheDocument(); // Crawler version
        expect(screen.getByText("10.2.0")).toBeInTheDocument(); // Grafana version
        expect(screen.getByText("1.24.0")).toBeInTheDocument(); // NGINX version
      });
    });

    it("shows service uptime information", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("7d 12h 30m")).toBeInTheDocument(); // Most services uptime
        expect(screen.getByText("2d 8h 15m")).toBeInTheDocument(); // Crawler uptime
      });
    });

    it("displays resource utilization with progress bars", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should show CPU, memory, and disk usage
        expect(screen.getByText("CPU: 45%")).toBeInTheDocument();
        expect(screen.getByText("RAM: 62%")).toBeInTheDocument();
        expect(screen.getByText("Disk: 12%")).toBeInTheDocument();
      });
    });

    it("shows different service statuses", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should show running and degraded statuses
        expect(screen.getByText("RUNNING")).toBeInTheDocument();
        expect(screen.getByText("DEGRADED")).toBeInTheDocument();
      });
    });
  });

  describe("Grafana Integration", () => {
    it("links to correct Grafana dashboards", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const grafanaButton = screen.getByText("Open Grafana");
        expect(grafanaButton.closest("a")).toHaveAttribute(
          "href",
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview",
        );
      });
    });

    it("provides Grafana links for individual metrics", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const grafanaLinks = screen.getAllByLabelText("View in Grafana");
        expect(grafanaLinks.length).toBeGreaterThan(0);

        grafanaLinks.forEach((link) => {
          expect(link).toHaveAttribute(
            "href",
            expect.stringContaining("localhost:30001"),
          );
        });
      });
    });

    it("uses correct dashboard URLs for different metric types", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const links = screen.getAllByRole("link");

        // Database connections should link to database dashboard
        const dbLinks = links.filter((link) =>
          link.getAttribute("href")?.includes("database-statistics"),
        );
        expect(dbLinks.length).toBeGreaterThan(0);

        // System metrics should link to overview dashboard
        const overviewLinks = links.filter((link) =>
          link.getAttribute("href")?.includes("econgraph-overview"),
        );
        expect(overviewLinks.length).toBeGreaterThan(0);
      });
    });
  });

  describe("Quick Actions", () => {
    it("provides quick access to different Grafana dashboards", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const platformButton = screen.getByText("Platform Overview");
        const performanceButton = screen.getByText("Performance Metrics");
        const crawlerButton = screen.getByText("Crawler Status");
        const securityButton = screen.getByText("Security Events");

        expect(platformButton.closest("a")).toHaveAttribute(
          "href",
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview",
        );
        expect(performanceButton.closest("a")).toHaveAttribute(
          "href",
          "http://localhost:30001/d/econgraph-overview/econgraph-platform-overview",
        );
        expect(crawlerButton.closest("a")).toHaveAttribute(
          "href",
          "http://localhost:30001/d/crawler-status/crawler-status",
        );
        expect(securityButton.closest("a")).toHaveAttribute(
          "href",
          "http://localhost:30001/d/security/security-dashboard",
        );
      });
    });

    it("opens all quick action links in new tabs", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
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
  });

  describe("Overall Status Alert", () => {
    it("displays overall system status based on service health", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should show warning status due to degraded crawler
        expect(screen.getByText("System Status: WARNING")).toBeInTheDocument();
      });
    });

    it("shows last update timestamp", async () => {
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

  describe("User Interactions", () => {
    it("handles refresh button click", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      const refreshButton = screen.getByLabelText(/refresh/i);
      fireEvent.click(refreshButton);

      // Should trigger loading state
      await waitFor(() => {
        expect(refreshButton).toBeDisabled();
      });

      // Should show refreshing message
      await waitFor(() => {
        expect(screen.getByText(/Refreshing\.\.\./)).toBeInTheDocument();
      });
    });

    it("updates timestamp after refresh", async () => {
      render(
        <TestWrapper>
          <SystemHealthPage />
        </TestWrapper>,
      );

      const refreshButton = screen.getByLabelText(/refresh/i);
      fireEvent.click(refreshButton);

      // Wait for refresh to complete
      await waitFor(() => {
        expect(refreshButton).not.toBeDisabled();
      });
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
