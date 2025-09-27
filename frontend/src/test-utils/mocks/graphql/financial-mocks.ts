// MSW handlers for financial component GraphQL queries
// These handlers provide realistic mock data for financial components

import { graphql, HttpResponse } from 'msw';
import {
  GET_FINANCIAL_DASHBOARD,
  GET_FINANCIAL_STATEMENT,
  GET_TREND_ANALYSIS,
  GET_PEER_COMPARISON,
  GET_BENCHMARK_COMPARISON,
  GET_FINANCIAL_ALERTS,
  GET_FINANCIAL_EXPORT,
} from './financial-queries';

// Mock financial data
const mockCompany = {
  id: '0000320193',
  name: 'Apple Inc.',
  ticker: 'AAPL',
  industry: 'Computer Hardware',
  sector: 'Technology',
};

const mockFinancialStatements = [
  {
    id: 'statement-1',
    type: 'INCOME_STATEMENT',
    period: '2024-Q3',
    data: {
      revenue: 89498000000,
      grossProfit: 40417000000,
      operatingIncome: 24133000000,
      netIncome: 22956000000,
    },
    lineItems: [
      {
        id: 'revenue',
        name: 'Total Revenue',
        value: 89498000000,
        category: 'REVENUE',
      },
      {
        id: 'gross-profit',
        name: 'Gross Profit',
        value: 40417000000,
        category: 'PROFIT',
      },
      {
        id: 'operating-income',
        name: 'Operating Income',
        value: 24133000000,
        category: 'PROFIT',
      },
      {
        id: 'net-income',
        name: 'Net Income',
        value: 22956000000,
        category: 'PROFIT',
      },
    ],
  },
  {
    id: 'statement-2',
    type: 'BALANCE_SHEET',
    period: '2024-Q3',
    data: {
      totalAssets: 352755000000,
      totalLiabilities: 258549000000,
      totalEquity: 94206000000,
    },
    lineItems: [
      {
        id: 'total-assets',
        name: 'Total Assets',
        value: 352755000000,
        category: 'ASSETS',
      },
      {
        id: 'total-liabilities',
        name: 'Total Liabilities',
        value: 258549000000,
        category: 'LIABILITIES',
      },
      {
        id: 'total-equity',
        name: 'Total Equity',
        value: 94206000000,
        category: 'EQUITY',
      },
    ],
  },
];

const mockPeers = [
  {
    id: '0000789019',
    name: 'Microsoft Corporation',
    ticker: 'MSFT',
    industry: 'Software',
    sector: 'Technology',
  },
  {
    id: '0001018724',
    name: 'Amazon.com Inc.',
    ticker: 'AMZN',
    industry: 'E-commerce',
    sector: 'Consumer Discretionary',
  },
];

const mockBenchmarks = [
  {
    id: 'sp500',
    name: 'S&P 500',
    type: 'INDEX',
    value: 4500.0,
  },
  {
    id: 'nasdaq',
    name: 'NASDAQ',
    type: 'INDEX',
    value: 14000.0,
  },
];

const mockAlerts = [
  {
    id: 'alert-1',
    type: 'REVENUE_GROWTH',
    severity: 'HIGH',
    message: 'Revenue growth has slowed significantly',
    createdAt: '2024-01-15T10:00:00Z',
    resolved: false,
  },
  {
    id: 'alert-2',
    type: 'PROFIT_MARGIN',
    severity: 'MEDIUM',
    message: 'Profit margins are below industry average',
    createdAt: '2024-01-14T15:30:00Z',
    resolved: false,
  },
];

export const financialHandlers = [
  graphql.query('GetFinancialDashboard', ({ variables }) => {
    const { companyId } = variables as { companyId: string };

    if (companyId === '0000320193') {
      return HttpResponse.json({
        data: {
          company: {
            ...mockCompany,
            financialStatements: mockFinancialStatements,
          },
        },
      });
    }

    return HttpResponse.json({
      data: { company: null },
      errors: [{ message: 'Company not found' }],
    });
  }),

  graphql.query('GetFinancialStatement', ({ variables }) => {
    const { statementId } = variables as { statementId: string };
    const statement = mockFinancialStatements.find(s => s.id === statementId);

    if (statement) {
      return HttpResponse.json({
        data: { financialStatement: statement },
      });
    }

    return HttpResponse.json({
      data: { financialStatement: null },
      errors: [{ message: 'Financial statement not found' }],
    });
  }),

  graphql.query('GetTrendAnalysis', ({ variables }) => {
    const { companyId, timeRange } = variables as { companyId: string; timeRange: string };

    if (companyId === '0000320193') {
      return HttpResponse.json({
        data: {
          company: {
            ...mockCompany,
            financialStatements: mockFinancialStatements,
          },
        },
      });
    }

    return HttpResponse.json({
      data: { company: null },
      errors: [{ message: 'Company not found' }],
    });
  }),

  graphql.query('GetPeerComparison', ({ variables }) => {
    const { companyId, peerIds } = variables as { companyId: string; peerIds: string[] };

    if (companyId === '0000320193') {
      return HttpResponse.json({
        data: {
          company: mockCompany,
          peers: mockPeers.filter(peer => peerIds.includes(peer.id)),
        },
      });
    }

    return HttpResponse.json({
      data: { company: null, peers: [] },
      errors: [{ message: 'Company not found' }],
    });
  }),

  graphql.query('GetBenchmarkComparison', ({ variables }) => {
    const { companyId, benchmarkType } = variables as { companyId: string; benchmarkType: string };

    if (companyId === '0000320193') {
      return HttpResponse.json({
        data: {
          company: mockCompany,
          benchmark: mockBenchmarks.find(b => b.type === benchmarkType) || mockBenchmarks[0],
        },
      });
    }

    return HttpResponse.json({
      data: { company: null, benchmark: null },
      errors: [{ message: 'Company not found' }],
    });
  }),

  graphql.query('GetFinancialAlerts', ({ variables }) => {
    const { companyId } = variables as { companyId: string };

    if (companyId === '0000320193') {
      return HttpResponse.json({
        data: {
          company: {
            ...mockCompany,
            alerts: mockAlerts,
          },
        },
      });
    }

    return HttpResponse.json({
      data: { company: null },
      errors: [{ message: 'Company not found' }],
    });
  }),

  graphql.query('GetFinancialExport', ({ variables }) => {
    const { companyId, format } = variables as { companyId: string; format: string };

    if (companyId === '0000320193') {
      return HttpResponse.json({
        data: {
          company: {
            ...mockCompany,
            financialStatements: mockFinancialStatements,
          },
        },
      });
    }

    return HttpResponse.json({
      data: { company: null },
      errors: [{ message: 'Company not found' }],
    });
  }),
];
