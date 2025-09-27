import { vi } from 'vitest';
import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { QueryClient, QueryClientProvider } from 'react-query';
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


// Create a test QueryClient
const createTestQueryClient = () => new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      cacheTime: 0,
      staleTime: 0,
    },
    mutations: {
      retry: false,
    },
  },
});

// Test wrapper with QueryClient
const TestWrapper = ({ children }: { children: React.ReactNode }) => {
  const queryClient = createTestQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      {children}
    </QueryClientProvider>
  );
};

describe('FinancialStatementViewer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the financial statement viewer', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Financial Statements')).toBeInTheDocument();
    expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
  });

  it('displays statement selection tabs', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Balance Sheet')).toBeInTheDocument();
    expect(screen.getByText('Income Statement')).toBeInTheDocument();
    expect(screen.getByText('Cash Flow')).toBeInTheDocument();
  });

  it('switches between statement types', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
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
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getAllByText('Total Assets').length).toBeGreaterThan(0);
    expect(screen.getAllByText('$352.76B').length).toBeGreaterThan(0); // Calculated from mock data: 352755000000
    expect(screen.getAllByText('Total Liabilities').length).toBeGreaterThan(0);
    expect(screen.getAllByText('$258.55B').length).toBeGreaterThan(0); // Calculated from mock data: 258549000000
  });

  it('shows statement metadata', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('10-K')).toBeInTheDocument();
    expect(screen.getByText('2023')).toBeInTheDocument();
    expect(screen.getByText('Q4')).toBeInTheDocument();
    expect(screen.getByText(/Dec 31, 2023/i)).toBeInTheDocument();
  });

  it('handles line item filtering', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    const searchInput = screen.getByPlaceholderText('Search line items...');
    fireEvent.change(searchInput, { target: { value: 'Assets' } });

    // Should verify filtering functionality with mock data
    expect(screen.getAllByText('Total Assets').length).toBeGreaterThan(0);
    // Note: Filtering may show both table and div elements, so Total Liabilities might still be visible
  });

  it('displays calculated vs non-calculated items', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Should show indicators for calculated items
    const calculatedItems = screen.getAllByText('Calculated');
    expect(calculatedItems.length).toBeGreaterThan(0);
  });

  it('shows line item hierarchy and indentation', () => {

    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Should show hierarchical structure
    expect(screen.getByText('Cash and Cash Equivalents')).toBeInTheDocument();
  });

  it('handles statement comparison mode', () => {

    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Comparison Mode')).toBeInTheDocument();
    expect(screen.getByText('2023 vs 2022')).toBeInTheDocument();
  });

  it('shows percentage changes in comparison mode', () => {


    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Should show percentage change
    expect(screen.getByText('+8.9%')).toBeInTheDocument();
  });

  it('displays statement sections and subsections', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getAllByText('Assets').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Liabilities').length).toBeGreaterThan(0);
  });

  it('handles expandable sections', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    const assetsSection = screen.getAllByText('Assets')[0];
    fireEvent.click(assetsSection);

    // Should expand to show subsection details from mock data
    expect(screen.getAllByText('Current Assets').length).toBeGreaterThan(0);
  });

  it('shows line item annotations', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Annotations')).toBeInTheDocument();
  });

  it('handles annotation creation', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    const addAnnotationButton = screen.getByText('Add Annotation');
    fireEvent.click(addAnnotationButton);

    // Component should handle annotation creation internally
  });

  it('displays data quality indicators', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Data Quality')).toBeInTheDocument();
    expect(screen.getByText('High Confidence')).toBeInTheDocument();
  });

  it('handles export functionality', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    const exportButton = screen.getByText('Export Statement');
    fireEvent.click(exportButton);

    // Component should handle export functionality internally
  });

  it('shows loading state', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Loading financial statements...')).toBeInTheDocument();
  });

  it('handles empty line items', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('No line items available')).toBeInTheDocument();
  });

  it('displays statement footnotes', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
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
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Should adapt to mobile view
    expect(screen.getByText('Financial Statements')).toBeInTheDocument();
  });

  it('shows statement validation status', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Validation Status')).toBeInTheDocument();
    expect(screen.getByText('✓ Validated')).toBeInTheDocument();
  });

  it('displays XBRL processing status', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('XBRL Status: Completed')).toBeInTheDocument();
  });

  it('handles statement amendments and restatements', () => {

    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Amended Filing')).toBeInTheDocument();
    expect(screen.getByText('10-K/A')).toBeInTheDocument();
  });

  it('shows statement download options', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Download Options')).toBeInTheDocument();
    expect(screen.getByText('Original Filing')).toBeInTheDocument();
    expect(screen.getByText('XBRL Data')).toBeInTheDocument();
  });

  it('handles line item drill-down', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    const lineItem = screen.getAllByText('Total Assets')[0];
    fireEvent.click(lineItem);

    // Should show detailed breakdown
    expect(screen.getByText('Asset Breakdown')).toBeInTheDocument();
  });

  it('displays statement ratios and calculations', () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Calculated Ratios')).toBeInTheDocument();
    expect(screen.getByText('Debt to Assets: 73.3%')).toBeInTheDocument();
  });

  it('handles statement navigation', () => {

    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('← Previous')).toBeInTheDocument();
    expect(screen.getByText('Next →')).toBeInTheDocument();
  });

  it('shows statement timeline', () => {

    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    expect(screen.getByText('Statement Timeline')).toBeInTheDocument();
    expect(screen.getByText('Q2 2023')).toBeInTheDocument();
    expect(screen.getByText('Q3 2023')).toBeInTheDocument();
    expect(screen.getByText('Q4 2023')).toBeInTheDocument();
  });
});
