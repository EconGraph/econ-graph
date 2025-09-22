/**
 * Integration tests for XBRL financial statement features
 *
 * These tests verify the complete flow from XBRL data through financial analysis
 * and ensure proper integration between frontend components and backend APIs.
 */

import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { QueryClient, QueryClientProvider } from 'react-query';
import { BrowserRouter } from 'react-router-dom';

// Import the components we're testing
import { FinancialDashboard } from '../../../components/financial/FinancialDashboard';
import { FinancialStatementViewer } from '../../../components/financial/FinancialStatementViewer';
import { BenchmarkComparison } from '../../../components/financial/BenchmarkComparison';
import { TrendAnalysisChart } from '../../../components/financial/TrendAnalysisChart';

// Mock the API calls
const mockFetch = jest.fn();
global.fetch = mockFetch;

// Mock financial data
const mockCompany = {
  id: 'test-company-1',
  cik: '0000320193',
  name: 'Apple Inc.',
  ticker: 'AAPL',
  industry: 'Computer Hardware',
  sector: 'Technology'
};

const mockFinancialStatements = [
  {
    id: 'statement-1',
    companyId: 'test-company-1',
    filingType: '10-K',
    formType: '10-K',
    accessionNumber: '0001234567-23-000001',
    filingDate: '2024-01-15',
    periodEndDate: '2023-12-30',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    documentType: 'balanceSheet',
    documentUrl: 'https://example.com/statement-1',
    xbrlProcessingStatus: 'completed',
    xbrlProcessingCompletedAt: '2024-01-15T10:00:00Z',
    isAmended: false,
    isRestated: false,
    lineItems: [
      {
        id: 'item-1',
        statementId: 'statement-1',
        taxonomyConcept: 'Assets',
        standardLabel: 'Total Assets',
        value: 352755000000,
        unit: 'USD',
        contextRef: 'context-1',
        statementType: 'balanceSheet',
        statementSection: 'assets',
        isCalculated: false,
        createdAt: '2024-01-15T10:00:00Z',
        updatedAt: '2024-01-15T10:00:00Z'
      },
      {
        id: 'item-2',
        statementId: 'statement-1',
        taxonomyConcept: 'AssetsCurrent',
        standardLabel: 'Current Assets',
        value: 143566000000,
        unit: 'USD',
        contextRef: 'context-1',
        statementType: 'balanceSheet',
        statementSection: 'assets',
        isCalculated: false,
        parentConcept: 'Assets',
        createdAt: '2024-01-15T10:00:00Z',
        updatedAt: '2024-01-15T10:00:00Z'
      },
      {
        id: 'item-3',
        statementId: 'statement-1',
        taxonomyConcept: 'Liabilities',
        standardLabel: 'Total Liabilities',
        value: 258549000000,
        unit: 'USD',
        contextRef: 'context-1',
        statementType: 'balanceSheet',
        statementSection: 'liabilities',
        isCalculated: false,
        createdAt: '2024-01-15T10:00:00Z',
        updatedAt: '2024-01-15T10:00:00Z'
      },
      {
        id: 'item-4',
        statementId: 'statement-1',
        taxonomyConcept: 'StockholdersEquity',
        standardLabel: "Stockholders' Equity",
        value: 94206000000,
        unit: 'USD',
        contextRef: 'context-1',
        statementType: 'balanceSheet',
        statementSection: 'equity',
        isCalculated: false,
        createdAt: '2024-01-15T10:00:00Z',
        updatedAt: '2024-01-15T10:00:00Z'
      }
    ],
    createdAt: '2024-01-15T10:00:00Z',
    updatedAt: '2024-01-15T10:00:00Z'
  },
  {
    id: 'statement-2',
    companyId: 'test-company-1',
    filingType: '10-K',
    formType: '10-K',
    accessionNumber: '0001234567-23-000002',
    filingDate: '2024-01-15',
    periodEndDate: '2023-12-30',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    documentType: 'incomeStatement',
    documentUrl: 'https://example.com/statement-2',
    xbrlProcessingStatus: 'completed',
    xbrlProcessingCompletedAt: '2024-01-15T10:00:00Z',
    isAmended: false,
    isRestated: false,
    lineItems: [
      {
        id: 'item-5',
        statementId: 'statement-2',
        taxonomyConcept: 'Revenues',
        standardLabel: 'Net Sales',
        value: 383285000000,
        unit: 'USD',
        contextRef: 'context-2',
        statementType: 'incomeStatement',
        statementSection: 'revenue',
        isCalculated: false,
        createdAt: '2024-01-15T10:00:00Z',
        updatedAt: '2024-01-15T10:00:00Z'
      },
      {
        id: 'item-6',
        statementId: 'statement-2',
        taxonomyConcept: 'NetIncomeLoss',
        standardLabel: 'Net Income',
        value: 96995000000,
        unit: 'USD',
        contextRef: 'context-2',
        statementType: 'incomeStatement',
        statementSection: 'income',
        isCalculated: false,
        createdAt: '2024-01-15T10:00:00Z',
        updatedAt: '2024-01-15T10:00:00Z'
      }
    ],
    createdAt: '2024-01-15T10:00:00Z',
    updatedAt: '2024-01-15T10:00:00Z'
  }
];

const mockFinancialRatios = [
  {
    id: 'ratio-1',
    statementId: 'statement-1',
    ratioName: 'returnOnEquity',
    ratioDisplayName: 'Return on Equity',
    value: 0.147,
    category: 'profitability',
    formula: 'Net Income / Average Stockholders Equity',
    interpretation: 'Strong profitability, above industry average',
    benchmarkPercentile: 75,
    periodEndDate: '2023-12-30',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 95
  },
  {
    id: 'ratio-2',
    statementId: 'statement-1',
    ratioName: 'netProfitMargin',
    ratioDisplayName: 'Net Profit Margin',
    value: 0.253,
    category: 'profitability',
    formula: 'Net Income / Revenue',
    interpretation: 'Excellent profit margins, well above industry average',
    benchmarkPercentile: 95,
    periodEndDate: '2023-12-30',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 98
  }
];

const mockBenchmarkData = {
  ratioName: 'returnOnEquity',
  ratioDisplayName: 'Return on Equity',
  companyValue: 0.147,
  industryP10: 0.08,
  industryP25: 0.10,
  industryMedian: 0.12,
  industryP75: 0.15,
  industryP90: 0.18,
  percentile: 75.0,
  performance: 'Above Average',
  lastUpdated: '2024-01-15T10:30:00Z'
};

// Helper function to create a test wrapper with providers
const createTestWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });

  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        {children}
      </BrowserRouter>
    </QueryClientProvider>
  );
};

describe('XBRL Financial Integration Tests', () => {
  beforeEach(() => {
    jest.clearAllMocks();

    // Mock successful API responses
    mockFetch.mockImplementation((url: string) => {
      if (url.includes('/api/companies/')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve(mockCompany)
        });
      }
      if (url.includes('/api/financial-statements')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve(mockFinancialStatements)
        });
      }
      if (url.includes('/api/financial-ratios')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve(mockFinancialRatios)
        });
      }
      if (url.includes('/api/benchmark-data')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve(mockBenchmarkData)
        });
      }
      return Promise.resolve({
        ok: false,
        status: 404
      });
    });
  });

  describe('Financial Dashboard Integration', () => {
    it('should load and display company financial data from XBRL sources', async () => {
      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <FinancialDashboard
              companyId="test-company-1"
              userType="intermediate"
              showEducationalContent={true}
            />
          </TestWrapper>
        );

      // Wait for data to load
      await waitFor(() => {
        expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
      });

      // Verify company information is displayed
      expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
      expect(screen.getByText('AAPL')).toBeInTheDocument();
      expect(screen.getByText('Technology Hardware & Equipment')).toBeInTheDocument();
    });

    it('should display financial ratios calculated from XBRL data', async () => {
      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <FinancialDashboard
              companyId="test-company-1"
              userType="intermediate"
              showEducationalContent={false}
            />
          </TestWrapper>
        );

      // Wait for ratios to load
      await waitFor(() => {
        expect(screen.getByText('Return on Equity')).toBeInTheDocument();
      });

      // Verify key ratios are displayed
      expect(screen.getByText('Return on Equity')).toBeInTheDocument();
      expect(screen.getByText('Net Profit Margin')).toBeInTheDocument();
      expect(screen.getByLabelText('Return on Equity value')).toBeInTheDocument();
      expect(screen.getByLabelText('Net Profit Margin value')).toBeInTheDocument();
    });

    it('should show benchmark comparisons with industry data', async () => {
      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <FinancialDashboard
              companyId="test-company-1"
              userType="advanced"
              showEducationalContent={false}
            />
          </TestWrapper>
        );

      // Wait for benchmark data to load
      await waitFor(() => {
        expect(screen.getByText(/Industry Benchmark/)).toBeInTheDocument();
      });

      // Verify benchmark information is displayed
      expect(screen.getByText(/Industry Benchmark/)).toBeInTheDocument();
      expect(screen.getByText('75.0%')).toBeInTheDocument();
      expect(screen.getByText('Above Average')).toBeInTheDocument();
    });
  });

  describe('Financial Statement Viewer Integration', () => {
    it('should display balance sheet data from XBRL parsing', async () => {
      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <FinancialStatementViewer
              companyId="test-company-1"
                    statementId="statement-1"
              showEducationalContent={false}
            />
          </TestWrapper>
        );

      // Wait for statement data to load
      await waitFor(() => {
        expect(screen.getByText('Total Assets')).toBeInTheDocument();
      });

      // Verify balance sheet line items are displayed
      expect(screen.getByText('Total Assets')).toBeInTheDocument();
      expect(screen.getByText('Current Assets')).toBeInTheDocument();
      expect(screen.getByText('Total Liabilities')).toBeInTheDocument();
      expect(screen.getByText("Stockholders' Equity")).toBeInTheDocument();

      // Verify values are formatted correctly
      expect(screen.getByText('$352.76B')).toBeInTheDocument();
      expect(screen.getByText('$143.57B')).toBeInTheDocument();
      expect(screen.getByText('$258.55B')).toBeInTheDocument();
      expect(screen.getByText('$94.21B')).toBeInTheDocument();
    });

    it('should display income statement data from XBRL parsing', async () => {
      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <FinancialStatementViewer
              companyId="test-company-1"
                    statementId="statement-2"
              showEducationalContent={false}
            />
          </TestWrapper>
        );

      // Wait for statement data to load
      await waitFor(() => {
        expect(screen.getByText('Net Sales')).toBeInTheDocument();
      });

      // Verify income statement line items are displayed
      expect(screen.getByText('Net Sales')).toBeInTheDocument();
      expect(screen.getByText('Net Income')).toBeInTheDocument();

      // Verify values are formatted correctly
      expect(screen.getByText('$383.29B')).toBeInTheDocument();
      expect(screen.getByText('$96.99B')).toBeInTheDocument();
    });

    it('should handle hierarchical line item display', async () => {
      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <FinancialStatementViewer
              companyId="test-company-1"
                    statementId="statement-1"
              showEducationalContent={false}
            />
          </TestWrapper>
        );

      // Wait for statement data to load
      await waitFor(() => {
        expect(screen.getByText('Total Assets')).toBeInTheDocument();
      });

      // Verify hierarchical structure is displayed
      const totalAssetsRow = screen.getByText('Total Assets').closest('tr');
      const currentAssetsRow = screen.getByText('Current Assets').closest('tr');

      expect(totalAssetsRow).toBeInTheDocument();
      expect(currentAssetsRow).toBeInTheDocument();

      // Current Assets should be indented under Total Assets
      const currentAssetsCell = currentAssetsRow?.querySelector('td:first-child');
      expect(currentAssetsCell).toHaveClass('pl-4'); // Indentation class
    });
  });

  describe('Benchmark Comparison Integration', () => {
    it('should display industry benchmark data for XBRL-derived ratios', async () => {
      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <BenchmarkComparison
              ratioName="returnOnEquity"
              companyValue={0.147}
              benchmarkData={mockBenchmarkData}
            />
          </TestWrapper>
        );

      // Verify benchmark data is displayed
      expect(screen.getByText('Industry Benchmark: returnOnEquity')).toBeInTheDocument();
      expect(screen.getByText('Company Value:')).toBeInTheDocument();
      expect(screen.getAllByText('0.15').length).toBeGreaterThan(0);
      expect(screen.getByText('Industry Percentile:')).toBeInTheDocument();
      expect(screen.getByLabelText('Company ranks at 75.0 percentile, Top 25%')).toBeInTheDocument();
      expect(screen.getByText('Above Average')).toBeInTheDocument();
    });

    it('should show industry distribution percentiles', async () => {
      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <BenchmarkComparison
              ratioName="returnOnEquity"
              companyValue={0.147}
              benchmarkData={mockBenchmarkData}
            />
          </TestWrapper>
        );

      // Verify industry distribution is displayed
      expect(screen.getByText('Industry Distribution')).toBeInTheDocument();
      expect(screen.getByText('P10:')).toBeInTheDocument();
      expect(screen.getByText('0.08')).toBeInTheDocument();
      expect(screen.getByText('P25:')).toBeInTheDocument();
      expect(screen.getByText('0.10')).toBeInTheDocument();
      expect(screen.getByText('Median:')).toBeInTheDocument();
      expect(screen.getByText('0.12')).toBeInTheDocument();
      expect(screen.getByText('P75:')).toBeInTheDocument();
      expect(screen.getByLabelText('75th percentile value: 0.15')).toBeInTheDocument();
      expect(screen.getByText('P90:')).toBeInTheDocument();
      expect(screen.getByText('0.18')).toBeInTheDocument();
    });
  });

  describe('Trend Analysis Integration', () => {

    it('should display trend analysis for XBRL-derived financial ratios', async () => {
      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
                  <TrendAnalysisChart
                    ratios={mockFinancialRatios}
                    statements={mockFinancialStatements}
                    timeRange="3Y"
                    onTimeRangeChange={() => {}}
                    selectedRatios={['returnOnEquity']}
                    onRatioSelectionChange={() => {}}
                  />
          </TestWrapper>
        );

      // Verify trend data is displayed
      expect(screen.getByText('Return on Equity Trend Analysis')).toBeInTheDocument();
      expect(screen.getByText('2021')).toBeInTheDocument();
      expect(screen.getByText('2022')).toBeInTheDocument();
      expect(screen.getByText('2023')).toBeInTheDocument();
    });

    it('should show trend direction and strength', async () => {
      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
                  <TrendAnalysisChart
                    ratios={mockFinancialRatios}
                    statements={mockFinancialStatements}
                    timeRange="3Y"
                    onTimeRangeChange={() => {}}
                    selectedRatios={['returnOnEquity']}
                    onRatioSelectionChange={() => {}}
                  />
          </TestWrapper>
        );

      // Verify trend indicators are displayed (using getAllByText for multiple instances)
      expect(screen.getAllByText(/Trend:/i).length).toBeGreaterThan(0);
      expect(screen.getByText(/Improving/)).toBeInTheDocument();
      expect(screen.getAllByText(/Strength:/i).length).toBeGreaterThan(0);
      expect(screen.getAllByText(/80%/i).length).toBeGreaterThan(0);
    });
  });

  describe('XBRL Data Processing Integration', () => {
    it('should handle XBRL instance document processing status', async () => {
      // Mock XBRL processing status
      mockFetch.mockImplementation((url: string) => {
        if (url.includes('/api/xbrl-instances/')) {
          return Promise.resolve({
            ok: true,
            json: () => Promise.resolve({
              id: 'instance-1',
              companyId: 'test-company-1',
              accessionNumber: '0000320193-24-000006',
              processingStatus: 'processed',
              factsCount: 1250,
              processedAt: '2024-01-15T10:30:00Z'
            })
          });
        }
        return Promise.resolve({ ok: false, status: 404 });
      });

      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <FinancialDashboard
              companyId="test-company-1"
              userType="intermediate"
              showEducationalContent={false}
            />
          </TestWrapper>
        );

      // Wait for processing status to load
      await waitFor(() => {
        expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
      });

      // Verify that processed data is displayed
      expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
    });

    it('should handle XBRL taxonomy schema references', async () => {
      // Mock taxonomy schema data
      mockFetch.mockImplementation((url: string) => {
        if (url.includes('/api/xbrl-taxonomies/')) {
          return Promise.resolve({
            ok: true,
            json: () => Promise.resolve([
              {
                id: 'taxonomy-1',
                namespace: 'http://fasb.org/us-gaap/2023',
                prefix: 'us-gaap',
                version: '2023',
                fileType: 'schema',
                sourceType: 'standard'
              }
            ])
          });
        }
        return Promise.resolve({ ok: false, status: 404 });
      });

      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <FinancialStatementViewer
              companyId="test-company-1"
                    statementId="statement-1"
              showEducationalContent={true}
            />
          </TestWrapper>
        );

      // Wait for taxonomy data to load
      await waitFor(() => {
        expect(screen.getByText('Total Assets')).toBeInTheDocument();
      });

      // Verify that data is displayed using correct taxonomy concepts
      expect(screen.getByText('Total Assets')).toBeInTheDocument();
      expect(screen.getByText('Current Assets')).toBeInTheDocument();
    });
  });

  describe('Error Handling Integration', () => {
    it('should handle XBRL parsing errors gracefully', async () => {
      // Mock XBRL parsing error
      mockFetch.mockImplementation((url: string) => {
        if (url.includes('/api/financial-statements')) {
          return Promise.resolve({
            ok: false,
            status: 500,
            json: () => Promise.resolve({
              error: 'XBRL parsing failed',
              details: 'Invalid XML structure'
            })
          });
        }
        return Promise.resolve({ ok: false, status: 404 });
      });

      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <FinancialStatementViewer
              companyId="test-company-1"
                    statementId="statement-1"
              showEducationalContent={false}
            />
          </TestWrapper>
        );

      // Wait for error to be handled
      await waitFor(() => {
        expect(screen.getByText(/Error loading financial data/)).toBeInTheDocument();
      });

      // Verify error message is displayed
      expect(screen.getByText(/Error loading financial data/)).toBeInTheDocument();
    });

    it('should handle missing XBRL taxonomy schemas', async () => {
      // Mock missing taxonomy error
      mockFetch.mockImplementation((url: string) => {
        if (url.includes('/api/xbrl-taxonomies/')) {
          return Promise.resolve({
            ok: false,
            status: 404,
            json: () => Promise.resolve({
              error: 'Taxonomy schema not found',
              details: 'Required US-GAAP taxonomy not available'
            })
          });
        }
        return Promise.resolve({ ok: false, status: 404 });
      });

      const TestWrapper = createTestWrapper();

      render(
          <TestWrapper>
            <FinancialDashboard
              companyId="test-company-1"
              userType="intermediate"
              showEducationalContent={false}
            />
          </TestWrapper>
        );

      // Wait for error to be handled
      await waitFor(() => {
        expect(screen.getByText(/Taxonomy schema not found/)).toBeInTheDocument();
      });

      // Verify error message is displayed
      expect(screen.getByText(/Taxonomy schema not found/)).toBeInTheDocument();
    });
  });

  describe('Performance Integration', () => {
    it('should handle large XBRL datasets efficiently', async () => {
      // Mock large dataset
      const largeDataset = Array.from({ length: 1000 }, (_, i) => ({
        id: `item-${i}`,
        statementId: 'statement-1',
        conceptName: `Concept${i}`,
        displayName: `Concept ${i}`,
        value: Math.random() * 1000000,
        unit: 'USD',
        decimals: -6,
        isInstant: true,
        isDuration: false,
        parentConcept: i > 0 ? `Concept${i-1}` : null,
        level: Math.floor(i / 100)
      }));

      mockFetch.mockImplementation((url: string) => {
        if (url.includes('/api/financial-statements')) {
          return Promise.resolve({
            ok: true,
            json: () => Promise.resolve([{
              ...mockFinancialStatements[0],
              lineItems: largeDataset
            }])
          });
        }
        return Promise.resolve({ ok: false, status: 404 });
      });

      const TestWrapper = createTestWrapper();
      const startTime = performance.now();

      render(
          <TestWrapper>
            <FinancialStatementViewer
              companyId="test-company-1"
                    statementId="statement-1"
              showEducationalContent={false}
            />
          </TestWrapper>
        );

      // Wait for large dataset to load
      await waitFor(() => {
        expect(screen.getByText('Total Assets')).toBeInTheDocument();
      });

      const endTime = performance.now();
      const loadTime = endTime - startTime;

      // Verify performance is acceptable (should load within 2 seconds)
      expect(loadTime).toBeLessThan(2000);

      // Verify data is displayed
      expect(screen.getByText('Total Assets')).toBeInTheDocument();
    });
  });
});
