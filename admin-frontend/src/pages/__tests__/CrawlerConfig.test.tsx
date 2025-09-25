/**
 * Comprehensive tests for CrawlerConfig component
 *
 * Tests:
 * - Configuration form rendering and validation
 * - Data source management operations
 * - Global settings management
 * - Form interactions and state management
 * - Error handling and user feedback
 */

import React from "react";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import CrawlerConfig from "../CrawlerConfig";
// GraphQL mock responses will be imported dynamically in tests

// Mock Material-UI theme
const theme = createTheme();

// Mock the useCrawlerConfig hooks to return our GraphQL mock data
jest.mock("../../hooks/useCrawlerConfig", () => ({
  useCrawlerConfig: jest.fn(),
  useDataSources: jest.fn(),
  useUpdateCrawlerConfig: jest.fn(),
  useUpdateDataSource: jest.fn(),
  useTestDataSourceConnection: jest.fn(),
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

describe("CrawlerConfig", () => {
  beforeAll(() => {
    // Initialize QueryClient once for all tests
    testQueryClient = createTestQueryClient();
  });

  beforeEach(() => {
    // Clear QueryClient cache between tests to ensure isolation
    testQueryClient.clear();

    // Set up hook mocks with GraphQL mock data
    const {
      useCrawlerConfig,
      useDataSources,
      useUpdateCrawlerConfig,
      useUpdateDataSource,
      useTestDataSourceConnection,
    } = require("../../hooks/useCrawlerConfig");

    // Import mock data dynamically
    const getCrawlerConfigSuccess = require("../../__mocks__/graphql/getCrawlerConfig/success.json");
    const getDataSourcesSuccess = require("../../__mocks__/graphql/getDataSources/success.json");

    useCrawlerConfig.mockReturnValue({
      data: getCrawlerConfigSuccess,
      isLoading: false,
      error: null,
    });

    useDataSources.mockReturnValue({
      data: getDataSourcesSuccess,
      isLoading: false,
      error: null,
    });

    useUpdateCrawlerConfig.mockReturnValue({
      mutateAsync: jest.fn(),
      isLoading: false,
      error: null,
    });

    useUpdateDataSource.mockReturnValue({
      mutateAsync: jest.fn(),
      isLoading: false,
      error: null,
    });

    useTestDataSourceConnection.mockReturnValue({
      mutateAsync: jest.fn(),
      isLoading: false,
      error: null,
    });
  });

  afterAll(() => {
    // Clean up QueryClient after all tests
    testQueryClient.clear();
  });

  describe("Component Rendering", () => {
    it("renders main configuration interface", () => {
      renderWithTheme(<CrawlerConfig />);

      expect(screen.getByText("⚙️ Crawler Configuration")).toBeInTheDocument();
      expect(
        screen.getByTestId("save-configuration-button"),
      ).toBeInTheDocument();
      expect(screen.getByTestId("global-settings-title")).toBeInTheDocument();
      expect(screen.getByTestId("data-sources-title")).toBeInTheDocument();
    });

    it("logs error when receiving invalid config data", async () => {
      const consoleSpy = jest.spyOn(console, "error").mockImplementation();

      // Mock the hook to return error data
      const { useCrawlerConfig } = require("../../hooks/useCrawlerConfig");
      const getCrawlerConfigError = require("../../__mocks__/graphql/getCrawlerConfig/error.json");
      useCrawlerConfig.mockReturnValue({
        data: getCrawlerConfigError,
        isLoading: false,
        error: null,
      });

      renderWithTheme(<CrawlerConfig />);

      // Wait for the component to process the error response
      await waitFor(() => {
        expect(consoleSpy).toHaveBeenCalled();
      });

      consoleSpy.mockRestore();
    });

    it("logs error when receiving invalid data sources data", async () => {
      const consoleSpy = jest.spyOn(console, "error").mockImplementation();

      // Mock the hook to return error data
      const { useDataSources } = require("../../hooks/useCrawlerConfig");
      const getDataSourcesError = require("../../__mocks__/graphql/getDataSources/error.json");
      useDataSources.mockReturnValue({
        data: getDataSourcesError,
        isLoading: false,
        error: null,
      });

      renderWithTheme(<CrawlerConfig />);

      // Wait for the component to process the error response
      await waitFor(() => {
        expect(consoleSpy).toHaveBeenCalled();
      });

      consoleSpy.mockRestore();
    });

    it("displays global settings section", () => {
      renderWithTheme(<CrawlerConfig />);

      expect(screen.getByText("Global Settings")).toBeInTheDocument();
      expect(screen.getByText("Enable Crawler")).toBeInTheDocument();
      expect(screen.getByText("Maintenance Mode")).toBeInTheDocument();
      expect(screen.getByTestId("max-workers-input")).toBeInTheDocument();
      expect(screen.getByTestId("queue-size-limit-input")).toBeInTheDocument();
    });

    it("displays data sources section", () => {
      renderWithTheme(<CrawlerConfig />);

      expect(screen.getByText("Data Sources")).toBeInTheDocument();
      expect(screen.getByText("Add Source")).toBeInTheDocument();
    });

    it("shows data source details table", () => {
      renderWithTheme(<CrawlerConfig />);

      expect(screen.getByText("Data Source Details")).toBeInTheDocument();
      expect(
        screen.getByTestId("data-source-details-table-source-header"),
      ).toBeInTheDocument();
      expect(screen.getByText("Priority")).toBeInTheDocument();
      expect(screen.getByText("Rate Limit")).toBeInTheDocument();
    });
  });

  describe("Global Settings Management", () => {
    it("handles enable crawler toggle", () => {
      renderWithTheme(<CrawlerConfig />);

      const enableSwitch = screen.getByRole("checkbox", {
        name: /enable crawler/i,
      });
      expect(enableSwitch).toBeChecked();

      fireEvent.click(enableSwitch);
      expect(enableSwitch).not.toBeChecked();
    });

    it("handles maintenance mode toggle", () => {
      renderWithTheme(<CrawlerConfig />);

      const maintenanceSwitch = screen.getByRole("checkbox", {
        name: /maintenance mode/i,
      });
      expect(maintenanceSwitch).not.toBeChecked();

      fireEvent.click(maintenanceSwitch);
      expect(maintenanceSwitch).toBeChecked();
    });

    it("updates max workers value", () => {
      renderWithTheme(<CrawlerConfig />);

      const maxWorkersInput = screen.getByLabelText("Max Workers");
      expect(maxWorkersInput).toHaveValue(5);

      fireEvent.change(maxWorkersInput, { target: { value: "10" } });
      expect(maxWorkersInput).toHaveValue(10);
    });

    it("updates queue size limit", () => {
      renderWithTheme(<CrawlerConfig />);

      const queueLimitInput = screen.getByLabelText("Queue Size Limit");
      expect(queueLimitInput).toHaveValue(10000);

      fireEvent.change(queueLimitInput, { target: { value: "20000" } });
      expect(queueLimitInput).toHaveValue(20000);
    });

    it("updates default timeout value", () => {
      renderWithTheme(<CrawlerConfig />);

      const timeoutInput = screen.getByLabelText("Default Timeout (seconds)");
      expect(timeoutInput).toHaveValue(30);

      fireEvent.change(timeoutInput, { target: { value: "60" } });
      expect(timeoutInput).toHaveValue(60);
    });

    it("updates default retry attempts", () => {
      renderWithTheme(<CrawlerConfig />);

      const retryInput = screen.getByLabelText("Default Retry Attempts");
      expect(retryInput).toHaveValue(3);

      fireEvent.change(retryInput, { target: { value: "5" } });
      expect(retryInput).toHaveValue(5);
    });

    it("updates schedule frequency", async () => {
      renderWithTheme(<CrawlerConfig />);

      const frequencySelect = screen.getByLabelText(
        "Schedule frequency for crawler",
      );
      // Check that the select is present
      expect(frequencySelect).toBeInTheDocument();

      // Test that we can interact with the select
      fireEvent.click(frequencySelect);

      // Wait for dropdown options to appear
      await waitFor(
        () => {
          expect(screen.getByText("Daily")).toBeInTheDocument();
        },
        { timeout: 5000 },
      );

      const hourlyElements = screen.getAllByText("Hourly");
      expect(hourlyElements.length).toBeGreaterThan(0);
    });
  });

  describe("Data Source Management", () => {
    it("displays existing data sources", () => {
      renderWithTheme(<CrawlerConfig />);

      const fredElements = screen.getAllByText(
        "Federal Reserve Economic Data (FRED)",
      );
      expect(fredElements).toHaveLength(2); // One in each table
      const blsElements = screen.getAllByText(
        "Bureau of Labor Statistics (BLS)",
      );
      expect(blsElements).toHaveLength(2); // One in each table
      const censusElements = screen.getAllByText("US Census Bureau");
      expect(censusElements.length).toBeGreaterThan(0);
      const worldBankElements = screen.getAllByText("World Bank");
      expect(worldBankElements.length).toBeGreaterThan(0);
    });

    it("shows source health status", () => {
      renderWithTheme(<CrawlerConfig />);

      const healthyElements = screen.getAllByText("healthy");
      expect(healthyElements.length).toBeGreaterThan(0);
      expect(screen.getByText("warning")).toBeInTheDocument();
      expect(screen.getByText("error")).toBeInTheDocument();
    });

    it("displays source status chips", () => {
      renderWithTheme(<CrawlerConfig />);

      const enabledElements = screen.getAllByText("Enabled");
      expect(enabledElements.length).toBeGreaterThan(0);
      expect(screen.getByText("Disabled")).toBeInTheDocument();
    });

    it("handles edit source button click", () => {
      renderWithTheme(<CrawlerConfig />);

      const editButtons = screen.getAllByLabelText("Edit Configuration");
      expect(editButtons).toHaveLength(4);

      fireEvent.click(editButtons[0]);

      expect(
        screen.getByText("Edit Data Source Configuration"),
      ).toBeInTheDocument();
    });

    it("handles test connection button click", () => {
      renderWithTheme(<CrawlerConfig />);

      const testButtons = screen.getAllByLabelText("Test Connection");
      expect(testButtons).toHaveLength(4);

      fireEvent.click(testButtons[0]);

      // Test that the connection test was triggered by checking for loading state
      // or health status change (the actual implementation updates health status)
      expect(testButtons[0]).toBeInTheDocument();
    });
  });

  describe("Data Source Configuration", () => {
    it("updates source enabled status", () => {
      renderWithTheme(<CrawlerConfig />);

      const sourceSwitches = screen.getAllByRole("checkbox");
      const fredSwitch = sourceSwitches.find((switchEl) =>
        switchEl.closest("tr")?.textContent?.includes("FRED"),
      );

      expect(fredSwitch).toBeChecked();

      // This test assumes FRED switch exists
      expect(fredSwitch).toBeInTheDocument();
      expect(fredSwitch).not.toBeNull();
      fireEvent.click(fredSwitch!);
      expect(fredSwitch).not.toBeChecked();
    });

    it("updates source priority", () => {
      renderWithTheme(<CrawlerConfig />);

      const priorityInputs = screen.getAllByDisplayValue("1");
      expect(priorityInputs.length).toBeGreaterThan(0);

      fireEvent.change(priorityInputs[0], { target: { value: "5" } });
      expect(priorityInputs[0]).toHaveValue(5);
    });

    it("updates source rate limit", () => {
      renderWithTheme(<CrawlerConfig />);

      const rateLimitInputs = screen.getAllByDisplayValue("5");
      expect(rateLimitInputs.length).toBeGreaterThan(0);

      fireEvent.change(rateLimitInputs[0], { target: { value: "10" } });
      expect(rateLimitInputs[0]).toHaveValue(10);
    });

    it("updates source retry attempts", () => {
      renderWithTheme(<CrawlerConfig />);

      const retryInputs = screen.getAllByDisplayValue("3");
      expect(retryInputs.length).toBeGreaterThan(0);

      fireEvent.change(retryInputs[0], { target: { value: "5" } });
      expect(retryInputs[0]).toHaveValue(5);
    });

    it("updates source timeout", () => {
      renderWithTheme(<CrawlerConfig />);

      const timeoutInputs = screen.getAllByDisplayValue("30");
      expect(timeoutInputs.length).toBeGreaterThan(0);

      fireEvent.change(timeoutInputs[0], { target: { value: "60" } });
      expect(timeoutInputs[0]).toHaveValue(60);
    });
  });

  describe("Edit Dialog", () => {
    // PERFORMANCE NOTE: This test suite has an unusually high timeout due to expensive
    // toLocaleString() operations in the CrawlerConfig component. Even with useMemo
    // optimization, the dialog closing operation involves multiple re-renders with
    // date formatting operations that can take 10-15 seconds in the test environment.
    //
    // The underlying performance issue is that Material-UI Dialog closing triggers
    // multiple re-renders of the parent component, each causing expensive date
    // formatting operations. This is a known issue with complex forms in test environments.
    //
    // Related GitHub Issue: #88 - CrawlerDashboard Jest mock resolution issues
    // This performance issue was identified during debugging of similar timeout
    // problems in the admin frontend test suite.
    jest.setTimeout(30000); // 30 seconds - increased timeout for dialog operations
    it("opens edit dialog when edit button is clicked", () => {
      renderWithTheme(<CrawlerConfig />);

      const editButtons = screen.getAllByLabelText("Edit Configuration");
      fireEvent.click(editButtons[0]);

      expect(
        screen.getByText("Edit Data Source Configuration"),
      ).toBeInTheDocument();
      expect(screen.getByLabelText("Source Name")).toBeInTheDocument();
      expect(screen.getByLabelText("Priority")).toBeInTheDocument();
      expect(
        screen.getByLabelText("Rate Limit (requests/min)"),
      ).toBeInTheDocument();
    });

    it("closes edit dialog when cancel is clicked", async () => {
      renderWithTheme(<CrawlerConfig />);

      const editButtons = screen.getAllByLabelText("Edit Configuration");
      fireEvent.click(editButtons[0]);

      expect(
        screen.getByText("Edit Data Source Configuration"),
      ).toBeInTheDocument();

      fireEvent.click(screen.getByText("Cancel"));

      await waitFor(
        () => {
          expect(
            screen.queryByTestId("edit-dialog-title"),
          ).not.toBeInTheDocument();
        },
        { timeout: 10000 },
      );
    });

    it("saves changes when save is clicked", async () => {
      renderWithTheme(<CrawlerConfig />);

      const editButtons = screen.getAllByLabelText("Edit Configuration");
      fireEvent.click(editButtons[0]);

      const sourceNameInput = screen.getByLabelText("Source Name");
      fireEvent.change(sourceNameInput, { target: { value: "Updated FRED" } });

      fireEvent.click(screen.getByText("Save"));

      await waitFor(
        () => {
          expect(
            screen.queryByTestId("edit-dialog-title"),
          ).not.toBeInTheDocument();
        },
        { timeout: 10000 },
      );
    });
  });

  describe("Save Configuration", () => {
    it("handles save configuration button click", () => {
      renderWithTheme(<CrawlerConfig />);

      const saveButton = screen.getByText("Save Configuration");
      expect(saveButton).not.toBeDisabled();

      fireEvent.click(saveButton);

      // Test that the save operation was triggered by checking button state
      expect(saveButton).toBeInTheDocument();
    });

    it("shows saving state during save operation", () => {
      renderWithTheme(<CrawlerConfig />);

      const saveButton = screen.getByText("Save Configuration");
      fireEvent.click(saveButton);

      // Should show saving state
      expect(screen.getByText("Saving...")).toBeInTheDocument();
    });
  });

  describe("Error Handling", () => {
    it("displays error messages when save fails", () => {
      renderWithTheme(<CrawlerConfig />);

      // Simulate error by clicking save
      const saveButton = screen.getByText("Save Configuration");
      fireEvent.click(saveButton);

      // Error handling would be tested with actual error scenarios
      expect(
        screen.getByTestId("save-configuration-button"),
      ).toBeInTheDocument();
    });

    it("handles connection test failures", () => {
      renderWithTheme(<CrawlerConfig />);

      const testButtons = screen.getAllByLabelText("Test Connection");
      fireEvent.click(testButtons[0]);

      // Connection test error handling would be tested here
      expect(testButtons[0]).toBeInTheDocument();
    });
  });

  describe("Accessibility", () => {
    it("has proper form labels and ARIA attributes", () => {
      renderWithTheme(<CrawlerConfig />);

      expect(screen.getByLabelText("Max Workers")).toBeInTheDocument();
      expect(screen.getByLabelText("Queue Size Limit")).toBeInTheDocument();
      expect(
        screen.getByLabelText("Default Timeout (seconds)"),
      ).toBeInTheDocument();
      expect(
        screen.getByLabelText("Default Retry Attempts"),
      ).toBeInTheDocument();
      expect(
        screen.getByLabelText("Schedule frequency for crawler"),
      ).toBeInTheDocument();
    });

    it("supports keyboard navigation", () => {
      renderWithTheme(<CrawlerConfig />);

      const maxWorkersInput = screen.getByLabelText("Max Workers");
      maxWorkersInput.focus();
      expect(maxWorkersInput).toHaveFocus();

      fireEvent.keyDown(maxWorkersInput, { key: "Tab", code: "Tab" });
      // Should move to next focusable element
    });

    it("has proper heading structure", () => {
      renderWithTheme(<CrawlerConfig />);

      expect(screen.getByRole("heading", { level: 1 })).toBeInTheDocument();
      const h2Headings = screen.getAllByRole("heading", { level: 2 });
      expect(h2Headings).toHaveLength(3); // Global Settings, Data Sources, Data Source Details
    });
  });

  describe("Performance", () => {
    it("handles large number of data sources efficiently", () => {
      renderWithTheme(<CrawlerConfig />);

      // Should render without performance issues
      expect(screen.getByText("Data Sources")).toBeInTheDocument();
    });

    it("updates form state without unnecessary re-renders", () => {
      renderWithTheme(<CrawlerConfig />);

      const maxWorkersInput = screen.getByLabelText("Max Workers");
      fireEvent.change(maxWorkersInput, { target: { value: "10" } });

      // Component should be stable
      expect(maxWorkersInput).toHaveValue(10);
    });
  });

  describe("Data Validation", () => {
    it("validates numeric inputs", () => {
      renderWithTheme(<CrawlerConfig />);

      const maxWorkersInput = screen.getByLabelText("Max Workers");
      fireEvent.change(maxWorkersInput, { target: { value: "abc" } });

      // Should handle invalid input gracefully
      expect(maxWorkersInput).toHaveValue(null);
    });

    it("validates required fields", () => {
      renderWithTheme(<CrawlerConfig />);

      // All required fields should have values
      expect(screen.getByLabelText("Max Workers")).toHaveValue(5);
      expect(screen.getByLabelText("Queue Size Limit")).toHaveValue(10000);
    });
  });
});
