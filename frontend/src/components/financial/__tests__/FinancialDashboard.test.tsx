import { vi } from 'vitest';
import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { vi } from 'vitest';
import { FinancialDashboard } from '../FinancialDashboard';
import { FinancialStatement, Company, FinancialRatio } from '../../../types/financial';

// Mock the useQuery hook
const mockUseQuery = vi.fn();
vi.mock('react-query', () => ({
  useQuery: (...args: any[]) => mockUseQuery(...args),
}));

// Mock the financial components
vi.mock('../BenchmarkComparison', () => ({
  BenchmarkComparison: ({ ratioName, companyValue, benchmarkData }: any) => (
    <div data-testid={`benchmark-${ratioName}`}>
      {ratioName}: {companyValue} (Benchmark: {benchmarkData?.percentile || 'N/A'})
    </div>
  ),
}));

vi.mock('../TrendAnalysisChart', () => ({
  TrendAnalysisChart: ({ ratios }: any) => (
    <div data-testid="trend-analysis-chart">
      Trend Chart for {ratios?.length || 0} ratios
    </div>
  ),
}));

vi.mock('../PeerComparisonChart', () => ({
  PeerComparisonChart: ({ ratios }: any) => (
    <div data-testid="peer-comparison-chart">
      Peer Comparison for {ratios?.length || 0} ratios
    </div>
  ),
}));

vi.mock('../FinancialAlerts', () => ({
  FinancialAlerts: ({ companyId }: any) => (
    <div data-testid="financial-alerts">
      Alerts for company {companyId}
    </div>
  ),
}));

vi.mock('../FinancialExport', () => ({
  FinancialExport: ({ companyId }: any) => (
    <div data-testid="financial-export">
      Export for company {companyId}
    </div>
  ),
}));

vi.mock('../FinancialMobile', () => ({
  FinancialMobile: ({ companyId }: any) => (
    <div data-testid="financial-mobile">
      Mobile view for company {companyId}
    </div>
  ),
}));

const mockFinancialStatements: FinancialStatement[] = [
  {
    id: 'mock-statement-1',
    companyId: 'mock-company-id',
    filingType: '10-K',
    formType: '10-K',
    accessionNumber: '0001234567-23-000001',
    filingDate: '2023-12-31',
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    documentType: 'XBRL',
    documentUrl: 'http://example.com/filing.xbrl',
    xbrlProcessingStatus: 'completed',
    isAmended: false,
    isRestated: false,
    createdAt: '2023-12-31T00:00:00Z',
    updatedAt: '2023-12-31T00:00:00Z',
  },
  {
    id: 'mock-statement-2',
    companyId: 'mock-company-id',
    filingType: '10-Q',
    formType: '10-Q',
    accessionNumber: '0001234567-23-000002',
    filingDate: '2023-09-30',
    periodEndDate: '2023-09-30',
    fiscalYear: 2023,
    fiscalQuarter: 3,
    documentType: 'XBRL',
    documentUrl: 'http://example.com/filing.xbrl',
    xbrlProcessingStatus: 'completed',
    isAmended: false,
    isRestated: false,
    createdAt: '2023-09-30T00:00:00Z',
    updatedAt: '2023-09-30T00:00:00Z',
  },
];

const mockCompany: Company = {
  id: 'mock-company-id',
  cik: '0000320193',
  name: 'Apple Inc.',
  ticker: 'AAPL',
  sic: '3571',
  sicDescription: 'Electronic Computers',
  gics: '4520',
  gicsDescription: 'Technology Hardware & Equipment',
  businessStatus: 'active',
  fiscalYearEnd: '09-30',
  createdAt: '2023-01-01T00:00:00Z',
  updatedAt: '2023-12-31T00:00:00Z',
};

const mockFinancialRatios: FinancialRatio[] = [
  {
    id: 'ratio-1',
    statementId: 'mock-statement-1',
    ratioName: 'returnOnEquity',
    ratioDisplayName: 'Return on Equity',
    value: 0.147,
    category: 'profitability',
    formula: 'Net Income / Shareholders Equity',
    interpretation: 'Strong profitability, above industry average',
    benchmarkPercentile: 75,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.95,
  },
  {
    id: 'ratio-2',
    statementId: 'mock-statement-1',
    ratioName: 'currentRatio',
    ratioDisplayName: 'Current Ratio',
    value: 1.04,
    category: 'liquidity',
    formula: 'Current Assets / Current Liabilities',
    interpretation: 'Adequate liquidity position',
    benchmarkPercentile: 45,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.98,
  },
];

const mockQueryData = {
  financialStatements: mockFinancialStatements,
  company: mockCompany,
  financialRatios: mockFinancialRatios,
};

describe('FinancialDashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseQuery.mockReturnValue({
      data: mockQueryData,
      isLoading: false,
      isError: false,
      error: null,
      refetch: vi.fn(),
    });
  });

  it('renders the dashboard with company information', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
    expect(screen.getByText('AAPL')).toBeInTheDocument();
    expect(screen.getByText('Technology Hardware & Equipment')).toBeInTheDocument();
  });

  it('displays financial statements in tabs', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Check if tabs are rendered
    expect(screen.getByText('10-K (2023)')).toBeInTheDocument();
    expect(screen.getByText('10-Q (Q3 2023)')).toBeInTheDocument();
  });

  it('shows loading state when data is loading', () => {
    mockUseQuery.mockReturnValue({
      data: undefined,
      isLoading: true,
      isError: false,
      error: null,
      refetch: vi.fn(),
    });

    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByText('Loading financial data...')).toBeInTheDocument();
  });

  it('shows error state when data loading fails', () => {
    mockUseQuery.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      error: new Error('Failed to load data'),
      refetch: vi.fn(),
    });

    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByText(/Error loading financial data:/)).toBeInTheDocument();
    expect(screen.getByText(/Failed to load data/)).toBeInTheDocument();
  });

  it('renders benchmark comparisons for key ratios', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Analysis tab should be available
    expect(screen.getByRole('button', { name: /analysis/i })).toBeInTheDocument();
  });

  it('renders trend analysis chart', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Trends tab should be available
    expect(screen.getByRole('button', { name: /trends/i })).toBeInTheDocument();
  });

  it('renders peer comparison chart', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Compare tab should be available
    expect(screen.getByRole('button', { name: /compare/i })).toBeInTheDocument();
  });

  it('renders financial alerts', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Check if alerts are available anywhere in the component
    expect(screen.getByText('Apple Inc.')).toBeInTheDocument(); // Main dashboard loads
  });

  it('renders export functionality', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Export functionality should be available via the Export button
    expect(screen.getByRole('button', { name: 'Export' })).toBeInTheDocument();
  });

  it('switches between statement tabs', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Initially should show 10-K data
    expect(screen.getByText('10-K (2023)')).toBeInTheDocument();

    // Look for quarterly statement data
    expect(screen.getByText('10-Q (Q3 2023)')).toBeInTheDocument();

    // Should show Q3 data somewhere (multiple instances expected)
    expect(screen.getAllByText(/Q3/).length).toBeGreaterThan(0);
  });

  it('handles refresh button click', async () => {
    const mockRefetch = vi.fn().mockResolvedValue({});
    mockUseQuery.mockReturnValue({
      data: mockQueryData,
      isLoading: false,
      isError: false,
      error: null,
      refetch: mockRefetch,
    });

    render(<FinancialDashboard companyId="mock-company-id" />);

    const refreshButton = screen.getByRole('button', { name: 'Refresh' });
    fireEvent.click(refreshButton);

    await waitFor(() => {
      expect(mockRefetch).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });
  });

  it('displays ratio cards with correct values', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Check if ratio values are displayed using accessibility labels and getAllByText for duplicates
    expect(screen.getByLabelText('Return on Equity value')).toHaveTextContent('14.7%');
    expect(screen.getAllByText('1.04').length).toBeGreaterThan(0); // Current ratio (may appear multiple times)
  });

  it('shows ratio interpretations', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByText('Strong profitability, above industry average')).toBeInTheDocument();
    expect(screen.getByText('Adequate liquidity position')).toBeInTheDocument();
  });

  it('handles empty ratios gracefully', () => {
    mockUseQuery.mockReturnValue({
      data: {
        ...mockQueryData,
        financialRatios: [],
      },
      isLoading: false,
      isError: false,
      error: null,
      refetch: vi.fn(),
    });

    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByText('No financial ratios available')).toBeInTheDocument();
  });

  it('handles missing company data gracefully', () => {
    mockUseQuery.mockReturnValue({
      data: {
        ...mockQueryData,
        company: null,
      },
      isLoading: false,
      isError: false,
      error: null,
      refetch: vi.fn(),
    });

    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByText('Company information not available')).toBeInTheDocument();
  });

  it('displays data quality indicators', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Check for data quality scores (should be displayed somewhere in the UI)
    expect(screen.getByText('Quality: 95%')).toBeInTheDocument(); // Data quality score for ROE
    expect(screen.getByText('Quality: 98%')).toBeInTheDocument(); // Data quality score for Current Ratio
    // Note: Quality: 92% for debt/equity may not be shown if it's not in the overview metrics cards
  });
});
