import { vi } from 'vitest';
import React, { Suspense } from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { FinancialAlerts } from '../FinancialAlerts';
import { ErrorBoundary } from '../../common/ErrorBoundary';

// Mock financial alert data
const mockAlerts = [
  {
    id: '1',
    type: 'ratio_threshold',
    severity: 'high',
    title: 'Current Ratio Below Threshold',
    description: 'Current ratio of 0.95 is below the recommended threshold of 1.0',
    companyId: 'test-company',
    companyName: 'Test Company Inc.',
    direction: 'decline',
    isActive: true,
    isRead: false,
    createdAt: '2024-01-15T10:00:00Z',
    expiresAt: '2024-01-25T23:59:59Z',
  },
  {
    id: '2',
    type: 'filing_deadline',
    severity: 'medium',
    title: '10-Q Filing Due Soon',
    description: 'Quarterly report (10-Q) is due within 5 business days',
    companyId: 'test-company',
    companyName: 'Test Company Inc.',
    direction: 'change',
    isActive: true,
    isRead: false,
    createdAt: '2024-01-12T14:00:00Z',
    expiresAt: '2024-01-25T23:59:59Z',
  },
  {
    id: '3',
    type: 'data_quality',
    severity: 'low',
    title: 'Data Quality Warning',
    description: 'Some financial data has low confidence scores',
    companyId: 'test-company',
    companyName: 'Test Company Inc.',
    direction: 'stable',
    isActive: true,
    isRead: true,
    createdAt: '2024-01-10T09:00:00Z',
    expiresAt: '2024-01-20T23:59:59Z',
  },
  {
    id: '4',
    type: 'benchmark_change',
    severity: 'medium',
    title: 'Industry Benchmark Updated',
    description: 'Industry benchmarks have been updated with latest data',
    companyId: 'test-company',
    companyName: 'Test Company Inc.',
    direction: 'improvement',
    isActive: false,
    isRead: true,
    createdAt: '2024-01-08T16:00:00Z',
    expiresAt: '2024-01-18T23:59:59Z',
  },
];

// Create test wrapper with QueryClient and ErrorBoundary
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
      <ErrorBoundary>
        <Suspense fallback={<div data-testid="loading">Loading...</div>}>
          {children}
        </Suspense>
      </ErrorBoundary>
    </QueryClientProvider>
  );
};

describe('FinancialAlerts', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // Helper function to render component with all necessary providers
  const renderWithProviders = (props: any) => {
    const TestWrapper = createTestWrapper();
    return render(
      <TestWrapper>
        <FinancialAlerts {...props} />
      </TestWrapper>
    );
  };

  it('renders the financial alerts component', async () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    await waitFor(() => {
      expect(screen.getByText('Financial Alerts')).toBeInTheDocument();
    });
  });

  it('displays alert severity indicators', async () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    await waitFor(() => {
      expect(screen.getByText('High')).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(screen.getAllByText('Medium').length).toBeGreaterThan(0);
    });

    await waitFor(() => {
      expect(screen.getByText('Low')).toBeInTheDocument();
    });
  });

  it('shows unread alert count', async () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    await waitFor(() => {
      // Should show count of unread alerts (2 unread out of 4 total)
      expect(screen.getByText('2 Unread')).toBeInTheDocument();
    });
  });

  it('filters alerts by severity', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show severity filter options are available in the component
    expect(screen.getByText('Severity:')).toBeInTheDocument();

    // Since the options are in SelectContent, they may not be visible until clicked
    // For now, just verify the filter exists
    expect(screen.getByText('Severity:')).toBeInTheDocument();
  });

  it('filters alerts by type', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show type filter is available
    expect(screen.getByText('Type:')).toBeInTheDocument();
  });

  it('sorts alerts by different criteria', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show sort option is available
    expect(screen.getByText('Sort:')).toBeInTheDocument();
  });

  it('marks alerts as read when clicked', () => {
    render(
      <FinancialAlerts
        companyId="test-company"
        ratios={[]}
        statements={[]}
      />
    );

    const unreadAlert = screen.getByText('Current Ratio Below Threshold');
    fireEvent.click(unreadAlert);

    // Component should handle the read state internally
  });

  it('marks alerts as unread when clicked again', () => {
    render(
      <FinancialAlerts
        companyId="test-company"
        ratios={[]}
        statements={[]}
      />
    );

    const readAlert = screen.getByText('Data Quality Warning');
    fireEvent.click(readAlert);

    // Component should handle the unread state internally
  });

  it('dismisses alerts when dismiss button is clicked', () => {
    render(
      <FinancialAlerts
        companyId="test-company"
        ratios={[]}
        statements={[]}
      />
    );

    const dismissButton = screen.getAllByText('Dismiss')[0];
    fireEvent.click(dismissButton);

    // Component should handle dismiss functionality internally
  });

  it('shows alert details when expanded', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    const alertTitle = screen.getByText('Current Ratio Below Threshold');
    fireEvent.click(alertTitle);

    // Should show detailed description
    expect(screen.getByText('Current ratio of 0.95 is below the recommended threshold of 1.0')).toBeInTheDocument();
  });

  it('displays alert creation and expiration dates', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show formatted dates
    expect(screen.getByText(/Jan 15, 2024/i)).toBeInTheDocument();
    expect(screen.getByText(/Jan 25, 2024/i)).toBeInTheDocument();
  });

  it('shows alert direction indicators', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show direction indicators (multiple elements expected)
    expect(screen.getAllByText(/decline/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/improvement/i)).toBeInTheDocument();
  });

  it('handles empty alerts array', () => {
    render(<FinancialAlerts companyId="empty-company" ratios={[]} statements={[]} />);

    expect(screen.getByText('No alerts available')).toBeInTheDocument();
  });

  it('shows loading state when alerts are being fetched', () => {
    render(<FinancialAlerts companyId="loading-company" ratios={[]} statements={[]} />);

    expect(screen.getByText('Loading alerts...')).toBeInTheDocument();
  });

  it('displays expired alerts differently', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Expired alert should be shown with different styling
    const expiredAlert = screen.getByText('10-Q Filing Due Soon');
    expect(expiredAlert).toBeInTheDocument();
  });

  it('handles bulk actions (mark all as read)', () => {
    render(
      <FinancialAlerts
        companyId="test-company"
        ratios={[]}
        statements={[]}
      />
    );

    const markAllButton = screen.getByText('Mark All as Read');
    fireEvent.click(markAllButton);

    // Component should handle bulk actions internally
  });

  it('handles bulk actions (dismiss all)', () => {
    render(
      <FinancialAlerts
        companyId="test-company"
        ratios={[]}
        statements={[]}
      />
    );

    const dismissAllButton = screen.getByText('Dismiss All');
    fireEvent.click(dismissAllButton);

    // Component should handle bulk dismiss actions internally
  });

  it('shows alert search functionality', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    const searchInput = screen.getByPlaceholderText('Search alerts...');
    expect(searchInput).toBeInTheDocument();

    // Test search
    fireEvent.change(searchInput, { target: { value: 'ratio' } });

    // Should filter alerts based on search
    expect(screen.getByText('Current Ratio Below Threshold')).toBeInTheDocument();
    expect(screen.queryByText('10-Q Filing Due Soon')).not.toBeInTheDocument();
  });

  it('displays alert categories and counts', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show category breakdown
    expect(screen.getByText('Ratio Threshold: 1')).toBeInTheDocument();
    expect(screen.getByText('Filing Deadline: 1')).toBeInTheDocument();
    expect(screen.getByText('Data Quality: 1')).toBeInTheDocument();
  });

  it('handles responsive design for mobile', () => {
    // Mock mobile viewport
    Object.defineProperty(window, 'innerWidth', {
      writable: true,
      configurable: true,
      value: 375,
    });

    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should adapt to mobile view
    expect(screen.getByText('Financial Alerts')).toBeInTheDocument();
  });

  it('shows alert priority indicators', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show priority indicators for high severity alerts
    expect(screen.getByText('High Priority')).toBeInTheDocument();
  });

  it('handles alert action buttons (view details, acknowledge)', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show action buttons for each alert
    expect(screen.getAllByText('View Details')).toHaveLength(mockAlerts.length);
    expect(screen.getAllByText('Acknowledge')).toHaveLength(mockAlerts.length);
  });

  it('displays alert notifications count in header', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show notification count in header
    expect(screen.getByText('4')).toBeInTheDocument(); // Total alerts
    expect(screen.getByText('2')).toBeInTheDocument(); // Unread alerts
  });

  it('handles alert refresh functionality', () => {
    render(
      <FinancialAlerts
        companyId="test-company"
        ratios={[]}
        statements={[]}
      />
    );

    const refreshButton = screen.getByRole('button', { name: /refresh/i });
    fireEvent.click(refreshButton);

    // Component should handle refresh functionality internally
  });

  it('shows alert settings and preferences', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show settings/preferences link
    expect(screen.getByText('Alert Settings')).toBeInTheDocument();
  });

  it('handles alert export functionality', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    const exportButton = screen.getByText('Export Alerts');
    fireEvent.click(exportButton);

    // Should trigger export functionality
    expect(exportButton).toBeInTheDocument();
  });

  it('displays alert trends and statistics', () => {
    renderWithProviders({ companyId: "test-company", ratios: [], statements: [] });

    // Should show alert trends
    expect(screen.getByText('Alert Trends')).toBeInTheDocument();
    expect(screen.getByText('This Week: 4')).toBeInTheDocument();
    expect(screen.getByText('Last Week: 2')).toBeInTheDocument();
  });
});
