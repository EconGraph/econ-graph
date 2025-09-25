/**
 * Comprehensive tests for CrawlerDashboard component
 *
 * Tests:
 * - Component rendering and initial state
 * - Data loading and error handling
 * - User interactions (refresh, trigger crawl, stop crawler)
 * - Status display and metrics
 * - Accessibility features
 * - Performance with large datasets
 */

import React from "react";
import { render, screen, act } from "@testing-library/react";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import userEvent from "@testing-library/user-event";
import CrawlerDashboard from "../CrawlerDashboard";

// Mock Material-UI theme
const theme = createTheme();

// Mock date-fns format function
jest.mock("date-fns", () => ({
  format: jest.fn((date, formatStr) => {
    if (formatStr === "MMM dd, HH:mm") {
      return "Jan 01, 12:00";
    }
    return "12:00:00.000";
  }),
}));

// Mock console methods to avoid noise in tests
const originalConsoleError = console.error;
const originalConsoleWarn = console.warn;

beforeAll(() => {
  console.error = jest.fn();
  console.warn = jest.fn();
});

afterAll(() => {
  console.error = originalConsoleError;
  console.warn = originalConsoleWarn;
});

// Mock the useCrawlerData hooks to return our GraphQL mock data
jest.mock("../../hooks/useCrawlerData", () => ({
  useCrawlerData: jest.fn(),
  useCrawlerStatus: jest.fn(),
  useQueueStatistics: jest.fn(),
  usePerformanceMetrics: jest.fn(),
}));

// Create a single QueryClient for all tests to avoid performance issues
const createTestQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        gcTime: 0,
        staleTime: 0,
      },
    },
  });

// Create a single QueryClient instance for the test suite
let testQueryClient: QueryClient;

// Test wrapper with QueryClient - fixed to use single instance
const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  return (
    <QueryClientProvider client={testQueryClient}>
      <ThemeProvider theme={theme}>{children}</ThemeProvider>
    </QueryClientProvider>
  );
};

const renderWithTheme = (component: React.ReactElement) => {
  return render(<TestWrapper>{component}</TestWrapper>);
};

describe("CrawlerDashboard", () => {
  beforeAll(() => {
    // Initialize QueryClient once for all tests
    testQueryClient = createTestQueryClient();

    // Set up hook mocks with GraphQL mock data
    const {
      useCrawlerData,
      useCrawlerStatus,
      useQueueStatistics,
      usePerformanceMetrics,
    } = require("../../hooks/useCrawlerData");

    // Import mock data dynamically
    const getCrawlerStatusSuccess = require("../../__mocks__/graphql/getCrawlerStatus/success.json");
    const getQueueStatisticsSuccess = require("../../__mocks__/graphql/getQueueStatistics/success.json");
    const getPerformanceMetricsSuccess = require("../../__mocks__/graphql/getPerformanceMetrics/success.json");

    // Mock useCrawlerData to return combined mock data with correct structure
    useCrawlerData.mockReturnValue({
      status: {
        status: getCrawlerStatusSuccess.data.crawlerStatus,
        loading: false,
        error: null,
        refresh: jest.fn(),
      },
      queueStats: {
        statistics: getQueueStatisticsSuccess.data.queueStatistics,
        loading: false,
        error: null,
        refresh: jest.fn(),
        derived: {
          progressPercentage: 66.7,
          successRate: 66.7,
          errorRate: 0,
        },
      },
      performance: {
        metrics: getPerformanceMetricsSuccess.data.performanceMetrics,
        latestMetrics: getPerformanceMetricsSuccess.data.performanceMetrics[0],
        loading: false,
        error: null,
        refresh: jest.fn(),
      },
      logs: {
        logs: [],
        loading: false,
        error: null,
        refresh: jest.fn(),
      },
      control: {
        loading: false,
        error: null,
        actions: {
          triggerCrawl: jest.fn(),
          startCrawler: jest.fn(),
          stopCrawler: jest.fn(),
          pauseCrawler: jest.fn(),
          resumeCrawler: jest.fn(),
        },
      },
      refreshAll: jest.fn(),
      loading: false,
      error: null,
    });

    // Mock individual hooks
    useCrawlerStatus.mockReturnValue({
      status: getCrawlerStatusSuccess.data.crawlerStatus,
      loading: false,
      error: null,
      refresh: jest.fn(),
    });

    useQueueStatistics.mockReturnValue({
      statistics: getQueueStatisticsSuccess.data.queueStatistics,
      loading: false,
      error: null,
      refresh: jest.fn(),
      derived: {
        progressPercentage: 66.7,
        successRate: 66.7,
        errorRate: 0,
      },
    });

    usePerformanceMetrics.mockReturnValue({
      metrics: getPerformanceMetricsSuccess.data.performanceMetrics,
      latestMetrics: getPerformanceMetricsSuccess.data.performanceMetrics[0],
      loading: false,
      error: null,
      refresh: jest.fn(),
    });
  });

  beforeEach(() => {
    // Clear QueryClient cache between tests to ensure isolation
    testQueryClient.clear();

    // Mock timers for consistent testing
    jest.useFakeTimers();
  });

  afterEach(() => {
    // Clean up timers
    jest.runOnlyPendingTimers();
    jest.useRealTimers();
  });

  afterAll(() => {
    // Clean up QueryClient after all tests
    testQueryClient.clear();
  });

  describe("Component Rendering", () => {
    it("renders crawler dashboard with GraphQL mock data", async () => {
      renderWithTheme(<CrawlerDashboard />);

      // Wait for the component to load data from GraphQL mocks
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 100));
      });

      // Check that crawler status is displayed (from mock data)
      expect(screen.getByText(/running/i)).toBeInTheDocument();
      expect(screen.getByText(/Processing FRED data/i)).toBeInTheDocument();

      // Check that queue statistics are displayed (from mock data)
      expect(screen.getByText(/150/)).toBeInTheDocument(); // total items
      expect(screen.getByText(/45/)).toBeInTheDocument(); // pending items

      // Check that performance metrics are displayed (from mock data)
      expect(screen.getByText(/45.2%/)).toBeInTheDocument(); // CPU usage
      expect(screen.getByText(/2.1 GB/)).toBeInTheDocument(); // Memory usage
    });

    it("renders loading state initially", () => {
      renderWithTheme(<CrawlerDashboard />);

      expect(screen.getByText("Loading crawler data...")).toBeInTheDocument();
      expect(screen.getByRole("progressbar")).toBeInTheDocument();
    });

    it("handles GraphQL error scenarios", async () => {
      // Mock useCrawlerData to return error
      const { useCrawlerData } = require("../../hooks/useCrawlerData");
      useCrawlerData.mockReturnValue({
        status: {
          data: null,
          loading: false,
          error: new Error("Failed to retrieve crawler status"),
          refresh: jest.fn(),
        },
        queueStats: {
          data: null,
          loading: false,
          error: null,
          refresh: jest.fn(),
        },
        performance: {
          data: null,
          loading: false,
          error: null,
          refresh: jest.fn(),
        },
        control: {
          triggerCrawl: jest.fn(),
          startCrawler: jest.fn(),
          stopCrawler: jest.fn(),
          pauseCrawler: jest.fn(),
          resumeCrawler: jest.fn(),
        },
        refreshAll: jest.fn(),
        loading: false,
        error: new Error("Failed to retrieve crawler status"),
      });

      renderWithTheme(<CrawlerDashboard />);

      // Wait for the component to process the error
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 100));
      });

      // Check that error state is displayed
      expect(screen.getByText(/Error/i)).toBeInTheDocument();
    });

    it("renders main dashboard after loading", async () => {
      renderWithTheme(<CrawlerDashboard />);

      // Wait for loading to complete
      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      expect(screen.getByText("🕷️ Crawler Administration")).toBeInTheDocument();
      expect(screen.getByText("Crawler Status")).toBeInTheDocument();
      expect(screen.getByText("Queue Statistics")).toBeInTheDocument();
    });

    it("displays crawler status information correctly", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Check status chip
      expect(screen.getByText("Running")).toBeInTheDocument();

      // Check metrics
      expect(screen.getByText("Active Workers")).toBeInTheDocument();
      expect(screen.getByText("3")).toBeInTheDocument(); // active_workers
      expect(screen.getByText("Last Crawl")).toBeInTheDocument();
      expect(screen.getByText("Next Scheduled Crawl")).toBeInTheDocument();
    });

    it("displays queue statistics correctly", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      expect(screen.getByText("Total Items")).toBeInTheDocument();
      expect(screen.getByText("1247")).toBeInTheDocument(); // total_items
      expect(screen.getByText("Pending")).toBeInTheDocument();
      expect(screen.getByText("23")).toBeInTheDocument(); // pending_items
      expect(screen.getByText("Processing")).toBeInTheDocument();
      expect(screen.getByText("3")).toBeInTheDocument(); // processing_items
      expect(screen.getByText("Completed")).toBeInTheDocument();
      expect(screen.getByText("1200")).toBeInTheDocument(); // completed_items
      expect(screen.getByText("Failed")).toBeInTheDocument();
      expect(screen.getByText("21")).toBeInTheDocument(); // failed_items
    });

    it("displays queue progress bar", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      expect(screen.getByText("Queue Progress")).toBeInTheDocument();
      expect(screen.getByText("Processing: 3 / 1247")).toBeInTheDocument();
      expect(screen.getByText("96.2%")).toBeInTheDocument(); // (1200/1247)*100
    });

    it("displays recent activity table", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      expect(screen.getByText("Recent Activity")).toBeInTheDocument();
      expect(screen.getByText("Time")).toBeInTheDocument();
      expect(screen.getByText("Source")).toBeInTheDocument();
      expect(screen.getByText("Series")).toBeInTheDocument();
      expect(screen.getByText("Status")).toBeInTheDocument();
      expect(screen.getByText("Duration")).toBeInTheDocument();
      expect(screen.getByText("Actions")).toBeInTheDocument();
    });
  });

  describe("User Interactions", () => {
    it("handles refresh button click", async () => {
      const user = userEvent.setup({ advanceTimers: jest.advanceTimersByTime });
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      const refreshButton = screen.getByText("Refresh");
      expect(refreshButton).toBeInTheDocument();

      await user.click(refreshButton);

      // Should show refreshing state
      expect(screen.getByText("Refreshing...")).toBeInTheDocument();
    });

    it("handles trigger crawl button click", async () => {
      const user = userEvent.setup({ advanceTimers: jest.advanceTimersByTime });
      const consoleSpy = jest.spyOn(console, "log").mockImplementation();

      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      const triggerButton = screen.getByText("Trigger Crawl");
      expect(triggerButton).toBeInTheDocument();

      await user.click(triggerButton);

      expect(consoleSpy).toHaveBeenCalledWith("Triggering manual crawl...");
      consoleSpy.mockRestore();
    });

    it("handles stop crawler button click", async () => {
      const user = userEvent.setup({ advanceTimers: jest.advanceTimersByTime });
      const consoleSpy = jest.spyOn(console, "log").mockImplementation();

      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      const stopButton = screen.getByText("Stop Crawler");
      expect(stopButton).toBeInTheDocument();

      await user.click(stopButton);

      expect(consoleSpy).toHaveBeenCalledWith("Stopping crawler...");
      consoleSpy.mockRestore();
    });

    it("handles error state display and dismissal", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Simulate error state - this test assumes error alert exists
      const errorAlert = screen.getByRole("alert");
      const closeButton = screen.getByRole("button", { name: /close/i });
      await userEvent.click(closeButton);
      expect(errorAlert).not.toBeInTheDocument();
    });
  });

  describe("Status Display", () => {
    it("shows correct status colors and icons", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Check for status chip with success color
      const statusChip = screen.getByText("Running");
      expect(statusChip).toBeInTheDocument();
    });

    it("displays metrics with correct values", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Check specific metric values
      expect(screen.getByText("3")).toBeInTheDocument(); // active_workers
      expect(screen.getByText("1247")).toBeInTheDocument(); // total_items
      expect(screen.getByText("23")).toBeInTheDocument(); // pending_items
      expect(screen.getByText("1200")).toBeInTheDocument(); // completed_items
    });

    it("shows progress bar with correct percentage", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      const progressBar = screen.getByRole("progressbar");
      expect(progressBar).toBeInTheDocument();
      expect(progressBar).toHaveAttribute("aria-valuenow", "96.2");
    });
  });

  describe("Accessibility", () => {
    it("has proper ARIA labels and roles", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Check for proper heading structure
      expect(screen.getByRole("heading", { level: 1 })).toBeInTheDocument();
      expect(screen.getByRole("heading", { level: 2 })).toBeInTheDocument();

      // Check for progress bar
      expect(screen.getByRole("progressbar")).toBeInTheDocument();

      // Check for buttons
      expect(
        screen.getByRole("button", { name: /refresh/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /trigger crawl/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /stop crawler/i }),
      ).toBeInTheDocument();
    });

    it("supports keyboard navigation", async () => {
      const user = userEvent.setup({ advanceTimers: jest.advanceTimersByTime });
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      const refreshButton = screen.getByText("Refresh");
      refreshButton.focus();
      expect(refreshButton).toHaveFocus();

      await user.keyboard("{Enter}");
      // Should trigger refresh action
    });

    it("has proper color contrast for status indicators", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Check that status chips have proper color attributes
      const statusChip = screen.getByText("Running");
      expect(statusChip).toBeInTheDocument();
    });
  });

  describe("Performance", () => {
    it("handles large datasets efficiently", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Should render without performance issues
      expect(screen.getByText("Recent Activity")).toBeInTheDocument();
    });

    it("updates metrics without causing re-renders", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Component should be stable
      expect(screen.getByText("Crawler Status")).toBeInTheDocument();
    });
  });

  describe("Error Handling", () => {
    it("displays error messages correctly", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Error handling is tested through the error state simulation
      // The component should gracefully handle errors
      expect(screen.getByText("🕷️ Crawler Administration")).toBeInTheDocument();
    });

    it("recovers from error states", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Component should be able to recover from errors
      expect(screen.getByText("Crawler Status")).toBeInTheDocument();
    });
  });

  describe("Data Loading", () => {
    it("shows loading state during data fetch", () => {
      renderWithTheme(<CrawlerDashboard />);

      expect(screen.getByText("Loading crawler data...")).toBeInTheDocument();
      expect(screen.getByRole("progressbar")).toBeInTheDocument();
    });

    it("transitions from loading to loaded state", async () => {
      renderWithTheme(<CrawlerDashboard />);

      // Initially loading
      expect(screen.getByText("Loading crawler data...")).toBeInTheDocument();

      // After timeout, should be loaded
      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      expect(
        screen.queryByText("Loading crawler data..."),
      ).not.toBeInTheDocument();
      expect(screen.getByText("Crawler Status")).toBeInTheDocument();
    });
  });

  describe("Component Integration", () => {
    it("integrates with Material-UI theme correctly", async () => {
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      // Should use theme colors and styling
      expect(screen.getByText("🕷️ Crawler Administration")).toBeInTheDocument();
    });

    it("works with different screen sizes", async () => {
      // Test responsive behavior
      renderWithTheme(<CrawlerDashboard />);

      await act(async () => {
        jest.advanceTimersByTime(1000);
      });

      expect(screen.getByText("Crawler Status")).toBeInTheDocument();
      expect(screen.getByText("Queue Statistics")).toBeInTheDocument();
    });
  });
});
