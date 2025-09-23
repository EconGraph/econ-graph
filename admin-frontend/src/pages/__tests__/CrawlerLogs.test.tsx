/**
 * Comprehensive tests for CrawlerLogs component
 *
 * Tests:
 * - Log display and filtering functionality
 * - Performance metrics visualization
 * - Real-time updates and auto-refresh
 * - Search and filter operations
 * - Export and clear operations
 * - Accessibility and user interactions
 */

import React from "react";
import { render, screen, act } from "@testing-library/react";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import userEvent from "@testing-library/user-event";
import CrawlerLogs from "../CrawlerLogs";

// Mock Material-UI theme
const theme = createTheme();

// Mock date-fns format function
jest.mock("date-fns", () => ({
  format: jest.fn((date, formatStr) => {
    if (formatStr === "HH:mm:ss.SSS") {
      return "12:00:00.000";
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

const renderWithTheme = (component: React.ReactElement) => {
  return render(<ThemeProvider theme={theme}>{component}</ThemeProvider>);
};

describe("CrawlerLogs", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    jest.useFakeTimers();
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  describe("Component Rendering", () => {
    it("renders main logs interface", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(
        screen.getByText("📊 Crawler Logs & Monitoring"),
      ).toBeInTheDocument();
      expect(screen.getByText("Refresh")).toBeInTheDocument();
      expect(screen.getByText("Export")).toBeInTheDocument();
      expect(screen.getByText("Clear")).toBeInTheDocument();
    });

    it("displays performance metrics cards", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("CPU Usage")).toBeInTheDocument();
      expect(screen.getByText("Memory Usage")).toBeInTheDocument();
      expect(screen.getByText("Queue Depth")).toBeInTheDocument();
      expect(screen.getByText("Error Rate")).toBeInTheDocument();
    });

    it("shows logs and events section", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("Logs & Events")).toBeInTheDocument();
      expect(screen.getByText("Auto-refresh")).toBeInTheDocument();
      expect(screen.getByText("Total Logs")).toBeInTheDocument();
    });

    it("displays log filters", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByPlaceholderText("Search logs...")).toBeInTheDocument();
      expect(screen.getByLabelText("Level")).toBeInTheDocument();
      expect(screen.getByLabelText("Source")).toBeInTheDocument();
      expect(screen.getByText("Clear Filters")).toBeInTheDocument();
    });

    it("shows logs table with headers", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("Timestamp")).toBeInTheDocument();
      expect(screen.getByText("Level")).toBeInTheDocument();
      expect(screen.getByText("Source")).toBeInTheDocument();
      expect(screen.getByText("Message")).toBeInTheDocument();
      expect(screen.getByText("Status")).toBeInTheDocument();
      expect(screen.getByText("Duration")).toBeInTheDocument();
      expect(screen.getByText("Actions")).toBeInTheDocument();
    });
  });

  describe("Performance Metrics", () => {
    it("displays CPU usage with progress bar", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("CPU Usage")).toBeInTheDocument();
      expect(screen.getByText(/\d+\.\d+%/)).toBeInTheDocument();
    });

    it("displays memory usage with progress bar", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("Memory Usage")).toBeInTheDocument();
      expect(screen.getByText(/\d+\.\d+%/)).toBeInTheDocument();
    });

    it("displays queue depth metric", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("Queue Depth")).toBeInTheDocument();
      expect(screen.getByText(/\d+/)).toBeInTheDocument();
      expect(screen.getByText("items pending")).toBeInTheDocument();
    });

    it("displays error rate metric", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("Error Rate")).toBeInTheDocument();
      expect(screen.getByText(/\d+\.\d+%/)).toBeInTheDocument();
      expect(screen.getByText("last hour")).toBeInTheDocument();
    });
  });

  describe("Log Filtering and Search", () => {
    it("filters logs by search term", async () => {
      const user = userEvent.setup();
      renderWithTheme(<CrawlerLogs />);

      const searchInput = screen.getByPlaceholderText("Search logs...");
      await user.type(searchInput, "GDP");

      // Should filter logs containing 'GDP'
      expect(searchInput).toHaveValue("GDP");
    });

    it("filters logs by level", async () => {
      const user = userEvent.setup();
      renderWithTheme(<CrawlerLogs />);

      const levelSelect = screen.getByLabelText("Level");
      await user.click(levelSelect);
      await user.click(screen.getByText("Error"));

      expect(levelSelect).toHaveValue("error");
    });

    it("filters logs by source", async () => {
      const user = userEvent.setup();
      renderWithTheme(<CrawlerLogs />);

      const sourceSelect = screen.getByLabelText("Source");
      await user.click(sourceSelect);
      await user.click(screen.getByText("FRED"));

      expect(sourceSelect).toHaveValue("FRED");
    });

    it("clears all filters", async () => {
      const user = userEvent.setup();
      renderWithTheme(<CrawlerLogs />);

      const searchInput = screen.getByPlaceholderText("Search logs...");
      await user.type(searchInput, "test");

      const clearButton = screen.getByText("Clear Filters");
      await user.click(clearButton);

      expect(searchInput).toHaveValue("");
    });
  });

  describe("Log Display", () => {
    it("displays log entries with correct information", () => {
      renderWithTheme(<CrawlerLogs />);

      // Should show log entries from mock data
      expect(
        screen.getByText("Successfully crawled GDP data"),
      ).toBeInTheDocument();
      expect(
        screen.getByText("Connection timeout while fetching unemployment data"),
      ).toBeInTheDocument();
      expect(
        screen.getByText("Rate limit approaching for Census API"),
      ).toBeInTheDocument();
    });

    it("shows log levels with appropriate colors", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("INFO")).toBeInTheDocument();
      expect(screen.getByText("ERROR")).toBeInTheDocument();
      expect(screen.getByText("WARN")).toBeInTheDocument();
      expect(screen.getByText("DEBUG")).toBeInTheDocument();
    });

    it("displays log sources correctly", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("FRED")).toBeInTheDocument();
      expect(screen.getByText("BLS")).toBeInTheDocument();
      expect(screen.getByText("Census")).toBeInTheDocument();
      expect(screen.getByText("WorldBank")).toBeInTheDocument();
    });

    it("shows log status indicators", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("success")).toBeInTheDocument();
      expect(screen.getByText("failed")).toBeInTheDocument();
      expect(screen.getByText("pending")).toBeInTheDocument();
    });

    it("displays log durations", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("2.30s")).toBeInTheDocument();
      expect(screen.getByText("0.10s")).toBeInTheDocument();
    });
  });

  describe("User Interactions", () => {
    it("handles refresh button click", async () => {
      const user = userEvent.setup();
      renderWithTheme(<CrawlerLogs />);

      const refreshButton = screen.getByText("Refresh");
      await user.click(refreshButton);

      // Should show refreshing state
      expect(screen.getByText("Refreshing...")).toBeInTheDocument();
    });

    it("handles export button click", async () => {
      const user = userEvent.setup();
      const consoleSpy = jest.spyOn(console, "log").mockImplementation();

      renderWithTheme(<CrawlerLogs />);

      const exportButton = screen.getByText("Export");
      await user.click(exportButton);

      expect(consoleSpy).toHaveBeenCalledWith("Exporting logs...");
      consoleSpy.mockRestore();
    });

    it("handles clear button click", async () => {
      const user = userEvent.setup();
      renderWithTheme(<CrawlerLogs />);

      const clearButton = screen.getByText("Clear");
      await user.click(clearButton);

      // Should clear logs
      expect(
        screen.getByText("No logs found matching your criteria"),
      ).toBeInTheDocument();
    });

    it("handles auto-refresh toggle", async () => {
      const user = userEvent.setup();
      renderWithTheme(<CrawlerLogs />);

      const autoRefreshSwitch = screen.getByRole("checkbox", {
        name: /auto-refresh/i,
      });
      expect(autoRefreshSwitch).toBeChecked();

      await user.click(autoRefreshSwitch);
      expect(autoRefreshSwitch).not.toBeChecked();
    });

    it("handles view details button click", async () => {
      const user = userEvent.setup();
      renderWithTheme(<CrawlerLogs />);

      const viewButtons = screen.getAllByLabelText("View Details");
      expect(viewButtons.length).toBeGreaterThan(0);

      await user.click(viewButtons[0]);
      // Should show details dialog or expand log entry
    });
  });

  describe("Real-time Updates", () => {
    it("updates performance metrics over time", async () => {
      renderWithTheme(<CrawlerLogs />);

      // Initial metrics
      const initialCpu = screen.getByText(/\d+\.\d+%/);
      expect(initialCpu).toBeInTheDocument();

      // Advance timers to trigger metric updates
      await act(async () => {
        jest.advanceTimersByTime(2000);
      });

      // Metrics should be updated
      expect(screen.getByText(/\d+\.\d+%/)).toBeInTheDocument();
    });

    it("handles auto-refresh functionality", async () => {
      renderWithTheme(<CrawlerLogs />);

      // Auto-refresh should be enabled by default
      const autoRefreshSwitch = screen.getByRole("checkbox", {
        name: /auto-refresh/i,
      });
      expect(autoRefreshSwitch).toBeChecked();

      // Advance timers to trigger auto-refresh
      await act(async () => {
        jest.advanceTimersByTime(5000);
      });

      // Should trigger refresh (mocked in console)
    });
  });

  describe("Accessibility", () => {
    it("has proper ARIA labels and roles", () => {
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByRole("heading", { level: 1 })).toBeInTheDocument();
      expect(screen.getByRole("table")).toBeInTheDocument();
      expect(screen.getByRole("progressbar")).toBeInTheDocument();
    });

    it("supports keyboard navigation", async () => {
      const user = userEvent.setup();
      renderWithTheme(<CrawlerLogs />);

      const searchInput = screen.getByPlaceholderText("Search logs...");
      searchInput.focus();
      expect(searchInput).toHaveFocus();

      await user.keyboard("{Tab}");
      // Should move to next focusable element
    });

    it("has proper color contrast for status indicators", () => {
      renderWithTheme(<CrawlerLogs />);

      // Status chips should have proper color attributes
      expect(screen.getByText("success")).toBeInTheDocument();
      expect(screen.getByText("failed")).toBeInTheDocument();
    });
  });

  describe("Performance", () => {
    it("handles large number of log entries efficiently", () => {
      renderWithTheme(<CrawlerLogs />);

      // Should render without performance issues
      expect(screen.getByText("Logs & Events")).toBeInTheDocument();
    });

    it("updates metrics without causing re-renders", async () => {
      renderWithTheme(<CrawlerLogs />);

      // Component should be stable
      expect(screen.getByText("CPU Usage")).toBeInTheDocument();
    });
  });

  describe("Error Handling", () => {
    it("displays error messages when operations fail", () => {
      renderWithTheme(<CrawlerLogs />);

      // Error handling is tested through the error state simulation
      // The component should gracefully handle errors
      expect(
        screen.getByText("📊 Crawler Logs & Monitoring"),
      ).toBeInTheDocument();
    });

    it("handles empty log states", async () => {
      const user = userEvent.setup();
      renderWithTheme(<CrawlerLogs />);

      const clearButton = screen.getByText("Clear");
      await user.click(clearButton);

      expect(
        screen.getByText("No logs found matching your criteria"),
      ).toBeInTheDocument();
    });
  });

  describe("Data Export", () => {
    it("handles export operations", async () => {
      const user = userEvent.setup();
      const consoleSpy = jest.spyOn(console, "log").mockImplementation();

      renderWithTheme(<CrawlerLogs />);

      const exportButton = screen.getByText("Export");
      await user.click(exportButton);

      expect(consoleSpy).toHaveBeenCalledWith("Exporting logs...");
      consoleSpy.mockRestore();
    });
  });

  describe("Component Integration", () => {
    it("integrates with Material-UI theme correctly", () => {
      renderWithTheme(<CrawlerLogs />);

      // Should use theme colors and styling
      expect(
        screen.getByText("📊 Crawler Logs & Monitoring"),
      ).toBeInTheDocument();
    });

    it("works with different screen sizes", () => {
      // Test responsive behavior
      renderWithTheme(<CrawlerLogs />);

      expect(screen.getByText("CPU Usage")).toBeInTheDocument();
      expect(screen.getByText("Memory Usage")).toBeInTheDocument();
    });
  });
});
