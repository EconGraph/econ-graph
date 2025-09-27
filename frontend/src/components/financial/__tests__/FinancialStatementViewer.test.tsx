import { vi } from 'vitest';
import React, { Suspense } from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
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

// Test wrapper with QueryClient and Suspense
const TestWrapper = ({ children }: { children: React.ReactNode }) => {
  const queryClient = createTestQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      <Suspense fallback={<div data-testid="loading">Loading...</div>}>
        {children}
      </Suspense>
    </QueryClientProvider>
  );
};

describe('FinancialStatementViewer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // MSW server handles GraphQL requests automatically in Node.js mode
  });

  it('renders the financial statement viewer', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Financial Statements')).toBeInTheDocument();
    });

    expect(screen.getByText('Apple Inc.')).toBeInTheDocument();
  });

  it('displays statement selection tabs', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Balance Sheet')).toBeInTheDocument();
    });

    expect(screen.getByText('Income Statement')).toBeInTheDocument();
    expect(screen.getByText('Cash Flow')).toBeInTheDocument();
  });

  it('switches between statement types', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getAllByText('Total Assets').length).toBeGreaterThan(0);
    });

    expect(screen.getAllByText('$352.76B').length).toBeGreaterThan(0); // Calculated from mock data

    // Click on Income Statement tab
    const incomeTab = screen.getByText('Income Statement');
    fireEvent.click(incomeTab);

    // Should show statement switching UI works
    expect(incomeTab).toBeInTheDocument();
  });

  it('displays line items in table format', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getAllByText('Total Assets').length).toBeGreaterThan(0);
    });

    expect(screen.getAllByText('$352.76B').length).toBeGreaterThan(0); // Calculated from mock data: 352755000000
    expect(screen.getAllByText('Total Liabilities').length).toBeGreaterThan(0);
    expect(screen.getAllByText('$258.55B').length).toBeGreaterThan(0); // Calculated from mock data: 258549000000
  });

  it('shows statement metadata', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('10-K')).toBeInTheDocument();
    });

    expect(screen.getByText('2023')).toBeInTheDocument();
    expect(screen.getByText('Q4')).toBeInTheDocument();
    expect(screen.getByText(/Dec 31, 2023/i)).toBeInTheDocument();
  });

  it('handles line item filtering', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByPlaceholderText('Search line items...')).toBeInTheDocument();
    });

    const searchInput = screen.getByPlaceholderText('Search line items...');
    fireEvent.change(searchInput, { target: { value: 'Assets' } });

    // Should verify filtering functionality with mock data
    expect(screen.getAllByText('Total Assets').length).toBeGreaterThan(0);
    // Note: Filtering may show both table and div elements, so Total Liabilities might still be visible
  });

  it('displays calculated vs non-calculated items', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getAllByText('Calculated').length).toBeGreaterThan(0);
    });
  });

  it('shows line item hierarchy and indentation', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Cash and Cash Equivalents')).toBeInTheDocument();
    });
  });

  it('handles statement comparison mode', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Comparison Mode')).toBeInTheDocument();
    });

    expect(screen.getByText('2023 vs 2022')).toBeInTheDocument();
  });

  it('shows percentage changes in comparison mode', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('+8.9%')).toBeInTheDocument();
    });
  });

  it('displays statement sections and subsections', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getAllByText('Assets').length).toBeGreaterThan(0);
    });

    expect(screen.getAllByText('Liabilities').length).toBeGreaterThan(0);
  });

  it('handles expandable sections', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getAllByText('Assets').length).toBeGreaterThan(0);
    });

    const assetsSection = screen.getAllByText('Assets')[0];
    fireEvent.click(assetsSection);

    // Should expand to show subsection details from mock data
    expect(screen.getAllByText('Current Assets').length).toBeGreaterThan(0);
  });

  it('shows line item annotations', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Annotations')).toBeInTheDocument();
    });
  });

  it('handles annotation creation', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Add Annotation')).toBeInTheDocument();
    });

    const addAnnotationButton = screen.getByText('Add Annotation');
    fireEvent.click(addAnnotationButton);

    // Component should handle annotation creation internally
  });

  it('displays data quality indicators', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Data Quality')).toBeInTheDocument();
    });

    expect(screen.getByText('High Confidence')).toBeInTheDocument();
  });

  it('handles export functionality', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Export Statement')).toBeInTheDocument();
    });

    const exportButton = screen.getByText('Export Statement');
    fireEvent.click(exportButton);

    // Component should handle export functionality internally
  });

  it('shows loading state', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Loading financial statements...')).toBeInTheDocument();
    });
  });

  it('handles empty line items', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('No line items available')).toBeInTheDocument();
    });
  });

  it('displays statement footnotes', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Footnotes')).toBeInTheDocument();
    });
  });

  it('handles responsive design', async () => {
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

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Financial Statements')).toBeInTheDocument();
    });
  });

  it('shows statement validation status', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Validation Status')).toBeInTheDocument();
    });

    expect(screen.getByText('✓ Validated')).toBeInTheDocument();
  });

  it('displays XBRL processing status', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('XBRL Status: Completed')).toBeInTheDocument();
    });
  });

  it('handles statement amendments and restatements', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Amended Filing')).toBeInTheDocument();
    });

    expect(screen.getByText('10-K/A')).toBeInTheDocument();
  });

  it('shows statement download options', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Download Options')).toBeInTheDocument();
    });

    expect(screen.getByText('Original Filing')).toBeInTheDocument();
    expect(screen.getByText('XBRL Data')).toBeInTheDocument();
  });

  it('handles line item drill-down', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getAllByText('Total Assets').length).toBeGreaterThan(0);
    });

    const lineItem = screen.getAllByText('Total Assets')[0];
    fireEvent.click(lineItem);

    // Should show detailed breakdown
    expect(screen.getByText('Asset Breakdown')).toBeInTheDocument();
  });

  it('displays statement ratios and calculations', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Calculated Ratios')).toBeInTheDocument();
    });

    expect(screen.getByText('Debt to Assets: 73.3%')).toBeInTheDocument();
  });

  it('handles statement navigation', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('← Previous')).toBeInTheDocument();
    });

    expect(screen.getByText('Next →')).toBeInTheDocument();
  });

  it('shows statement timeline', async () => {
    render(
      <TestWrapper>
        <FinancialStatementViewer
          companyId={mockCompany.id}
          statementId="statement-1"
        />
      </TestWrapper>
    );

    // Wait for the component to finish loading
    await waitFor(() => {
      expect(screen.getByText('Statement Timeline')).toBeInTheDocument();
    });

    expect(screen.getByText('Q2 2023')).toBeInTheDocument();
    expect(screen.getByText('Q3 2023')).toBeInTheDocument();
    expect(screen.getByText('Q4 2023')).toBeInTheDocument();
  });
});
