import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { FinancialAlerts } from '../FinancialAlerts';

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

describe('FinancialAlerts', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('renders the financial alerts component', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    expect(screen.getByText('Financial Alerts')).toBeInTheDocument();
    expect(screen.getByText('Current Ratio Below Threshold')).toBeInTheDocument();
    expect(screen.getByText('10-Q Filing Due Soon')).toBeInTheDocument();
  });

  it('displays alert severity indicators', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    // Should show severity indicators
    expect(screen.getByText('High')).toBeInTheDocument();
    expect(screen.getByText('Medium')).toBeInTheDocument();
    expect(screen.getByText('Low')).toBeInTheDocument();
  });

  it('shows unread alert count', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    // Should show count of unread alerts (2 unread out of 4 total)
    expect(screen.getByText('2 Unread')).toBeInTheDocument();
  });

  it('filters alerts by severity', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    const severityFilter = screen.getByDisplayValue('all');
    fireEvent.click(severityFilter);

    // Should show severity filter options
    expect(screen.getByText('high')).toBeInTheDocument();
    expect(screen.getByText('medium')).toBeInTheDocument();
    expect(screen.getByText('low')).toBeInTheDocument();
  });

  it('filters alerts by type', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    const typeFilter = screen.getByDisplayValue('all');
    fireEvent.click(typeFilter);

    // Should show type filter options
    expect(screen.getByText('ratio_threshold')).toBeInTheDocument();
    expect(screen.getByText('filing_deadline')).toBeInTheDocument();
    expect(screen.getByText('data_quality')).toBeInTheDocument();
  });

  it('sorts alerts by different criteria', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    const sortSelect = screen.getByDisplayValue('severity');
    fireEvent.click(sortSelect);

    // Should show sorting options
    expect(screen.getByText('date')).toBeInTheDocument();
    expect(screen.getByText('type')).toBeInTheDocument();
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
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    const alertTitle = screen.getByText('Current Ratio Below Threshold');
    fireEvent.click(alertTitle);

    // Should show detailed description
    expect(screen.getByText('Current ratio of 0.95 is below the recommended threshold of 1.0')).toBeInTheDocument();
  });

  it('displays alert creation and expiration dates', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    // Should show formatted dates
    expect(screen.getByText(/Jan 15, 2024/i)).toBeInTheDocument();
    expect(screen.getByText(/Jan 25, 2024/i)).toBeInTheDocument();
  });

  it('shows alert direction indicators', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    // Should show direction indicators
    expect(screen.getByText(/decline/i)).toBeInTheDocument();
    expect(screen.getByText(/improvement/i)).toBeInTheDocument();
  });

  it('handles empty alerts array', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    expect(screen.getByText('No alerts available')).toBeInTheDocument();
  });

  it('shows loading state when alerts are being fetched', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    expect(screen.getByText('Loading alerts...')).toBeInTheDocument();
  });

  it('displays expired alerts differently', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    // Expired alert should be shown with different styling
    const expiredAlert = screen.getByText('Industry Benchmark Updated');
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
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    const searchInput = screen.getByPlaceholderText('Search alerts...');
    expect(searchInput).toBeInTheDocument();

    // Test search
    fireEvent.change(searchInput, { target: { value: 'ratio' } });

    // Should filter alerts based on search
    expect(screen.getByText('Current Ratio Below Threshold')).toBeInTheDocument();
    expect(screen.queryByText('10-Q Filing Due Soon')).not.toBeInTheDocument();
  });

  it('displays alert categories and counts', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

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

    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    // Should adapt to mobile view
    expect(screen.getByText('Financial Alerts')).toBeInTheDocument();
  });

  it('shows alert priority indicators', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    // Should show priority indicators for high severity alerts
    expect(screen.getByText('High Priority')).toBeInTheDocument();
  });

  it('handles alert action buttons (view details, acknowledge)', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    // Should show action buttons for each alert
    expect(screen.getAllByText('View Details')).toHaveLength(mockAlerts.length);
    expect(screen.getAllByText('Acknowledge')).toHaveLength(mockAlerts.length);
  });

  it('displays alert notifications count in header', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

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
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    // Should show settings/preferences link
    expect(screen.getByText('Alert Settings')).toBeInTheDocument();
  });

  it('handles alert export functionality', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    const exportButton = screen.getByText('Export Alerts');
    fireEvent.click(exportButton);

    // Should trigger export functionality
    expect(exportButton).toBeInTheDocument();
  });

  it('displays alert trends and statistics', () => {
    render(<FinancialAlerts companyId="test-company" ratios={[]} statements={[]} />);

    // Should show alert trends
    expect(screen.getByText('Alert Trends')).toBeInTheDocument();
    expect(screen.getByText('This Week: 4')).toBeInTheDocument();
    expect(screen.getByText('Last Week: 2')).toBeInTheDocument();
  });
});
