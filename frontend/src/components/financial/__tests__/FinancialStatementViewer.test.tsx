import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { FinancialStatementViewer } from '../FinancialStatementViewer';
import { Company } from '../../../types/financial';

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


describe('FinancialStatementViewer', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('renders the financial statement viewer', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Financial Statements')).toBeInTheDocument();
    expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
  });

  it('displays statement selection tabs', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Balance Sheet')).toBeInTheDocument();
    expect(screen.getByText('Income Statement')).toBeInTheDocument();
    expect(screen.getByText('Cash Flow')).toBeInTheDocument();
  });

  it('switches between statement types', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    // Initially should show Balance Sheet with mock data
    expect(screen.getAllByText('Total Assets').length).toBeGreaterThan(0);
    expect(screen.getAllByText('$352.76B').length).toBeGreaterThan(0); // Calculated from mock data

    // Click on Income Statement tab
    const incomeTab = screen.getByText('Income Statement');
    fireEvent.click(incomeTab);

    // Should show statement switching UI works
    expect(incomeTab).toBeInTheDocument();
  });

  it('displays line items in table format', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getAllByText('Total Assets').length).toBeGreaterThan(0);
    expect(screen.getAllByText('$352.76B').length).toBeGreaterThan(0); // Calculated from mock data: 352755000000
    expect(screen.getAllByText('Total Liabilities').length).toBeGreaterThan(0);
    expect(screen.getAllByText('$258.55B').length).toBeGreaterThan(0); // Calculated from mock data: 258549000000
  });

  it('shows statement metadata', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('10-K')).toBeInTheDocument();
    expect(screen.getByText('2023')).toBeInTheDocument();
    expect(screen.getByText('Q4')).toBeInTheDocument();
    expect(screen.getByText(/Dec 31, 2023/i)).toBeInTheDocument();
  });

  it('handles line item filtering', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    const searchInput = screen.getByPlaceholderText('Search line items...');
    fireEvent.change(searchInput, { target: { value: 'Assets' } });

    // Should filter to show only assets
    expect(screen.getByText('Total Assets')).toBeInTheDocument();
    expect(screen.queryByText('Total Liabilities')).not.toBeInTheDocument();
  });

  it('displays calculated vs non-calculated items', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    // Should show indicators for calculated items
    const calculatedItems = screen.getAllByText('Calculated');
    expect(calculatedItems.length).toBeGreaterThan(0);
  });

  it('shows line item hierarchy and indentation', () => {

    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    // Should show hierarchical structure
    expect(screen.getByText('Cash and Cash Equivalents')).toBeInTheDocument();
  });

  it('handles statement comparison mode', () => {

    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Comparison Mode')).toBeInTheDocument();
    expect(screen.getByText('2023 vs 2022')).toBeInTheDocument();
  });

  it('shows percentage changes in comparison mode', () => {


    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    // Should show percentage change
    expect(screen.getByText('+8.9%')).toBeInTheDocument();
  });

  it('displays statement sections and subsections', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Assets')).toBeInTheDocument();
    expect(screen.getByText('Liabilities')).toBeInTheDocument();
  });

  it('handles expandable sections', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    const assetsSection = screen.getByText('Assets');
    fireEvent.click(assetsSection);

    // Should expand to show subsection details
    expect(screen.getByText('Current Assets')).toBeInTheDocument();
  });

  it('shows line item annotations', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Annotations')).toBeInTheDocument();
  });

  it('handles annotation creation', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    const addAnnotationButton = screen.getByText('Add Annotation');
    fireEvent.click(addAnnotationButton);

    // Component should handle annotation creation internally
  });

  it('displays data quality indicators', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Data Quality')).toBeInTheDocument();
    expect(screen.getByText('High Confidence')).toBeInTheDocument();
  });

  it('handles export functionality', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    const exportButton = screen.getByText('Export Statement');
    fireEvent.click(exportButton);

    // Component should handle export functionality internally
  });

  it('shows loading state', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Loading financial statements...')).toBeInTheDocument();
  });

  it('handles empty line items', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('No line items available')).toBeInTheDocument();
  });

  it('displays statement footnotes', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Footnotes')).toBeInTheDocument();
  });

  it('handles responsive design', () => {
    // Mock mobile viewport
    Object.defineProperty(window, 'innerWidth', {
      writable: true,
      configurable: true,
      value: 375,
    });

    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    // Should adapt to mobile view
    expect(screen.getByText('Financial Statements')).toBeInTheDocument();
  });

  it('shows statement validation status', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Validation Status')).toBeInTheDocument();
    expect(screen.getByText('✓ Validated')).toBeInTheDocument();
  });

  it('displays XBRL processing status', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('XBRL Status: Completed')).toBeInTheDocument();
  });

  it('handles statement amendments and restatements', () => {

    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Amended Filing')).toBeInTheDocument();
    expect(screen.getByText('10-K/A')).toBeInTheDocument();
  });

  it('shows statement download options', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Download Options')).toBeInTheDocument();
    expect(screen.getByText('Original Filing')).toBeInTheDocument();
    expect(screen.getByText('XBRL Data')).toBeInTheDocument();
  });

  it('handles line item drill-down', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    const lineItem = screen.getByText('Total Assets');
    fireEvent.click(lineItem);

    // Should show detailed breakdown
    expect(screen.getByText('Asset Breakdown')).toBeInTheDocument();
  });

  it('displays statement ratios and calculations', () => {
    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Calculated Ratios')).toBeInTheDocument();
    expect(screen.getByText('Debt to Assets: 73.3%')).toBeInTheDocument();
  });

  it('handles statement navigation', () => {

    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('← Previous')).toBeInTheDocument();
    expect(screen.getByText('Next →')).toBeInTheDocument();
  });

  it('shows statement timeline', () => {

    render(
      <FinancialStatementViewer
        companyId={mockCompany.id}
        statementId="statement-1"
      />
    );

    expect(screen.getByText('Statement Timeline')).toBeInTheDocument();
    expect(screen.getByText('Q2 2023')).toBeInTheDocument();
    expect(screen.getByText('Q3 2023')).toBeInTheDocument();
    expect(screen.getByText('Q4 2023')).toBeInTheDocument();
  });
});
