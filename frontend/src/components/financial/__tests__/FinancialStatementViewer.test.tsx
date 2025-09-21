import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import './test-setup';
import { FinancialStatementViewer } from '../FinancialStatementViewer';
import { FinancialStatement, Company, FinancialLineItem } from '../../../types/financial';

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

const mockLineItems: FinancialLineItem[] = [
  {
    id: 'line-1',
    statementId: 'statement-1',
    taxonomyConcept: 'us-gaap:Assets',
    standardLabel: 'Total Assets',
    value: 352755000000,
    unit: 'USD',
    contextRef: 'c1',
    statementType: 'Balance Sheet',
    statementSection: 'Assets',
    isCalculated: false,
    createdAt: '2023-12-31T00:00:00Z',
    updatedAt: '2023-12-31T00:00:00Z',
  },
  {
    id: 'line-2',
    statementId: 'statement-1',
    taxonomyConcept: 'us-gaap:Liabilities',
    standardLabel: 'Total Liabilities',
    value: 258549000000,
    unit: 'USD',
    contextRef: 'c1',
    statementType: 'Balance Sheet',
    statementSection: 'Liabilities',
    isCalculated: false,
    createdAt: '2023-12-31T00:00:00Z',
    updatedAt: '2023-12-31T00:00:00Z',
  },
];

describe('FinancialStatementViewer', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('renders the financial statement viewer', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
      />
    );
    
    expect(screen.getByText('Financial Statements')).toBeInTheDocument();
    expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
  });

  it('displays statement selection tabs', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
      />
    );
    
    expect(screen.getByText('Balance Sheet')).toBeInTheDocument();
    expect(screen.getByText('Income Statement')).toBeInTheDocument();
    expect(screen.getByText('Cash Flow')).toBeInTheDocument();
  });

  it('switches between statement types', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
      />
    );
    
    // Initially should show Balance Sheet
    expect(screen.getByText('Total Assets')).toBeInTheDocument();
    
    // Click on Income Statement tab
    const incomeTab = screen.getByText('Income Statement');
    fireEvent.click(incomeTab);
    
    // Should show income statement items
    expect(screen.getByText('Revenue')).toBeInTheDocument();
  });

  it('displays line items in table format', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
      />
    );
    
    expect(screen.getByText('Total Assets')).toBeInTheDocument();
    expect(screen.getByText('$352.8B')).toBeInTheDocument(); // Formatted value
    expect(screen.getByText('Total Liabilities')).toBeInTheDocument();
    expect(screen.getByText('$258.5B')).toBeInTheDocument();
  });

  it('shows statement metadata', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
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
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
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
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
      />
    );
    
    // Should show indicators for calculated items
    const calculatedItems = screen.getAllByText('Calculated');
    expect(calculatedItems.length).toBeGreaterThan(0);
  });

  it('shows line item hierarchy and indentation', () => {
    const hierarchicalLineItems = [
      ...mockLineItems,
      {
        id: 'line-3',
        statementId: 'statement-1',
        taxonomyConcept: 'us-gaap:CashAndCashEquivalentsAtCarryingValue',
        standardLabel: 'Cash and Cash Equivalents',
        value: 29830000000,
        unit: 'USD',
        contextRef: 'c1',
        statementType: 'Balance Sheet',
        statementSection: 'Assets',
        isCalculated: false,
        parentConcept: 'us-gaap:Assets',
        createdAt: '2023-12-31T00:00:00Z',
        updatedAt: '2023-12-31T00:00:00Z',
      },
    ];

    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={hierarchicalLineItems}
      />
    );
    
    // Should show hierarchical structure
    expect(screen.getByText('Cash and Cash Equivalents')).toBeInTheDocument();
  });

  it('handles statement comparison mode', () => {
    const multipleStatements = [
      ...mockFinancialStatements,
      {
        ...mockFinancialStatements[0],
        id: 'statement-2',
        periodEndDate: '2022-12-31',
        fiscalYear: 2022,
      },
    ];

    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={multipleStatements}
        lineItems={mockLineItems}
        comparisonMode={true}
      />
    );
    
    expect(screen.getByText('Comparison Mode')).toBeInTheDocument();
    expect(screen.getByText('2023 vs 2022')).toBeInTheDocument();
  });

  it('shows percentage changes in comparison mode', () => {
    const multipleStatements = [
      ...mockFinancialStatements,
      {
        ...mockFinancialStatements[0],
        id: 'statement-2',
        periodEndDate: '2022-12-31',
        fiscalYear: 2022,
      },
    ];

    const lineItemsWithPrevious = [
      ...mockLineItems,
      {
        ...mockLineItems[0],
        id: 'line-1-prev',
        statementId: 'statement-2',
        value: 323888000000, // Previous year value
      },
    ];

    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={multipleStatements}
        lineItems={lineItemsWithPrevious}
        comparisonMode={true}
      />
    );
    
    // Should show percentage change
    expect(screen.getByText('+8.9%')).toBeInTheDocument();
  });

  it('displays statement sections and subsections', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        showSections={true}
      />
    );
    
    expect(screen.getByText('Assets')).toBeInTheDocument();
    expect(screen.getByText('Liabilities')).toBeInTheDocument();
  });

  it('handles expandable sections', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        showSections={true}
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
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        showAnnotations={true}
      />
    );
    
    expect(screen.getByText('Annotations')).toBeInTheDocument();
  });

  it('handles annotation creation', () => {
    const mockOnCreateAnnotation = jest.fn();
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        showAnnotations={true}
        onCreateAnnotation={mockOnCreateAnnotation}
      />
    );
    
    const addAnnotationButton = screen.getByText('Add Annotation');
    fireEvent.click(addAnnotationButton);
    
    expect(mockOnCreateAnnotation).toHaveBeenCalled();
  });

  it('displays data quality indicators', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        showDataQuality={true}
      />
    );
    
    expect(screen.getByText('Data Quality')).toBeInTheDocument();
    expect(screen.getByText('High Confidence')).toBeInTheDocument();
  });

  it('handles export functionality', () => {
    const mockOnExport = jest.fn();
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        onExport={mockOnExport}
      />
    );
    
    const exportButton = screen.getByText('Export Statement');
    fireEvent.click(exportButton);
    
    expect(mockOnExport).toHaveBeenCalledWith({
      statementId: 'statement-1',
      format: 'PDF',
    });
  });

  it('shows loading state', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={undefined}
      />
    );
    
    expect(screen.getByText('Loading financial statements...')).toBeInTheDocument();
  });

  it('handles empty line items', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={[]}
      />
    );
    
    expect(screen.getByText('No line items available')).toBeInTheDocument();
  });

  it('displays statement footnotes', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        showFootnotes={true}
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
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
      />
    );
    
    // Should adapt to mobile view
    expect(screen.getByText('Financial Statements')).toBeInTheDocument();
  });

  it('shows statement validation status', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        showValidation={true}
      />
    );
    
    expect(screen.getByText('Validation Status')).toBeInTheDocument();
    expect(screen.getByText('✓ Validated')).toBeInTheDocument();
  });

  it('displays XBRL processing status', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
      />
    );
    
    expect(screen.getByText('XBRL Status: Completed')).toBeInTheDocument();
  });

  it('handles statement amendments and restatements', () => {
    const amendedStatement = {
      ...mockFinancialStatements[0],
      isAmended: true,
      amendmentType: '10-K/A',
      originalFilingDate: '2023-12-31',
    };

    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={[amendedStatement]}
        lineItems={mockLineItems}
      />
    );
    
    expect(screen.getByText('Amended Filing')).toBeInTheDocument();
    expect(screen.getByText('10-K/A')).toBeInTheDocument();
  });

  it('shows statement download options', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        showDownloadOptions={true}
      />
    );
    
    expect(screen.getByText('Download Options')).toBeInTheDocument();
    expect(screen.getByText('Original Filing')).toBeInTheDocument();
    expect(screen.getByText('XBRL Data')).toBeInTheDocument();
  });

  it('handles line item drill-down', () => {
    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        allowDrillDown={true}
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
        company={mockCompany}
        statements={mockFinancialStatements}
        lineItems={mockLineItems}
        showCalculations={true}
      />
    );
    
    expect(screen.getByText('Calculated Ratios')).toBeInTheDocument();
    expect(screen.getByText('Debt to Assets: 73.3%')).toBeInTheDocument();
  });

  it('handles statement navigation', () => {
    const multipleStatements = [
      ...mockFinancialStatements,
      {
        ...mockFinancialStatements[0],
        id: 'statement-2',
        periodEndDate: '2023-09-30',
        fiscalQuarter: 3,
      },
    ];

    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={multipleStatements}
        lineItems={mockLineItems}
        allowNavigation={true}
      />
    );
    
    expect(screen.getByText('← Previous')).toBeInTheDocument();
    expect(screen.getByText('Next →')).toBeInTheDocument();
  });

  it('shows statement timeline', () => {
    const multipleStatements = [
      ...mockFinancialStatements,
      {
        ...mockFinancialStatements[0],
        id: 'statement-2',
        periodEndDate: '2023-09-30',
        fiscalQuarter: 3,
      },
      {
        ...mockFinancialStatements[0],
        id: 'statement-3',
        periodEndDate: '2023-06-30',
        fiscalQuarter: 2,
      },
    ];

    render(
      <FinancialStatementViewer
        company={mockCompany}
        statements={multipleStatements}
        lineItems={mockLineItems}
        showTimeline={true}
      />
    );
    
    expect(screen.getByText('Statement Timeline')).toBeInTheDocument();
    expect(screen.getByText('Q2 2023')).toBeInTheDocument();
    expect(screen.getByText('Q3 2023')).toBeInTheDocument();
    expect(screen.getByText('Q4 2023')).toBeInTheDocument();
  });
});
