import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { FinancialMobile } from '../FinancialMobile';
import { FinancialStatement, Company, FinancialRatio } from '../../../types/financial';

// Mock the mobile-specific components
jest.mock('../FinancialDashboard', () => ({
  FinancialDashboard: ({ companyId }: any) => (
    <div data-testid="mobile-dashboard">
      Mobile Dashboard for {companyId}
    </div>
  ),
}));

jest.mock('../TrendAnalysisChart', () => ({
  TrendAnalysisChart: ({ ratios }: any) => (
    <div data-testid="mobile-trend-chart">
      Mobile Trend Chart for {ratios?.length || 0} ratios
    </div>
  ),
}));

jest.mock('../PeerComparisonChart', () => ({
  PeerComparisonChart: ({ ratios }: any) => (
    <div data-testid="mobile-peer-chart">
      Mobile Peer Chart for {ratios?.length || 0} ratios
    </div>
  ),
}));

jest.mock('../FinancialAlerts', () => ({
  FinancialAlerts: ({ companyId }: any) => (
    <div data-testid="mobile-alerts">
      Mobile Alerts for {companyId}
    </div>
  ),
}));

const mockFinancialStatements: FinancialStatement[] = [
  {
    id: 'statement-1',
    companyId: 'test-company',
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
];

const mockCompany: Company = {
  id: 'test-company',
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
];

describe('FinancialMobile', () => {
  beforeEach(() => {
    jest.clearAllMocks();

    // Mock mobile viewport
    Object.defineProperty(window, 'innerWidth', {
      writable: true,
      configurable: true,
      value: 375,
    });

    Object.defineProperty(window, 'innerHeight', {
      writable: true,
      configurable: true,
      value: 667,
    });
  });

  it('renders the mobile financial component', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Financial Analysis')).toBeInTheDocument();
    expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
  });

  it('displays mobile navigation tabs', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Overview')).toBeInTheDocument();
    expect(screen.getByText('Trends')).toBeInTheDocument();
    expect(screen.getByText('Comparison')).toBeInTheDocument();
    expect(screen.getByText('Alerts')).toBeInTheDocument();
  });

  it('switches between navigation tabs', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Initially should show overview
    expect(screen.getByTestId('mobile-dashboard')).toBeInTheDocument();

    // Click on trends tab
    const trendsTab = screen.getByText('Trends');
    fireEvent.click(trendsTab);

    // Should show trends chart
    expect(screen.getByTestId('mobile-trend-chart')).toBeInTheDocument();
  });

  it('displays company summary in mobile format', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('AAPL')).toBeInTheDocument();
    expect(screen.getByText('Technology Hardware & Equipment')).toBeInTheDocument();
  });

  it('shows key financial metrics in mobile cards', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Should show key metrics in mobile-friendly cards
    expect(screen.getByText('14.7%')).toBeInTheDocument(); // ROE formatted
  });

  it('handles mobile swipe gestures', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const container = screen.getByTestId('mobile-dashboard');

    // Simulate swipe left
    fireEvent.touchStart(container, {
      touches: [{ clientX: 300, clientY: 200 }],
    });
    fireEvent.touchEnd(container, {
      changedTouches: [{ clientX: 100, clientY: 200 }],
    });

    // Should switch to next tab
    expect(screen.getByTestId('mobile-trend-chart')).toBeInTheDocument();
  });

  it('displays mobile-optimized charts', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Switch to trends tab
    fireEvent.click(screen.getByText('Trends'));

    expect(screen.getByTestId('mobile-trend-chart')).toBeInTheDocument();
  });

  it('shows mobile-friendly alerts', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Switch to alerts tab
    fireEvent.click(screen.getByText('Alerts'));

    expect(screen.getByTestId('mobile-alerts')).toBeInTheDocument();
  });

  it('handles mobile menu toggle', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const menuButton = screen.getByRole('button', { name: /menu/i });
    fireEvent.click(menuButton);

    // Should show mobile menu
    expect(screen.getByText('Settings')).toBeInTheDocument();
    expect(screen.getByText('Export')).toBeInTheDocument();
  });

  it('displays mobile search functionality', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const searchButton = screen.getByRole('button', { name: /search/i });
    fireEvent.click(searchButton);

    // Should show search interface
    expect(screen.getByPlaceholderText('Search ratios...')).toBeInTheDocument();
  });

  it('handles mobile pull-to-refresh', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const container = screen.getByTestId('mobile-dashboard');

    // Simulate pull-to-refresh
    fireEvent.touchStart(container, {
      touches: [{ clientY: 50 }],
    });
    fireEvent.touchMove(container, {
      touches: [{ clientY: 100 }],
    });
    fireEvent.touchEnd(container, {
      changedTouches: [{ clientY: 50 }],
    });

    // Component should handle refresh functionality internally
  });

  it('shows mobile loading states', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Loading...')).toBeInTheDocument();
  });

  it('displays mobile error states', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Error')).toBeInTheDocument();
    expect(screen.getByText('Failed to load data')).toBeInTheDocument();
  });

  it('handles mobile orientation changes', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Simulate orientation change
    Object.defineProperty(window, 'innerWidth', {
      writable: true,
      configurable: true,
      value: 667, // Portrait to landscape
    });

    Object.defineProperty(window, 'innerHeight', {
      writable: true,
      configurable: true,
      value: 375,
    });

    // Trigger resize event
    fireEvent.resize(window);

    // Should adapt to new orientation
    expect(screen.getByText('Financial Analysis')).toBeInTheDocument();
  });

  it('shows mobile-specific navigation indicators', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Should show page indicators
    expect(screen.getByText('1 of 4')).toBeInTheDocument();
  });

  it('handles mobile tab scrolling', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const tabsContainer = screen.getByRole('tablist');

    // Simulate horizontal scroll on tabs
    fireEvent.scroll(tabsContainer, { target: { scrollLeft: 100 } });

    // Should handle scrolling gracefully
    expect(tabsContainer).toBeInTheDocument();
  });

  it('displays mobile-optimized data tables', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Should show mobile-friendly data display
    expect(screen.getByText('Key Metrics')).toBeInTheDocument();
  });

  it('handles mobile keyboard navigation', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Test keyboard navigation
    const overviewTab = screen.getByText('Overview');
    overviewTab.focus();

    fireEvent.keyDown(overviewTab, { key: 'ArrowRight' });

    // Should navigate to next tab
    expect(screen.getByText('Trends')).toHaveFocus();
  });

  it('shows mobile-specific action buttons', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Should show mobile-optimized action buttons
    expect(screen.getByText('Share')).toBeInTheDocument();
    expect(screen.getByText('Bookmark')).toBeInTheDocument();
  });

  it('handles mobile sharing functionality', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const shareButton = screen.getByText('Share');
    fireEvent.click(shareButton);

    // Component should handle sharing functionality internally
  });

  it('displays mobile notification badges', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Should show notification badge
    expect(screen.getByText('3')).toBeInTheDocument();
  });

  it('handles mobile deep linking', () => {
    // Set URL hash for deep linking
    window.location.hash = '#trends';

    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Should start on trends tab
    expect(screen.getByTestId('mobile-trend-chart')).toBeInTheDocument();

    // Clean up
    window.location.hash = '';
  });

  it('shows mobile accessibility features', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Should have proper ARIA labels
    expect(screen.getByRole('tablist')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Overview' })).toBeInTheDocument();
  });

  it('handles mobile offline state', () => {
    render(
      <FinancialMobile
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Offline')).toBeInTheDocument();
    expect(screen.getByText('Limited functionality available')).toBeInTheDocument();
  });
});
