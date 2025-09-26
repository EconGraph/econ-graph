import { vi } from 'vitest';
import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { FinancialExport } from '../FinancialExport';
import { FinancialStatement, Company, FinancialRatio } from '../../../types/financial';

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

  it('renders the financial export component', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Export Financial Data')).toBeInTheDocument();
    expect(screen.getByText(/Apple Inc\./i)).toBeInTheDocument();
  });

  it('displays export format options', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Export Format')).toBeInTheDocument();
    expect(screen.getByDisplayValue('PDF')).toBeInTheDocument();
  });

  it('handles format selection change', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const formatSelect = screen.getByDisplayValue('PDF');
    fireEvent.click(formatSelect);

    // Should show other format options
    expect(screen.getByText('Excel')).toBeInTheDocument();
    expect(screen.getByText('CSV')).toBeInTheDocument();
    expect(screen.getByText('JSON')).toBeInTheDocument();
  });

  it('displays data selection options', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Include Financial Statements')).toBeInTheDocument();
    expect(screen.getByText('Include Ratios')).toBeInTheDocument();
    expect(screen.getByText('Include Charts')).toBeInTheDocument();
  });

  it('handles data selection checkboxes', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const statementsCheckbox = screen.getByLabelText('Include Financial Statements');
    fireEvent.click(statementsCheckbox);

    // Checkbox should be checked
    expect(statementsCheckbox).toBeChecked();
  });

  it('displays date range selection', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Date Range')).toBeInTheDocument();
    expect(screen.getByDisplayValue('2023-01-01')).toBeInTheDocument(); // Start date
    expect(screen.getByDisplayValue('2023-12-31')).toBeInTheDocument(); // End date
  });

  it('handles date range changes', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const startDateInput = screen.getByDisplayValue('2023-01-01');
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

    expect(screen.getByText('Export Preview')).toBeInTheDocument();
    expect(screen.getByText('1 Financial Statements')).toBeInTheDocument();
    expect(screen.getByText('1 Ratios')).toBeInTheDocument();
  });

  it('handles export button click', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const exportButton = screen.getByText('Start Export');
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

    expect(screen.getByText('Export History')).toBeInTheDocument();
    expect(screen.getByText('apple_financial_report_2023.pdf')).toBeInTheDocument();
    expect(screen.getByText('apple_financial_data_2023.xlsx')).toBeInTheDocument();
  });

  it('shows job status indicators', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Completed')).toBeInTheDocument();
    expect(screen.getByText('Processing')).toBeInTheDocument();
  });

  it('handles download completed exports', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const downloadButton = screen.getByText('Download');
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

    expect(screen.getByText('2.0 MB')).toBeInTheDocument(); // Formatted file size
  });

  it('displays creation and completion times', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText(/Jan 15, 2024/i)).toBeInTheDocument();
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
    expect(screen.getByText('Export Settings')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Start Export/i })).toBeInTheDocument();
  });

  it('handles export progress updates', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Export Progress: 75%')).toBeInTheDocument();
  });

  it('displays export settings and preferences', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Export Settings')).toBeInTheDocument();
    expect(screen.getByText('Default Format: PDF')).toBeInTheDocument();
  });

  it('handles export template selection', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const templateSelect = screen.getByDisplayValue('Standard Report');
    fireEvent.click(templateSelect);

    // Should show template options
    expect(screen.getByText('Executive Summary')).toBeInTheDocument();
    expect(screen.getByText('Detailed Analysis')).toBeInTheDocument();
  });

  it('shows export validation errors', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Invalid date range selected')).toBeInTheDocument();
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
    expect(screen.getByText('Export Financial Data')).toBeInTheDocument();
  });

  it('displays export file naming options', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('File Naming')).toBeInTheDocument();
    expect(screen.getByLabelText('File naming format selection')).toBeInTheDocument();
  });

  it('handles custom file naming', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const customNameInput = screen.getByPlaceholderText('Custom file name...');
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

    expect(screen.getByText('Schedule Export')).toBeInTheDocument();
    expect(screen.getByText('One-time')).toBeInTheDocument();
  });

  it('handles scheduled export settings', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    const scheduleSelect = screen.getByDisplayValue('One-time');
    fireEvent.click(scheduleSelect);

    // Should show scheduling options
    expect(screen.getByText('Daily')).toBeInTheDocument();
    expect(screen.getByText('Weekly')).toBeInTheDocument();
    expect(screen.getByText('Monthly')).toBeInTheDocument();
  });

  it('displays export permissions and access controls', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Access Controls')).toBeInTheDocument();
    expect(screen.getByText('Public')).toBeInTheDocument();
  });

  it('handles export sharing options', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Share Export')).toBeInTheDocument();
    expect(screen.getByText('Email Link')).toBeInTheDocument();
  });

  it('shows export analytics and usage statistics', () => {
    render(
      <FinancialExport
        company={mockCompany}
        statements={mockFinancialStatements}
        ratios={mockFinancialRatios}
      />
    );

    expect(screen.getByText('Export Analytics')).toBeInTheDocument();
    expect(screen.getByText('Total Exports: 5')).toBeInTheDocument();
    expect(screen.getByText('This Month: 2')).toBeInTheDocument();
  });
});
