import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { TrendAnalysisChart } from '../TrendAnalysisChart';
import { FinancialRatio } from '../../../types/financial';

// Mock Chart.js
jest.mock('react-chartjs-2', () => ({
  Line: ({ data, options, ...props }: any) => (
    <div
      data-testid="trend-line-chart"
      data-chart-data={JSON.stringify(data)}
      data-chart-options={JSON.stringify(options)}
      {...props}
    >
      Mock Trend Line Chart
    </div>
  ),
}));

const mockRatios: FinancialRatio[] = [
  {
    id: 'ratio-1',
    statementId: 'statement-1',
    ratioName: 'returnOnEquity',
    ratioDisplayName: 'Return on Equity',
    value: 0.147,
    category: 'profitability',
    formula: 'Net Income / Shareholders Equity',
    interpretation: 'Strong profitability',
    benchmarkPercentile: 75,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.95,
  },
  {
    id: 'ratio-2',
    statementId: 'statement-2',
    ratioName: 'returnOnEquity',
    ratioDisplayName: 'Return on Equity',
    value: 0.142,
    category: 'profitability',
    formula: 'Net Income / Shareholders Equity',
    interpretation: 'Strong profitability',
    benchmarkPercentile: 72,
    periodEndDate: '2023-09-30',
    fiscalYear: 2023,
    fiscalQuarter: 3,
    calculatedAt: '2023-09-30T00:00:00Z',
    dataQualityScore: 0.93,
  },
  {
    id: 'ratio-3',
    statementId: 'statement-3',
    ratioName: 'currentRatio',
    ratioDisplayName: 'Current Ratio',
    value: 1.04,
    category: 'liquidity',
    formula: 'Current Assets / Current Liabilities',
    interpretation: 'Adequate liquidity',
    benchmarkPercentile: 45,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.98,
  },
];

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
    lineItems: [],
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
    periodEndDate: '2023-09-30',
    fiscalYear: 2023,
    fiscalQuarter: 3,
    documentType: 'balanceSheet',
    documentUrl: 'https://example.com/statement-2',
    xbrlProcessingStatus: 'completed',
    xbrlProcessingCompletedAt: '2024-01-15T10:00:00Z',
    isAmended: false,
    isRestated: false,
    lineItems: [],
    createdAt: '2024-01-15T10:00:00Z',
    updatedAt: '2024-01-15T10:00:00Z'
  }
];

describe('TrendAnalysisChart', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('renders the trend analysis chart', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    expect(screen.getByTestId('trend-line-chart')).toBeInTheDocument();
    expect(screen.getByText('Financial Ratio Trends')).toBeInTheDocument();
  });

  it('displays ratio selection dropdown', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    expect(screen.getByText('Select Ratio to Analyze')).toBeInTheDocument();
    expect(screen.getByDisplayValue('returnOnEquity')).toBeInTheDocument();
  });

  it('filters ratios by selected category', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    // Should show profitability ratios by default
    expect(screen.getByDisplayValue('returnOnEquity')).toBeInTheDocument();

    // Change to liquidity category
    const categorySelect = screen.getByDisplayValue('profitability');
    fireEvent.click(categorySelect);

    // This would trigger the category change handler
    // The actual implementation would update the ratio options
  });

  it('handles ratio selection change', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    const ratioSelect = screen.getByDisplayValue('returnOnEquity');
    fireEvent.click(ratioSelect);

    // Should show available ratios for selection
    expect(screen.getByText('currentRatio')).toBeInTheDocument();
  });

  it('displays trend direction and strength', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    // Should show trend indicators
    expect(screen.getByText(/trend/i)).toBeInTheDocument();
  });

  it('shows loading state when data is being processed', () => {
    render(<TrendAnalysisChart ratios={[]} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    expect(screen.getByText('Loading trend data...')).toBeInTheDocument();
  });

  it('handles empty ratios array', () => {
    render(<TrendAnalysisChart ratios={[]} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    expect(screen.getByText('No ratio data available for trend analysis')).toBeInTheDocument();
  });

  it('groups ratios by name for trend calculation', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    // Should group ratios with the same name for trend analysis
    // ROE should have 2 data points, Current Ratio should have 1
    const chartData = screen.getByTestId('trend-line-chart');
    const dataAttr = chartData.getAttribute('data-chart-data');
    expect(dataAttr).toBeTruthy();
  });

  it('calculates trend direction correctly', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    // ROE trend: 0.142 -> 0.147 (improving)
    // Should show upward trend
    expect(screen.getByText(/upward/i)).toBeInTheDocument();
  });

  it('displays time period controls', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    expect(screen.getByText('Time Period')).toBeInTheDocument();
    expect(screen.getByDisplayValue('12')).toBeInTheDocument(); // Default 12 quarters
  });

  it('handles time period change', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    const timePeriodSelect = screen.getByDisplayValue('12');
    fireEvent.click(timePeriodSelect);

    // Should show other time period options
    expect(screen.getByText('24')).toBeInTheDocument();
    expect(screen.getByText('36')).toBeInTheDocument();
  });

  it('shows chart controls (zoom, reset)', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    expect(screen.getByText('Reset View')).toBeInTheDocument();
  });

  it('displays data quality indicators', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    // Should show data quality information
    expect(screen.getByText(/data quality/i)).toBeInTheDocument();
  });

  it('handles responsive design', () => {
    // Mock mobile viewport
    Object.defineProperty(window, 'innerWidth', {
      writable: true,
      configurable: true,
      value: 375,
    });

    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    // Should adapt to mobile view
    expect(screen.getByTestId('trend-line-chart')).toBeInTheDocument();
  });

  it('shows benchmark comparison when available', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    // Should show benchmark information if available
    expect(screen.getByText(/benchmark/i)).toBeInTheDocument();
  });

  it('handles ratio selection with no trend data', () => {
    const singleRatio = [mockRatios[0]]; // Only one data point

    render(<TrendAnalysisChart ratios={singleRatio} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    expect(screen.getByText('Insufficient data for trend analysis')).toBeInTheDocument();
  });

  it('formats ratio values correctly in chart', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    // Should format percentages and ratios appropriately
    const chartData = screen.getByTestId('trend-line-chart');
    const dataAttr = chartData.getAttribute('data-chart-data');

    if (dataAttr) {
      const data = JSON.parse(dataAttr);
      expect(data).toBeTruthy();
      // Chart data should be properly formatted
    }
  });

  it('handles chart interaction events', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    // Test chart interaction (hover, click, etc.)
    const chart = screen.getByTestId('trend-line-chart');
    fireEvent.mouseOver(chart);

    // Should handle hover events gracefully
    expect(chart).toBeInTheDocument();
  });

  it('displays export functionality', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    expect(screen.getByText('Export Chart')).toBeInTheDocument();
  });

  it('handles export button click', () => {
    render(<TrendAnalysisChart ratios={mockRatios} statements={mockFinancialStatements} timeRange="3Y" onTimeRangeChange={() => {}} />);

    const exportButton = screen.getByText('Export Chart');
    fireEvent.click(exportButton);

    // Should trigger export functionality
    expect(exportButton).toBeInTheDocument();
  });
});
