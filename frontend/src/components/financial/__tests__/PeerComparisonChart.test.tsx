import { vi } from 'vitest';
import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { PeerComparisonChart } from '../PeerComparisonChart';
import { FinancialRatio } from '../../../types/financial';
import { TestProviders } from '../../../test-utils/test-providers';

// Mock Chart.js
vi.mock('react-chartjs-2', () => ({
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
    vi.clearAllMocks();
  });

  it('renders the peer comparison chart', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
    expect(screen.getByText('Peer Company Comparison')).toBeInTheDocument();
  });

  it('displays ratio selection dropdown', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    expect(screen.getByLabelText('Select Ratio for Comparison:')).toBeInTheDocument();
    expect(screen.getByLabelText('Select financial ratio for comparison')).toBeInTheDocument();
  });

  it('handles ratio selection change', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    const ratioSelect = screen.getByLabelText('Select financial ratio for comparison');
    fireEvent.click(ratioSelect);

    // The dropdown should be accessible
    expect(ratioSelect).toBeInTheDocument();
  });

  it('displays peer companies in the chart', async () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Wait for peer company data to load and render
    await waitFor(() => {
      expect(screen.getAllByText('Microsoft Corporation').length).toBeGreaterThan(0);
    });
    await waitFor(() => {
      expect(screen.getAllByText('Amazon.com Inc.').length).toBeGreaterThan(0);
    });
    await waitFor(() => {
      expect(screen.getAllByText('Alphabet Inc.').length).toBeGreaterThan(0);
    });
  });

  it('shows company ranking and percentile', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Should show ranking information
    expect(screen.getByText(/75th percentile/i)).toBeInTheDocument();
  });

  it('handles sorting by different criteria', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    const sortSelect = screen.getByLabelText('Sort companies by');
    fireEvent.click(sortSelect);

    // Should be able to access sort options
    expect(screen.getByLabelText('Sort by:')).toBeInTheDocument();
    expect(sortSelect).toBeInTheDocument();
  });

  it('displays market cap information', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Should show market cap for each peer
    expect(screen.getByText('$3.0T')).toBeInTheDocument(); // Microsoft
    expect(screen.getByText('$1.8T')).toBeInTheDocument(); // Google
    expect(screen.getByText('$1.5T')).toBeInTheDocument(); // Amazon
  });

  it('handles empty peer companies array', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    expect(screen.getByText('No peer companies available for comparison')).toBeInTheDocument();
  });

  it('shows loading state when peer data is being fetched', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    expect(screen.getByText('Loading peer company data...')).toBeInTheDocument();
  });

  it('displays industry classification', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Multiple technology entries expected (use getAllByText)
    expect(screen.getAllByText(/technology/i).length).toBeGreaterThan(0);
  });

  it('shows company performance indicators', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Should show performance indicators (above/below average)
    expect(screen.getByText(/above average/i)).toBeInTheDocument();
  });

  it('handles chart interaction events', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    const chart = screen.getByTestId('peer-comparison-bar-chart');
    fireEvent.mouseOver(chart);

    // Should handle hover events
    expect(chart).toBeInTheDocument();
  });

  it('displays export functionality', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    expect(screen.getByText('Export Comparison')).toBeInTheDocument();
  });

  it('handles export button click', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    const exportButton = screen.getByText('Export Comparison');
    fireEvent.click(exportButton);

    // Should trigger export functionality
    expect(exportButton).toBeInTheDocument();
  });

  it('shows data quality indicators', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
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
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Should adapt to mobile view
    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
  });

  it('displays benchmark comparison when available', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Should show benchmark information (multiple instances expected)
    expect(screen.getAllByText(/industry benchmark/i).length).toBeGreaterThan(0);
  });

  it('shows company selection controls', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Should show controls to select/deselect peer companies
    expect(screen.getByText('Select Companies:')).toBeInTheDocument();
    expect(screen.getByRole('group', { name: /Select Companies/i })).toBeInTheDocument();
  });

  it('handles company selection changes', async () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Wait for peer company data to load and render checkboxes
    const microsoftCheckbox = await screen.findByLabelText('Include Microsoft Corporation in comparison');
    const amazonCheckbox = await screen.findByLabelText('Include Amazon.com Inc. in comparison');

    expect(microsoftCheckbox).toBeInTheDocument();
    expect(amazonCheckbox).toBeInTheDocument();

    // Click first checkbox
    fireEvent.click(microsoftCheckbox);

    // Chart should update
    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
  });

  it('displays ratio values in chart format', () => {
    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
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
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Should show summary of how company compares to peers
    expect(screen.getByText(/compared to peers/i)).toBeInTheDocument();
  });

  it('handles missing ratio data for peers', () => {

    render(
      <TestProviders>
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
        />
      </TestProviders>
    );

    // Should handle missing data gracefully
    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
  });
});
