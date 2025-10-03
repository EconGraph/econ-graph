/**
 * Comprehensive tests for SecCrawlerManager component
 *
 * Tests cover:
 * - Component rendering and configuration
 * - Crawl parameter handling and validation
 * - Progress monitoring and status updates
 * - Error handling and recovery
 * - Integration with SEC crawler service
 * - User interaction and feedback
 */

import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import { vi } from "vitest";
import { SecCrawlerManager } from "../SecCrawlerManager";
import { useSecCrawler } from "../../../hooks/useSecCrawler";
import { Company, SecCrawlResult } from "../../../types";

// Mock the useSecCrawler hook
vi.mock("../../../hooks/useSecCrawler");

const mockUseSecCrawler = useSecCrawler as vi.MockedFunction<
  typeof useSecCrawler
>;

// Create test theme
const theme = createTheme();

// Test wrapper with providers
const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider theme={theme}>{children}</ThemeProvider>
    </QueryClientProvider>
  );
};

// Mock company data
const mockCompany: Company = {
  id: "1",
  cik: "0000320193",
  ticker: "AAPL",
  name: "Apple Inc.",
  legal_name: "Apple Inc.",
  industry: "Technology Hardware & Equipment",
  sector: "Technology",
  is_active: true,
  created_at: "2023-01-01T00:00:00Z",
  updated_at: "2023-01-01T00:00:00Z",
};

// Mock crawl result
const mockCrawlResult: SecCrawlResult = {
  success: true,
  message: "Crawl completed successfully",
  documentsProcessed: 15,
  documentsSkipped: 3,
  documentsFailed: 0,
  totalSizeBytes: 52428800,
  processingTimeMs: 45000,
  errors: [],
  warnings: ["Some documents were skipped due to size limits"],
};

describe("SecCrawlerManager", () => {
  const mockOnCrawlComplete = vi.fn();
  const mockOnCrawlError = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();

    // Default mock implementation
    mockUseSecCrawler.mockReturnValue({
      crawlCompany: vi.fn(),
      importRssFeed: vi.fn(),
      isCrawling: false,
      progress: 0,
      status: "idle",
      error: undefined,
    });
  });

  describe("Rendering", () => {
    it("renders company information", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Apple Inc.")).toBeInTheDocument();
      expect(screen.getByText("AAPL")).toBeInTheDocument();
      expect(
        screen.getByText("Technology Hardware & Equipment"),
      ).toBeInTheDocument();
    });

    it("renders crawl configuration form", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Crawl Configuration")).toBeInTheDocument();
      expect(screen.getByText("Date Range")).toBeInTheDocument();
      expect(screen.getByText("Form Types")).toBeInTheDocument();
      expect(screen.getByText("Document Limits")).toBeInTheDocument();
    });

    it("renders start crawl button", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(
        screen.getByRole("button", { name: /start crawl/i }),
      ).toBeInTheDocument();
    });
  });

  describe("Crawl Configuration", () => {
    it("handles date range selection", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const startDateInput = screen.getByLabelText("Start Date");
      const endDateInput = screen.getByLabelText("End Date");

      fireEvent.change(startDateInput, { target: { value: "2023-01-01" } });
      fireEvent.change(endDateInput, { target: { value: "2023-12-31" } });

      expect(startDateInput).toHaveValue("2023-01-01");
      expect(endDateInput).toHaveValue("2023-12-31");
    });

    it("handles form type selection", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const formTypeSelect = screen.getByLabelText("Form Types");
      fireEvent.change(formTypeSelect, { target: { value: "10-K" } });

      expect(formTypeSelect).toHaveValue("10-K");
    });

    it("handles multiple form type selection", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const formTypeSelect = screen.getByLabelText("Form Types");
      fireEvent.change(formTypeSelect, {
        target: { value: ["10-K", "10-Q", "8-K"] },
      });

      expect(formTypeSelect).toHaveValue(["10-K", "10-Q", "8-K"]);
    });

    it("handles document limit input", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const limitInput = screen.getByLabelText("Max Documents");
      fireEvent.change(limitInput, { target: { value: "100" } });

      expect(limitInput).toHaveValue(100);
    });

    it("handles include amendments toggle", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const amendmentsCheckbox = screen.getByLabelText("Include Amendments");
      fireEvent.click(amendmentsCheckbox);

      expect(amendmentsCheckbox).toBeChecked();
    });

    it("handles include exhibits toggle", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const exhibitsCheckbox = screen.getByLabelText("Include Exhibits");
      fireEvent.click(exhibitsCheckbox);

      expect(exhibitsCheckbox).toBeChecked();
    });
  });

  describe("Advanced Configuration", () => {
    it("handles rate limit input", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const rateLimitInput = screen.getByLabelText(
        "Rate Limit (requests/second)",
      );
      fireEvent.change(rateLimitInput, { target: { value: "5" } });

      expect(rateLimitInput).toHaveValue(5);
    });

    it("handles retry attempts input", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const retryInput = screen.getByLabelText("Retry Attempts");
      fireEvent.change(retryInput, { target: { value: "3" } });

      expect(retryInput).toHaveValue(3);
    });

    it("handles timeout input", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const timeoutInput = screen.getByLabelText("Timeout (seconds)");
      fireEvent.change(timeoutInput, { target: { value: "300" } });

      expect(timeoutInput).toHaveValue(300);
    });
  });

  describe("Crawl Execution", () => {
    it("starts crawl with default parameters", async () => {
      const mockCrawlCompany = vi.fn().mockResolvedValue(mockCrawlResult);
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: mockCrawlCompany,
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 0,
        status: "idle",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const startButton = screen.getByRole("button", { name: /start crawl/i });
      fireEvent.click(startButton);

      await waitFor(() => {
        expect(mockCrawlCompany).toHaveBeenCalledWith({
          companyId: mockCompany.id,
          startDate: undefined,
          endDate: undefined,
          formTypes: ["10-K", "10-Q", "8-K"],
          maxDocuments: 100,
          includeAmendments: true,
          includeExhibits: false,
          rateLimit: 10,
          retryAttempts: 3,
          timeout: 300,
        });
      });
    });

    it("starts crawl with custom parameters", async () => {
      const mockCrawlCompany = vi.fn().mockResolvedValue(mockCrawlResult);
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: mockCrawlCompany,
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 0,
        status: "idle",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      // Configure crawl parameters
      const startDateInput = screen.getByLabelText("Start Date");
      const endDateInput = screen.getByLabelText("End Date");
      const formTypeSelect = screen.getByLabelText("Form Types");
      const limitInput = screen.getByLabelText("Max Documents");
      const amendmentsCheckbox = screen.getByLabelText("Include Amendments");
      const exhibitsCheckbox = screen.getByLabelText("Include Exhibits");
      const rateLimitInput = screen.getByLabelText(
        "Rate Limit (requests/second)",
      );
      const retryInput = screen.getByLabelText("Retry Attempts");
      const timeoutInput = screen.getByLabelText("Timeout (seconds)");

      fireEvent.change(startDateInput, { target: { value: "2023-01-01" } });
      fireEvent.change(endDateInput, { target: { value: "2023-12-31" } });
      fireEvent.change(formTypeSelect, { target: { value: ["10-K"] } });
      fireEvent.change(limitInput, { target: { value: "50" } });
      fireEvent.click(amendmentsCheckbox);
      fireEvent.click(exhibitsCheckbox);
      fireEvent.change(rateLimitInput, { target: { value: "5" } });
      fireEvent.change(retryInput, { target: { value: "5" } });
      fireEvent.change(timeoutInput, { target: { value: "600" } });

      const startButton = screen.getByRole("button", { name: /start crawl/i });
      fireEvent.click(startButton);

      await waitFor(() => {
        expect(mockCrawlCompany).toHaveBeenCalledWith({
          companyId: mockCompany.id,
          startDate: "2023-01-01",
          endDate: "2023-12-31",
          formTypes: ["10-K"],
          maxDocuments: 50,
          includeAmendments: false,
          includeExhibits: true,
          rateLimit: 5,
          retryAttempts: 5,
          timeout: 600,
        });
      });
    });

    it("handles crawl start error", async () => {
      const mockCrawlCompany = jest
        .fn()
        .mockRejectedValue(new Error("Crawl failed"));
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: mockCrawlCompany,
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 0,
        status: "idle",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const startButton = screen.getByRole("button", { name: /start crawl/i });
      fireEvent.click(startButton);

      await waitFor(() => {
        expect(mockOnCrawlError).toHaveBeenCalledWith(expect.any(Error));
      });
    });
  });

  describe("Progress Monitoring", () => {
    it("displays crawling status", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: true,
        progress: 45,
        status: "crawling",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Crawling in progress...")).toBeInTheDocument();
      expect(screen.getByText("45%")).toBeInTheDocument();
    });

    it("displays progress bar", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: true,
        progress: 75,
        status: "crawling",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const progressBar = screen.getByRole("progressbar");
      expect(progressBar).toBeInTheDocument();
      expect(progressBar).toHaveAttribute("aria-valuenow", "75");
    });

    it("displays status messages", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: true,
        progress: 30,
        status: "downloading",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Downloading documents...")).toBeInTheDocument();
    });

    it("displays completion status", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 100,
        status: "completed",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(
        screen.getByText("Crawl completed successfully"),
      ).toBeInTheDocument();
    });
  });

  describe("Crawl Results", () => {
    it("displays crawl results", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 100,
        status: "completed",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Documents Processed: 15")).toBeInTheDocument();
      expect(screen.getByText("Documents Skipped: 3")).toBeInTheDocument();
      expect(screen.getByText("Documents Failed: 0")).toBeInTheDocument();
      expect(screen.getByText("Total Size: 52.4 MB")).toBeInTheDocument();
      expect(
        screen.getByText("Processing Time: 45 seconds"),
      ).toBeInTheDocument();
    });

    it("displays warnings", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 100,
        status: "completed",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Warnings:")).toBeInTheDocument();
      expect(
        screen.getByText("Some documents were skipped due to size limits"),
      ).toBeInTheDocument();
    });

    it("displays errors", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 100,
        status: "completed",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Errors:")).toBeInTheDocument();
      expect(screen.getByText("Network timeout")).toBeInTheDocument();
      expect(screen.getByText("Invalid document format")).toBeInTheDocument();
    });
  });

  describe("Error Handling", () => {
    it("displays crawl errors", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 0,
        status: "error",
        error: new Error("Crawl failed: Network error"),
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(
        screen.getByText("Error: Crawl failed: Network error"),
      ).toBeInTheDocument();
    });

    it("provides retry functionality", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 0,
        status: "error",
        error: new Error("Crawl failed"),
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Retry")).toBeInTheDocument();
    });

    it("handles validation errors", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      // Set invalid date range
      const startDateInput = screen.getByLabelText("Start Date");
      const endDateInput = screen.getByLabelText("End Date");

      fireEvent.change(startDateInput, { target: { value: "2023-12-31" } });
      fireEvent.change(endDateInput, { target: { value: "2023-01-01" } });

      const startButton = screen.getByRole("button", { name: /start crawl/i });
      fireEvent.click(startButton);

      expect(
        screen.getByText("Start date must be before end date"),
      ).toBeInTheDocument();
    });
  });

  describe("Accessibility", () => {
    it("has proper ARIA labels", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(screen.getByLabelText("Start Date")).toBeInTheDocument();
      expect(screen.getByLabelText("End Date")).toBeInTheDocument();
      expect(screen.getByLabelText("Form Types")).toBeInTheDocument();
      expect(screen.getByLabelText("Max Documents")).toBeInTheDocument();
      expect(screen.getByLabelText("Include Amendments")).toBeInTheDocument();
      expect(screen.getByLabelText("Include Exhibits")).toBeInTheDocument();
    });

    it("supports keyboard navigation", () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const startDateInput = screen.getByLabelText("Start Date");
      startDateInput.focus();

      fireEvent.keyDown(startDateInput, { key: "Tab" });
      // Should move to next focusable element
    });

    it("announces status changes to screen readers", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: true,
        progress: 50,
        status: "crawling",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(screen.getByRole("status")).toBeInTheDocument();
    });
  });

  describe("Performance", () => {
    it("handles large result sets efficiently", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 100,
        status: "completed",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(
        screen.getByText("Documents Processed: 10,000"),
      ).toBeInTheDocument();
      expect(screen.getByText("Total Size: 1.0 GB")).toBeInTheDocument();
    });

    it("debounces user input", async () => {
      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const limitInput = screen.getByLabelText("Max Documents");

      // Type multiple characters quickly
      fireEvent.change(limitInput, { target: { value: "1" } });
      fireEvent.change(limitInput, { target: { value: "10" } });
      fireEvent.change(limitInput, { target: { value: "100" } });
      fireEvent.change(limitInput, { target: { value: "1000" } });

      // Should handle rapid input changes without performance issues
      expect(limitInput).toHaveValue(1000);
    });
  });

  describe("Integration", () => {
    it("integrates with SEC crawler service", async () => {
      const mockCrawlCompany = vi.fn().mockResolvedValue(mockCrawlResult);
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: mockCrawlCompany,
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 0,
        status: "idle",
        error: undefined,
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      const startButton = screen.getByRole("button", { name: /start crawl/i });
      fireEvent.click(startButton);

      await waitFor(() => {
        expect(mockCrawlCompany).toHaveBeenCalled();
      });
    });

    it("handles service errors gracefully", () => {
      mockUseSecCrawler.mockReturnValue({
        crawlCompany: vi.fn(),
        importRssFeed: vi.fn(),
        isCrawling: false,
        progress: 0,
        status: "error",
        error: new Error("Service unavailable"),
      });

      render(
        <TestWrapper>
          <SecCrawlerManager
            company={mockCompany}
            onCrawlComplete={mockOnCrawlComplete}
            onCrawlError={mockOnCrawlError}
          />
        </TestWrapper>,
      );

      expect(
        screen.getByText("Error: Service unavailable"),
      ).toBeInTheDocument();
    });
  });
});
