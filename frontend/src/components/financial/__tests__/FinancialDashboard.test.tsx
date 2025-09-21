import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import './test-setup';
import { FinancialDashboard } from '../FinancialDashboard';
import { FinancialStatement, Company, FinancialRatio } from '../../../types/financial';

// Mock the useQuery hook
const mockUseQuery = jest.fn();
jest.mock('react-query', () => ({
  useQuery: (...args: any[]) => mockUseQuery(...args),
}));

// Mock the financial components
jest.mock('../BenchmarkComparison', () => ({
  BenchmarkComparison: ({ ratioName, companyValue, benchmarkData }: any) => (
    <div data-testid={`benchmark-${ratioName}`}>
      {ratioName}: {companyValue} (Benchmark: {benchmarkData?.percentile || 'N/A'})
    </div>
  ),
}));

jest.mock('../TrendAnalysisChart', () => ({
  TrendAnalysisChart: ({ ratios }: any) => (
    <div data-testid="trend-analysis-chart">
      Trend Chart for {ratios?.length || 0} ratios
    </div>
  ),
}));

jest.mock('../PeerComparisonChart', () => ({
  PeerComparisonChart: ({ ratios }: any) => (
    <div data-testid="peer-comparison-chart">
      Peer Comparison for {ratios?.length || 0} ratios
    </div>
  ),
}));

jest.mock('../FinancialAlerts', () => ({
  FinancialAlerts: ({ companyId }: any) => (
    <div data-testid="financial-alerts">
      Alerts for company {companyId}
    </div>
  ),
}));

jest.mock('../FinancialExport', () => ({
  FinancialExport: ({ companyId }: any) => (
    <div data-testid="financial-export">
      Export for company {companyId}
    </div>
  ),
}));

jest.mock('../FinancialMobile', () => ({
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
    jest.clearAllMocks();
    mockUseQuery.mockReturnValue({
      data: mockQueryData,
      loading: false,
      error: null,
      refetch: jest.fn(),
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
      loading: true,
      error: null,
      refetch: jest.fn(),
    });

    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByText('Loading financial data...')).toBeInTheDocument();
  });

  it('shows error state when data loading fails', () => {
    mockUseQuery.mockReturnValue({
      data: undefined,
      loading: false,
      error: new Error('Failed to load data'),
      refetch: jest.fn(),
    });

    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByText('Error loading financial data')).toBeInTheDocument();
    expect(screen.getByText('Failed to load data')).toBeInTheDocument();
  });

  it('renders benchmark comparisons for key ratios', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByTestId('benchmark-returnOnEquity')).toBeInTheDocument();
    expect(screen.getByTestId('benchmark-currentRatio')).toBeInTheDocument();
  });

  it('renders trend analysis chart', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByTestId('trend-analysis-chart')).toBeInTheDocument();
  });

  it('renders peer comparison chart', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByTestId('peer-comparison-chart')).toBeInTheDocument();
  });

  it('renders financial alerts', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByTestId('financial-alerts')).toBeInTheDocument();
  });

  it('renders export functionality', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByTestId('financial-export')).toBeInTheDocument();
  });

  it('switches between statement tabs', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Initially should show 10-K data
    expect(screen.getByText('10-K (2023)')).toBeInTheDocument();

    // Click on 10-Q tab
    const q3Tab = screen.getByText('10-Q (Q3 2023)');
    fireEvent.click(q3Tab);

    // Should now show Q3 data
    expect(screen.getByText('Q3 2023')).toBeInTheDocument();
  });

  it('handles refresh button click', async () => {
    const mockRefetch = jest.fn();
    mockUseQuery.mockReturnValue({
      data: mockQueryData,
      loading: false,
      error: null,
      refetch: mockRefetch,
    });

    render(<FinancialDashboard companyId="mock-company-id" />);

    const refreshButton = screen.getByRole('button', { name: /refresh/i });
    fireEvent.click(refreshButton);

    await waitFor(() => {
      expect(mockRefetch).toHaveBeenCalledTimes(1);
    });
  });

  it('displays ratio cards with correct values', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Check if ratio values are displayed
    expect(screen.getByText('14.7%')).toBeInTheDocument(); // ROE formatted as percentage
    expect(screen.getByText('1.04')).toBeInTheDocument(); // Current ratio
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
      loading: false,
      error: null,
      refetch: jest.fn(),
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
      loading: false,
      error: null,
      refetch: jest.fn(),
    });

    render(<FinancialDashboard companyId="mock-company-id" />);

    expect(screen.getByText('Company information not available')).toBeInTheDocument();
  });

  it('displays data quality indicators', () => {
    render(<FinancialDashboard companyId="mock-company-id" />);

    // Check for data quality scores (should be displayed somewhere in the UI)
    // This would depend on the actual implementation
    expect(screen.getByText('95%')).toBeInTheDocument(); // Data quality score
  });
});
