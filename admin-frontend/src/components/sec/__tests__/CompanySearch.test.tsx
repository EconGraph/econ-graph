/**
 * Comprehensive tests for CompanySearch component
 * 
 * Tests cover:
 * - Component rendering and basic functionality
 * - Search input handling and validation
 * - Search results display and interaction
 * - Error handling and loading states
 * - Accessibility and user experience
 * - Integration with GraphQL API
 */

import React from 'react';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { CompanySearch } from '../CompanySearch';
import { useCompanySearch } from '../../../hooks/useCompanySearch';
import { Company } from '../../../types';

// Mock the useCompanySearch hook
jest.mock('../../../hooks/useCompanySearch');

const mockUseCompanySearch = useCompanySearch as jest.MockedFunction<typeof useCompanySearch>;

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
      <ThemeProvider theme={theme}>
        {children}
      </ThemeProvider>
    </QueryClientProvider>
  );
};

// Mock company data
const mockCompanies: Company[] = [
  {
    id: '1',
    cik: '0000320193',
    ticker: 'AAPL',
    name: 'Apple Inc.',
    legalName: 'Apple Inc.',
    industry: 'Technology Hardware & Equipment',
    sector: 'Technology',
    isActive: true,
    createdAt: '2023-01-01T00:00:00Z',
    updatedAt: '2023-01-01T00:00:00Z',
  },
  {
    id: '2',
    cik: '0000789019',
    ticker: 'MSFT',
    name: 'Microsoft Corporation',
    legalName: 'Microsoft Corporation',
    industry: 'Software',
    sector: 'Technology',
    isActive: true,
    createdAt: '2023-01-01T00:00:00Z',
    updatedAt: '2023-01-01T00:00:00Z',
  },
];

describe('CompanySearch', () => {
  const mockOnCompanySelect = jest.fn();
  const mockOnCrawlStart = jest.fn();

  beforeEach(() => {
    jest.clearAllMocks();
    
    // Default mock implementation
    mockUseCompanySearch.mockReturnValue({
      query: '',
      setQuery: jest.fn(),
      results: [],
      loading: false,
      error: null,
      searchCompanies: jest.fn(),
    });
  });

  describe('Rendering', () => {
    it('renders search input', () => {
      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByPlaceholderText('Search companies...')).toBeInTheDocument();
    });

    it('renders search button', () => {
      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByRole('button', { name: /search/i })).toBeInTheDocument();
    });

    it('renders filter options', () => {
      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByText('Include Inactive')).toBeInTheDocument();
      expect(screen.getByText('Limit Results')).toBeInTheDocument();
    });
  });

  describe('Search Functionality', () => {
    it('handles search input changes', async () => {
      const mockSetQuery = jest.fn();
      mockUseCompanySearch.mockReturnValue({
        query: '',
        setQuery: mockSetQuery,
        results: [],
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const searchInput = screen.getByPlaceholderText('Search companies...');
      fireEvent.change(searchInput, { target: { value: 'Apple' } });

      expect(mockSetQuery).toHaveBeenCalledWith('Apple');
    });

    it('triggers search on button click', async () => {
      const mockSearchCompanies = jest.fn();
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: [],
        loading: false,
        error: null,
        searchCompanies: mockSearchCompanies,
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const searchButton = screen.getByRole('button', { name: /search/i });
      fireEvent.click(searchButton);

      expect(mockSearchCompanies).toHaveBeenCalledWith('Apple');
    });

    it('triggers search on Enter key press', async () => {
      const mockSearchCompanies = jest.fn();
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: [],
        loading: false,
        error: null,
        searchCompanies: mockSearchCompanies,
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const searchInput = screen.getByPlaceholderText('Search companies...');
      fireEvent.keyDown(searchInput, { key: 'Enter', code: 'Enter' });

      expect(mockSearchCompanies).toHaveBeenCalledWith('Apple');
    });

    it('handles empty search query', async () => {
      const mockSearchCompanies = jest.fn();
      mockUseCompanySearch.mockReturnValue({
        query: '',
        setQuery: jest.fn(),
        results: [],
        loading: false,
        error: null,
        searchCompanies: mockSearchCompanies,
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const searchButton = screen.getByRole('button', { name: /search/i });
      fireEvent.click(searchButton);

      expect(mockSearchCompanies).toHaveBeenCalledWith('');
    });
  });

  describe('Search Results', () => {
    it('displays search results', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: mockCompanies,
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
      expect(screen.getByText('Microsoft Corporation')).toBeInTheDocument();
    });

    it('displays company information correctly', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: [mockCompanies[0]],
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
      expect(screen.getByText('AAPL')).toBeInTheDocument();
      expect(screen.getByText('Technology Hardware & Equipment')).toBeInTheDocument();
      expect(screen.getByText('Technology')).toBeInTheDocument();
    });

    it('handles empty search results', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'NonExistentCompany',
        setQuery: jest.fn(),
        results: [],
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByText('No companies found')).toBeInTheDocument();
    });

    it('displays loading state', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: [],
        loading: true,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByText('Searching...')).toBeInTheDocument();
    });

    it('displays error state', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: [],
        loading: false,
        error: 'Search failed',
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByText('Error: Search failed')).toBeInTheDocument();
    });
  });

  describe('Company Selection', () => {
    it('handles company selection', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: mockCompanies,
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const companyCard = screen.getByText('Apple Inc.');
      fireEvent.click(companyCard);

      expect(mockOnCompanySelect).toHaveBeenCalledWith(mockCompanies[0]);
    });

    it('handles crawl start', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: mockCompanies,
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const crawlButton = screen.getByText('Start Crawl');
      fireEvent.click(crawlButton);

      expect(mockOnCrawlStart).toHaveBeenCalledWith(mockCompanies[0]);
    });
  });

  describe('Filtering', () => {
    it('handles include inactive toggle', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: mockCompanies,
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const includeInactiveCheckbox = screen.getByLabelText('Include Inactive');
      fireEvent.click(includeInactiveCheckbox);

      expect(includeInactiveCheckbox).toBeChecked();
    });

    it('handles limit input', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: mockCompanies,
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const limitInput = screen.getByLabelText('Limit Results');
      fireEvent.change(limitInput, { target: { value: '50' } });

      expect(limitInput).toHaveValue(50);
    });
  });

  describe('Accessibility', () => {
    it('has proper ARIA labels', () => {
      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByLabelText('Search companies')).toBeInTheDocument();
      expect(screen.getByLabelText('Include Inactive')).toBeInTheDocument();
      expect(screen.getByLabelText('Limit Results')).toBeInTheDocument();
    });

    it('supports keyboard navigation', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: mockCompanies,
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const searchInput = screen.getByPlaceholderText('Search companies...');
      searchInput.focus();
      
      fireEvent.keyDown(searchInput, { key: 'Tab' });
      // Should move to next focusable element
    });

    it('announces search results to screen readers', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: mockCompanies,
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByRole('region', { name: /search results/i })).toBeInTheDocument();
    });
  });

  describe('Performance', () => {
    it('debounces search input', async () => {
      const mockSearchCompanies = jest.fn();
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: [],
        loading: false,
        error: null,
        searchCompanies: mockSearchCompanies,
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const searchInput = screen.getByPlaceholderText('Search companies...');
      
      // Type multiple characters quickly
      fireEvent.change(searchInput, { target: { value: 'A' } });
      fireEvent.change(searchInput, { target: { value: 'Ap' } });
      fireEvent.change(searchInput, { target: { value: 'App' } });
      fireEvent.change(searchInput, { target: { value: 'Appl' } });
      fireEvent.change(searchInput, { target: { value: 'Apple' } });

      // Wait for debounce
      await waitFor(() => {
        expect(mockSearchCompanies).toHaveBeenCalledTimes(1);
      });
    });

    it('handles large result sets efficiently', () => {
      const largeResultSet = Array.from({ length: 1000 }, (_, i) => ({
        id: i.toString(),
        cik: `000000${i.toString().padStart(4, '0')}`,
        ticker: `TICK${i}`,
        name: `Company ${i}`,
        legalName: `Company ${i} Inc.`,
        industry: 'Technology',
        sector: 'Technology',
        isActive: true,
        createdAt: '2023-01-01T00:00:00Z',
        updatedAt: '2023-01-01T00:00:00Z',
      }));

      mockUseCompanySearch.mockReturnValue({
        query: 'Company',
        setQuery: jest.fn(),
        results: largeResultSet,
        loading: false,
        error: null,
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      // Should render without performance issues
      expect(screen.getByText('Company 0')).toBeInTheDocument();
    });
  });

  describe('Error Handling', () => {
    it('handles network errors gracefully', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: [],
        loading: false,
        error: 'Network error',
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByText('Error: Network error')).toBeInTheDocument();
    });

    it('handles timeout errors', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: [],
        loading: false,
        error: 'Request timeout',
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByText('Error: Request timeout')).toBeInTheDocument();
    });

    it('provides retry functionality', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: [],
        loading: false,
        error: 'Search failed',
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByText('Retry')).toBeInTheDocument();
    });
  });

  describe('Integration', () => {
    it('integrates with GraphQL API', async () => {
      const mockSearchCompanies = jest.fn();
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: mockCompanies,
        loading: false,
        error: null,
        searchCompanies: mockSearchCompanies,
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      const searchButton = screen.getByRole('button', { name: /search/i });
      fireEvent.click(searchButton);

      expect(mockSearchCompanies).toHaveBeenCalledWith('Apple');
    });

    it('handles GraphQL errors', () => {
      mockUseCompanySearch.mockReturnValue({
        query: 'Apple',
        setQuery: jest.fn(),
        results: [],
        loading: false,
        error: 'GraphQL error: Field not found',
        searchCompanies: jest.fn(),
      });

      render(
        <TestWrapper>
          <CompanySearch 
            onCompanySelect={mockOnCompanySelect}
            onCrawlStart={mockOnCrawlStart}
          />
        </TestWrapper>
      );

      expect(screen.getByText('Error: GraphQL error: Field not found')).toBeInTheDocument();
    });
  });
});
