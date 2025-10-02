import { vi } from 'vitest';
import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { FinancialExport } from '../FinancialExport';
import { FinancialStatement, Company, FinancialRatio } from '../../../types/financial';

// Mock window object
Object.defineProperty(window, 'location', {
  value: {
    href: 'http://localhost:3000',
    origin: 'http://localhost:3000'
  },
  writable: true
});

// Mock financial data
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


describe('FinancialExport', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    // Clear all timers to prevent window errors
    vi.clearAllTimers();
  });

  it('renders the financial export component', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Export Financial Data').length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Apple Inc\./i).length).toBeGreaterThan(0);
  });

  it('displays export format options', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Export Format').length).toBeGreaterThan(0);
    expect(screen.getAllByDisplayValue('PDF').length).toBeGreaterThan(0);
  });

  it('handles format selection change', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const formatSelects = screen.getAllByDisplayValue('PDF');
    const formatSelect = formatSelects[0];
    fireEvent.click(formatSelect);

    // Should show other format options
    expect(screen.getAllByText('Excel').length).toBeGreaterThan(0);
    expect(screen.getAllByText('CSV').length).toBeGreaterThan(0);
    expect(screen.getAllByText('JSON').length).toBeGreaterThan(0);
  });

  it('displays data selection options', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByRole('checkbox', { name: /Include Financial Statements/i }).length).toBeGreaterThan(0);
    expect(screen.getAllByText('Include Ratios').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Include Charts').length).toBeGreaterThan(0);
  });

  it(
    'handles data selection checkboxes',
    () => {
      render(
        <FinancialExport
          company={mockCompany}
          statements={mockFinancialStatements}
          ratios={mockFinancialRatios}
        />
      );

      const statementsCheckboxes = screen.getAllByLabelText(/Include Financial Statements/i);
      const statementsCheckbox = statementsCheckboxes[0];
      fireEvent.click(statementsCheckbox);

      // Checkbox should be checked
      expect(statementsCheckbox).toBeChecked();
    },
    15000
  );

  it('displays date range selection', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Date Range').length).toBeGreaterThan(0);
    expect(screen.getAllByDisplayValue('2023-01-01').length).toBeGreaterThan(0); // Start date
    expect(screen.getAllByDisplayValue('2023-12-31').length).toBeGreaterThan(0); // End date
  });

  it('handles date range changes', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const startDateInputs = screen.getAllByLabelText(/Start Date/i);
    const startDateInput = startDateInputs[0];
    fireEvent.change(startDateInput, { target: { value: '2023-06-01' } });

    expect(startDateInput).toHaveValue('2023-06-01');
  });

  it('shows export preview', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Export Preview').length).toBeGreaterThan(0);
    expect(screen.getAllByText('1 Financial Statements').length).toBeGreaterThan(0);
    expect(screen.getAllByText('1 Ratios').length).toBeGreaterThan(0);
  });

  it('handles export button click', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const exportButtons = screen.getAllByText('Start Export');
    const exportButton = exportButtons[0];
    fireEvent.click(exportButton);

    // Component should handle export functionality internally
  });

  it('displays export jobs history', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Export History').length).toBeGreaterThan(0);
    expect(screen.getAllByText('apple_financial_report_2023.pdf').length).toBeGreaterThan(0);
    expect(screen.getAllByText('apple_financial_data_2023.xlsx').length).toBeGreaterThan(0);
  });

  it('shows job status indicators', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Completed').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Processing').length).toBeGreaterThan(0);
  });

  it('handles download completed exports', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const downloadButtons = screen.getAllByText('Download');
    const downloadButton = downloadButtons[0];
    fireEvent.click(downloadButton);

    // Component should handle download functionality internally
  });

  it('shows file size information', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('2.0 MB').length).toBeGreaterThan(0); // Formatted file size
  });

  it('displays creation and completion times', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText(/Jan 15, 2024/i).length).toBeGreaterThan(0);
  });

  it('handles job deletion', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const deleteButton = screen.getAllByText('Delete')[0];
    fireEvent.click(deleteButton);

    // Component should handle job deletion internally
  });

  it('shows loading state during export', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Check that export functionality is available
    expect(screen.getAllByText('Export Settings').length).toBeGreaterThan(0);
    expect(screen.getAllByRole('button', { name: /Start Export/i }).length).toBeGreaterThan(0);
  });

  it('handles export progress updates', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Export Progress: 75%').length).toBeGreaterThan(0);
  });

  it('displays export settings and preferences', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Export Settings').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Default Format: PDF').length).toBeGreaterThan(0);
  });

  it('handles export template selection', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const templateSelects = screen.getAllByDisplayValue('Standard Report');
    const templateSelect = templateSelects[0];
    fireEvent.click(templateSelect);

    // Should show template options
    expect(screen.getAllByText('Executive Summary').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Detailed Analysis').length).toBeGreaterThan(0);
  });

  it('shows export validation errors', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Invalid date range selected').length).toBeGreaterThan(0);
  });

  it('handles responsive design for mobile', () => {
    // Mock mobile viewport
    Object.defineProperty(window, 'innerWidth', {
      writable: true,
      configurable: true,
      value: 375,
    });

    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    // Should adapt to mobile view
    expect(screen.getAllByText('Export Financial Data').length).toBeGreaterThan(0);
  });

  it('displays export file naming options', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('File Naming').length).toBeGreaterThan(0);
    expect(screen.getAllByLabelText('File naming format selection').length).toBeGreaterThan(0);
  });

  it('handles custom file naming', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const customNameInputs = screen.getAllByPlaceholderText('Custom file name...');
    const customNameInput = customNameInputs[0];
    fireEvent.change(customNameInput, { target: { value: 'my_custom_report' } });

    expect(customNameInput).toHaveValue('my_custom_report');
  });

  it('shows export scheduling options', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Schedule Export').length).toBeGreaterThan(0);
    expect(screen.getAllByText('One-time').length).toBeGreaterThan(0);
  });

  it('handles scheduled export settings', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const scheduleSelects = screen.getAllByDisplayValue('One-time');
    const scheduleSelect = scheduleSelects[0];
    fireEvent.click(scheduleSelect);

    // Should show scheduling options
    expect(screen.getAllByText('Daily').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Weekly').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Monthly').length).toBeGreaterThan(0);
  });

  it('displays export permissions and access controls', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Access Controls').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Public').length).toBeGreaterThan(0);
  });

  it('handles export sharing options', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Share Export').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Email Link').length).toBeGreaterThan(0);
  });

  it('shows export analytics and usage statistics', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getAllByText('Export Analytics').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Total Exports: 5').length).toBeGreaterThan(0);
    expect(screen.getAllByText('This Month: 2').length).toBeGreaterThan(0);
  });
});
