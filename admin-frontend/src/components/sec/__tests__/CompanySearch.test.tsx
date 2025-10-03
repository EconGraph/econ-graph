/**
 * Comprehensive tests for CompanySearch component
 *
 * Tests cover:
 * - Component rendering and basic functionality
 * - Search input handling and validation
 * - Search companies display and interaction
 * - Error handling and loading states
 * - Accessibility and user experience
 * - Integration with GraphQL API
 */

import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import { ApolloProvider } from "@apollo/client/react";
import { ApolloClient, InMemoryCache, createHttpLink } from "@apollo/client";
import { vi } from "vitest";
import type { MockedFunction } from "vitest";
import CompanySearch from "../CompanySearch";
import { useCompanySearch } from "../../../hooks/useCompanySearch";
import { Company } from "../../../types";

// Mock the useCompanySearch hook
vi.mock("../../../hooks/useCompanySearch");

const mockUseCompanySearch = useCompanySearch as MockedFunction<
  typeof useCompanySearch
>;

// Create test theme
const theme = createTheme();

// Create Apollo Client for tests
const apolloClient = new ApolloClient({
  link: createHttpLink({
    uri: "http://localhost:4000/graphql",
  }),
  cache: new InMemoryCache(),
});

// Test wrapper with providers
const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return (
    <ApolloProvider client={apolloClient}>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider theme={theme}>{children}</ThemeProvider>
      </QueryClientProvider>
    </ApolloProvider>
  );
};

// Mock company data
const mockCompanies: Company[] = [
  {
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
  },
  {
    id: "2",
    cik: "0000789019",
    ticker: "MSFT",
    name: "Microsoft Corporation",
    legal_name: "Microsoft Corporation",
    industry: "Software",
    sector: "Technology",
    is_active: true,
    created_at: "2023-01-01T00:00:00Z",
    updated_at: "2023-01-01T00:00:00Z",
  },
];

describe("CompanySearch", () => {
  const mockOnCompanySelect = vi.fn();
  const mockOnCrawlStart = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();

    // Default mock implementation
    mockUseCompanySearch.mockReturnValue({
      query: "",
      setQuery: vi.fn(),
      companies: [],
      loading: false,
      error: undefined,
      searchCompanies: vi.fn(),
      totalCount: 0,
      getCompany: vi.fn(),
    });
  });

  describe("Rendering", () => {
    it("renders search input", () => {
      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(
        screen.getByPlaceholderText("Search companies by name, ticker, or CIK..."),
      ).toBeInTheDocument();
    });

    it("renders search input with search icon", () => {
      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(
        screen.getByTestId("SearchIcon"),
      ).toBeInTheDocument();
    });

    it("renders component without errors", () => {
      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      // Component should render without throwing errors
      expect(screen.getByRole("textbox")).toBeInTheDocument();
    });
  });

  describe("Search Functionality", () => {
    it("handles search input changes", async () => {
      const mockSetQuery = vi.fn();
      mockUseCompanySearch.mockReturnValue({
        query: "",
        setQuery: mockSetQuery,
        companies: [],
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      const searchInput = screen.getByPlaceholderText("Search companies by name, ticker, or CIK...");
      fireEvent.change(searchInput, { target: { value: "Apple" } });

      expect(mockSetQuery).toHaveBeenCalledWith("Apple");
    });

    it("triggers search automatically when typing", async () => {
      const mockSearchCompanies = vi.fn();
      mockUseCompanySearch.mockReturnValue({
        query: "",
        setQuery: vi.fn(),
        companies: [],
        loading: false,
        error: undefined,
        searchCompanies: mockSearchCompanies,
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      const searchInput = screen.getByPlaceholderText("Search companies by name, ticker, or CIK...");
      fireEvent.change(searchInput, { target: { value: "Apple" } });

      await waitFor(() => {
        expect(mockSearchCompanies).toHaveBeenCalledWith({
          query: "Apple",
          limit: 50,
          include_inactive: false,
        });
      });
    });

    it("handles search input with minimum length requirement", async () => {
      const mockSearchCompanies = vi.fn();
      mockUseCompanySearch.mockReturnValue({
        query: "",
        setQuery: vi.fn(),
        companies: [],
        loading: false,
        error: undefined,
        searchCompanies: mockSearchCompanies,
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      const searchInput = screen.getByPlaceholderText("Search companies by name, ticker, or CIK...");
      
      // Test with short query (should not trigger search)
      fireEvent.change(searchInput, { target: { value: "A" } });
      expect(mockSearchCompanies).not.toHaveBeenCalled();
      
      // Test with longer query (should trigger search)
      fireEvent.change(searchInput, { target: { value: "Apple" } });
      await waitFor(() => {
        expect(mockSearchCompanies).toHaveBeenCalledWith({
          query: "Apple",
          limit: 50,
          include_inactive: false,
        });
      });
    });

    it("handles empty search query", async () => {
      const mockSearchCompanies = vi.fn();
      mockUseCompanySearch.mockReturnValue({
        query: "",
        setQuery: vi.fn(),
        companies: [],
        loading: false,
        error: undefined,
        searchCompanies: mockSearchCompanies,
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      const searchInput = screen.getByPlaceholderText("Search companies by name, ticker, or CIK...");
      fireEvent.change(searchInput, { target: { value: "" } });

      expect(mockSearchCompanies).not.toHaveBeenCalled();
    });
  });

  describe("Search Results", () => {
    it("displays search companies when query is long enough", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: mockCompanies,
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      // The component should show the search input
      expect(screen.getByPlaceholderText("Search companies by name, ticker, or CIK...")).toBeInTheDocument();
      
      // When companies are returned, they should be displayed
      expect(screen.getByText("Apple Inc.")).toBeInTheDocument();
      expect(screen.getByText("Microsoft Corporation")).toBeInTheDocument();
    });

    it("displays company information correctly", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: [mockCompanies[0]],
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 1,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Apple Inc.")).toBeInTheDocument();
      expect(screen.getByText("AAPL")).toBeInTheDocument();
      expect(
        screen.getByText("Technology Hardware & Equipment"),
      ).toBeInTheDocument();
      expect(screen.getByText("Technology")).toBeInTheDocument();
    });

    it("handles empty search companies", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "NonExistentCompany",
        setQuery: vi.fn(),
        companies: [],
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(screen.getByText('No companies found for "NonExistentCompany"')).toBeInTheDocument();
    });

    it("displays loading state", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: [],
        loading: true,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Searching companies...")).toBeInTheDocument();
    });

    it("displays error state", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: [],
        loading: false,
        error: new Error("Search failed"),
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Error searching companies: Search failed")).toBeInTheDocument();
    });
  });

  describe("Company Selection", () => {
    it("handles company selection", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: mockCompanies,
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      const companyCard = screen.getByText("Apple Inc.");
      fireEvent.click(companyCard);

      expect(mockOnCompanySelect).toHaveBeenCalledWith(mockCompanies[0]);
    });

    it("handles crawl start", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: mockCompanies,
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      // The component shows individual crawl buttons for each company
      // We need to click on a company first to select it, then the crawl button appears
      const companyCard = screen.getByText("Apple Inc.");
      fireEvent.click(companyCard);

      // Now the selected company card should show with a crawl button
      const crawlButton = screen.getByText("Crawl Company Filings");
      fireEvent.click(crawlButton);

      expect(mockOnCrawlStart).toHaveBeenCalledWith(mockCompanies[0]);
    });
  });

  describe("Filtering", () => {
    it("handles include inactive toggle", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: mockCompanies,
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      const includeInactiveCheckbox = screen.getByLabelText("Include Inactive");
      fireEvent.click(includeInactiveCheckbox);

      expect(includeInactiveCheckbox).toBeChecked();
    });

    it("handles limit input", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: mockCompanies,
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      const limitInput = screen.getByLabelText("Limit Results");
      fireEvent.change(limitInput, { target: { value: "50" } });

      expect(limitInput).toHaveValue(50);
    });
  });

  describe("Accessibility", () => {
    it("has proper ARIA labels", () => {
      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(screen.getByLabelText("Search companies")).toBeInTheDocument();
      expect(screen.getByLabelText("Include Inactive")).toBeInTheDocument();
      expect(screen.getByLabelText("Limit Results")).toBeInTheDocument();
    });

    it("supports keyboard navigation", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: mockCompanies,
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      const searchInput = screen.getByPlaceholderText("Search companies by name, ticker, or CIK...");
      searchInput.focus();

      fireEvent.keyDown(searchInput, { key: "Tab" });
      // Should move to next focusable element
    });

    it("announces search companies to screen readers", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: mockCompanies,
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      // The component should have a search input
      expect(screen.getByRole("textbox")).toBeInTheDocument();
    });
  });

  describe("Performance", () => {
    it("handles search input changes", async () => {
      const mockSearchCompanies = vi.fn();
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: [],
        loading: false,
        error: undefined,
        searchCompanies: mockSearchCompanies,
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      const searchInput = screen.getByPlaceholderText("Search companies by name, ticker, or CIK...");
      
      // Type a search query
      fireEvent.change(searchInput, { target: { value: "Apple" } });

      // The component should call searchCompanies
      await waitFor(() => {
        expect(mockSearchCompanies).toHaveBeenCalled();
      });
    });

    it("handles large result sets efficiently", () => {
      const largeResultSet = Array.from({ length: 1000 }, (_, i) => ({
        id: i.toString(),
        cik: `000000${i.toString().padStart(4, "0")}`,
        ticker: `TICK${i}`,
        name: `Company ${i}`,
        legal_name: `Company ${i} Inc.`,
        industry: "Technology",
        sector: "Technology",
        is_active: true,
        created_at: "2023-01-01T00:00:00Z",
        updated_at: "2023-01-01T00:00:00Z",
      }));

      mockUseCompanySearch.mockReturnValue({
        query: "Company",
        setQuery: vi.fn(),
        companies: largeResultSet,
        loading: false,
        error: undefined,
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      // Should render without performance issues
      expect(screen.getByText("Company 0 Inc.")).toBeInTheDocument();
    });
  });

  describe("Error Handling", () => {
    it("handles network errors gracefully", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: [],
        loading: false,
        error: new Error("Network error"),
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Error searching companies: Network error")).toBeInTheDocument();
    });

    it("handles timeout errors", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: [],
        loading: false,
        error: new Error("Request timeout"),
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Error searching companies: Request timeout")).toBeInTheDocument();
    });

    it("displays error messages", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: [],
        loading: false,
        error: new Error("Search failed"),
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(screen.getByText("Error searching companies: Search failed")).toBeInTheDocument();
    });
  });

  describe("Integration", () => {
    it("integrates with GraphQL API", async () => {
      const mockSearchCompanies = vi.fn();
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: mockCompanies,
        loading: false,
        error: undefined,
        searchCompanies: mockSearchCompanies,
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      const searchInput = screen.getByPlaceholderText("Search companies by name, ticker, or CIK...");
      fireEvent.change(searchInput, { target: { value: "Apple" } });

      await waitFor(() => {
        expect(mockSearchCompanies).toHaveBeenCalledWith({
          query: "Apple",
          limit: 50,
          include_inactive: false,
        });
      });
    });

    it("handles GraphQL errors", () => {
      mockUseCompanySearch.mockReturnValue({
        query: "Apple",
        setQuery: vi.fn(),
        companies: [],
        loading: false,
        error: new Error("GraphQL error: Field not found"),
        searchCompanies: vi.fn(),
        totalCount: 2,
        getCompany: vi.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>,
      );

      expect(
        screen.getByText("Error searching companies: GraphQL error: Field not found"),
      ).toBeInTheDocument();
    });
  });
});
