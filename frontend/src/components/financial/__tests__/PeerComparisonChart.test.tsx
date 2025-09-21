import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import './test-setup';
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

const mockPeerCompanies = [
  {
    id: 'peer-1',
    name: 'Microsoft Corporation',
    ticker: 'MSFT',
    marketCap: 3000000000000,
    ratios: [
      { ratioName: 'returnOnEquity', value: 0.142, benchmarkPercentile: 70 },
      { ratioName: 'currentRatio', value: 1.15, benchmarkPercentile: 55 },
    ],
  },
  {
    id: 'peer-2',
    name: 'Google (Alphabet)',
    ticker: 'GOOGL',
    marketCap: 1800000000000,
    ratios: [
      { ratioName: 'returnOnEquity', value: 0.158, benchmarkPercentile: 80 },
      { ratioName: 'currentRatio', value: 0.98, benchmarkPercentile: 40 },
    ],
  },
  {
    id: 'peer-3',
    name: 'Amazon.com',
    ticker: 'AMZN',
    marketCap: 1500000000000,
    ratios: [
      { ratioName: 'returnOnEquity', value: 0.135, benchmarkPercentile: 65 },
      { ratioName: 'currentRatio', value: 1.22, benchmarkPercentile: 60 },
    ],
  },
];

describe('PeerComparisonChart', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('renders the peer comparison chart', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
    expect(screen.getByText('Peer Company Comparison')).toBeInTheDocument();
  });

  it('displays ratio selection dropdown', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    expect(screen.getByText('Select Ratio for Comparison')).toBeInTheDocument();
    expect(screen.getByDisplayValue('returnOnEquity')).toBeInTheDocument();
  });

  it('handles ratio selection change', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    const ratioSelect = screen.getByDisplayValue('returnOnEquity');
    fireEvent.click(ratioSelect);
    
    // Should show available ratios for selection
    expect(screen.getByText('currentRatio')).toBeInTheDocument();
  });

  it('displays peer companies in the chart', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    // Should show peer company names
    expect(screen.getByText('Microsoft Corporation')).toBeInTheDocument();
    expect(screen.getByText('Google (Alphabet)')).toBeInTheDocument();
    expect(screen.getByText('Amazon.com')).toBeInTheDocument();
  });

  it('shows company ranking and percentile', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    // Should show ranking information
    expect(screen.getByText(/75th percentile/i)).toBeInTheDocument();
  });

  it('handles sorting by different criteria', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    const sortSelect = screen.getByDisplayValue('performance');
    fireEvent.click(sortSelect);
    
    // Should show sorting options
    expect(screen.getByText('name')).toBeInTheDocument();
    expect(screen.getByText('marketCap')).toBeInTheDocument();
  });

  it('displays market cap information', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
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
        companyId="test-company"
        peerCompanies={[]}
      />
    );
    
    expect(screen.getByText('No peer companies available for comparison')).toBeInTheDocument();
  });

  it('shows loading state when peer data is being fetched', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={undefined}
      />
    );
    
    expect(screen.getByText('Loading peer company data...')).toBeInTheDocument();
  });

  it('displays industry classification', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    expect(screen.getByText(/technology/i)).toBeInTheDocument();
  });

  it('shows company performance indicators', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    // Should show performance indicators (above/below average)
    expect(screen.getByText(/above average/i)).toBeInTheDocument();
  });

  it('handles chart interaction events', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
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
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    expect(screen.getByText('Export Comparison')).toBeInTheDocument();
  });

  it('handles export button click', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
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
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    // Should show data quality information
    expect(screen.getByText(/data quality/i)).toBeInTheDocument();
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
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    // Should adapt to mobile view
    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
  });

  it('displays benchmark comparison when available', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    // Should show benchmark information
    expect(screen.getByText(/industry benchmark/i)).toBeInTheDocument();
  });

  it('shows company selection controls', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    // Should show controls to select/deselect peer companies
    expect(screen.getByText('Select Companies')).toBeInTheDocument();
  });

  it('handles company selection changes', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    // Test selecting/deselecting companies
    const companyCheckboxes = screen.getAllByRole('checkbox');
    expect(companyCheckboxes.length).toBeGreaterThan(0);
    
    // Click first checkbox
    fireEvent.click(companyCheckboxes[0]);
    
    // Chart should update
    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
  });

  it('displays ratio values in chart format', () => {
    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
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
        companyId="test-company"
        peerCompanies={mockPeerCompanies}
      />
    );
    
    // Should show summary of how company compares to peers
    expect(screen.getByText(/compared to peers/i)).toBeInTheDocument();
  });

  it('handles missing ratio data for peers', () => {
    const peersWithMissingData = [
      {
        ...mockPeerCompanies[0],
        ratios: [], // No ratio data
      },
      ...mockPeerCompanies.slice(1),
    ];

    render(
      <PeerComparisonChart 
        ratios={mockRatios} 
        companyId="test-company"
        peerCompanies={peersWithMissingData}
      />
    );
    
    // Should handle missing data gracefully
    expect(screen.getByTestId('peer-comparison-bar-chart')).toBeInTheDocument();
  });
});
