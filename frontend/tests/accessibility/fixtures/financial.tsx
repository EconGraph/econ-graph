import React from 'react';
import { createRoot } from 'react-dom/client';
import CssBaseline from '@mui/material/CssBaseline';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ThemeProvider } from '../../../src/contexts/ThemeContext';
import { FinancialExport } from '../../../src/components/financial/FinancialExport';
import { FinancialMobile } from '../../../src/components/financial/FinancialMobile';
import { FinancialDashboard } from '../../../src/components/financial/FinancialDashboard';
import type { FinancialStatement, FinancialRatio, Company } from '../../../src/types/financial';
import '../../../src/index.css';

// Real components and production styles, with deterministic data and no backend.
const financialStatements: FinancialStatement[] = [
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

const company: Company = {
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

const financialRatios: FinancialRatio[] = [
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

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
queryClient.setQueryData(['financial-dashboard', company.id], {
  ...company,
  financialStatements: [
    { id: 'statement-1', type: '10-K', period: '2023' },
    { id: 'statement-2', type: '10-Q', period: '2024' },
  ],
  financialRatios: [],
});
const surface = new URLSearchParams(location.search).get('surface');
createRoot(document.getElementById('root')!).render(
  <QueryClientProvider client={queryClient}>
    <CssBaseline />
    <ThemeProvider>
      {surface === 'dashboard' ? (
        <FinancialDashboard companyId={company.id} />
      ) : surface === 'mobile' ? (
        <FinancialMobile
          company={company}
          statements={financialStatements}
          ratios={financialRatios}
        />
      ) : (
        <FinancialExport
          company={company}
          statements={financialStatements}
          ratios={financialRatios}
        />
      )}
    </ThemeProvider>
  </QueryClientProvider>
);
