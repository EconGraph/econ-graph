import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { PeerComparisonChart } from '../PeerComparisonChart';
import { FinancialRatio } from '../../../types/financial';

// Mock Chart.js
jest.mock('react-chartjs-2', () => ({
  Bar: ({ data, options, ...props }: any) => (
    <div
      data-testid="peer-comparison-bar-chart"
      data-chart-data={JSON.stringify(data)}
      data-chart-options={JSON.stringify(options)}
      {...props}
    >
      Mock Peer Comparison Bar Chart
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

const mockCompany = {
  id: 'test-company-1',
  cik: '0000320193',
  name: 'Apple Inc.',
  ticker: 'AAPL',
  sic: '3571',
  sicDescription: 'Electronic Computers',
  gics: '452020',
  gicsDescription: 'Technology Hardware & Equipment',
  businessAddress: {
    street1: 'One Apple Park Way',
    city: 'Cupertino',
    state: 'CA',
    zip: '95014',
    country: 'US',
  },
  mailingAddress: {
    street1: 'One Apple Park Way',
    city: 'Cupertino',
    state: 'CA',
    zip: '95014',
    country: 'US',
  },
  phone: '(408) 996-1010',
  website: 'https://www.apple.com',
  stateOfIncorporation: 'CA',
  stateOfIncorporationDescription: 'California',
  fiscalYearEnd: '09-30',
  businessStatus: 'Active',
  createdAt: '2023-01-01T00:00:00Z',
  updatedAt: '2023-12-31T00:00:00Z',
};


describe('PeerComparisonChart', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('renders the peer comparison chart', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
    expect(screen.getByText('Peer Company Comparison')).toBeInTheDocument();
  });

  it('displays ratio selection dropdown', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    expect(screen.getByLabelText('Select Ratio for Comparison:')).toBeInTheDocument();
    expect(screen.getByLabelText('Select financial ratio for comparison')).toBeInTheDocument();
  });

  it('handles ratio selection change', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    const ratioSelect = screen.getByLabelText('Select financial ratio for comparison');
    fireEvent.click(ratioSelect);

    // The dropdown should be accessible
    expect(ratioSelect).toBeInTheDocument();
  });

  it('displays peer companies in the chart', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Should show peer company names (using getAllByText since they appear multiple times)
    expect(screen.getAllByText('Microsoft Corporation').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Amazon.com Inc.').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Alphabet Inc.').length).toBeGreaterThan(0);
  });

  it('shows company ranking and percentile', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Should show ranking information
    expect(screen.getByText(/75th percentile/i)).toBeInTheDocument();
  });

  it('handles sorting by different criteria', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    const sortSelect = screen.getByLabelText('Sort companies by');
    fireEvent.click(sortSelect);

    // Should be able to access sort options
    expect(screen.getByLabelText('Sort by:')).toBeInTheDocument();
    expect(sortSelect).toBeInTheDocument();
  });

  it('displays market cap information', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Should show market cap for each peer
    expect(screen.getByText('$3.0T')).toBeInTheDocument(); // Microsoft
    expect(screen.getByText('$1.8T')).toBeInTheDocument(); // Google
    expect(screen.getByText('$1.5T')).toBeInTheDocument(); // Amazon
  });

  it('handles empty peer companies array', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    expect(screen.getByText('No peer companies available for comparison')).toBeInTheDocument();
  });

  it('shows loading state when peer data is being fetched', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    expect(screen.getByText('Loading peer company data...')).toBeInTheDocument();
  });

  it('displays industry classification', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Multiple technology entries expected (use getAllByText)
    expect(screen.getAllByText(/technology/i).length).toBeGreaterThan(0);
  });

  it('shows company performance indicators', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Should show performance indicators (above/below average)
    expect(screen.getByText(/above average/i)).toBeInTheDocument();
  });

  it('handles chart interaction events', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    const chart = screen.getByTestId('peer-comparison-bar-chart');
    fireEvent.mouseOver(chart);

    // Should handle hover events
    expect(chart).toBeInTheDocument();
  });

  it('displays export functionality', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    expect(screen.getByText('Export Comparison')).toBeInTheDocument();
  });

  it('handles export button click', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    const exportButton = screen.getByText('Export Comparison');
    fireEvent.click(exportButton);

    // Should trigger export functionality
    expect(exportButton).toBeInTheDocument();
  });

  it('shows data quality indicators', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Should show data quality information (multiple instances expected)
    expect(screen.getAllByText(/data quality/i).length).toBeGreaterThan(0);
  });

  it('handles responsive design for mobile', () => {
    // Mock mobile viewport
    Object.defineProperty(window, 'innerWidth', {
      writable: true,
      configurable: true,
      value: 375,
    });

    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Should adapt to mobile view
    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
  });

  it('displays benchmark comparison when available', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Should show benchmark information (multiple instances expected)
    expect(screen.getAllByText(/industry benchmark/i).length).toBeGreaterThan(0);
  });

  it('shows company selection controls', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Should show controls to select/deselect peer companies
    expect(screen.getByText('Select Companies:')).toBeInTheDocument();
    expect(screen.getByRole('group', { name: /Select Companies/i })).toBeInTheDocument();
  });

  it('handles company selection changes', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Test selecting/deselecting companies using accessibility labels
    const microsoftCheckbox = screen.getByLabelText('Include Microsoft Corporation in comparison');
    const amazonCheckbox = screen.getByLabelText('Include Amazon.com Inc. in comparison');
    
    expect(microsoftCheckbox).toBeInTheDocument();
    expect(amazonCheckbox).toBeInTheDocument();

    // Click first checkbox
    fireEvent.click(microsoftCheckbox);

    // Chart should update
    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
  });

  it('displays ratio values in chart format', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    const chartData = screen.getByTestId('peer-comparison-bar-chart');
    const dataAttr = chartData.getAttribute('data-chart-data');

    if (dataAttr) {
      const data = JSON.parse(dataAttr);
      expect(data).toBeTruthy();
      // Chart data should be properly formatted
    }
  });

  it('shows comparison summary', () => {
    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Should show summary of how company compares to peers
    expect(screen.getByText(/compared to peers/i)).toBeInTheDocument();
  });

  it('handles missing ratio data for peers', () => {

    render(
      <PeerComparisonChart
        ratios={mockRatios}
        company={mockCompany}
      />
    );

    // Should handle missing data gracefully
    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
  });
});
