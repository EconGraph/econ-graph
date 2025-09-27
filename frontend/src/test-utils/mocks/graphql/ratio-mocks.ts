// MSW handlers for ratio analysis GraphQL queries
// These handlers provide realistic mock data for ratio analysis components

import { graphql, HttpResponse } from 'msw';
// GraphQL queries are now loaded dynamically from JSON files
// import { GET_FINANCIAL_RATIOS, GET_RATIO_BENCHMARKS, GET_RATIO_EXPLANATION } from './ratio-queries';

// Mock ratio data with comprehensive financial metrics
const mockFinancialRatios = [
  {
    id: 'ratio-1',
    statementId: 'statement-1',
    ratioName: 'returnOnEquity',
    ratioDisplayName: 'Return on Equity (ROE)',
    value: 0.147,
    category: 'profitability',
    formula: 'Net Income / Shareholders Equity',
    interpretation: 'Strong profitability, above industry average',
    benchmarkPercentile: 75,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.95,
  },
  {
    id: 'ratio-2',
    statementId: 'statement-1',
    ratioName: 'currentRatio',
    ratioDisplayName: 'Current Ratio',
    value: 1.04,
    category: 'liquidity',
    formula: 'Current Assets / Current Liabilities',
    interpretation: 'Adequate liquidity position',
    benchmarkPercentile: 45,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.98,
  },
  {
    id: 'ratio-3',
    statementId: 'statement-1',
    ratioName: 'debtToEquity',
    ratioDisplayName: 'Debt-to-Equity Ratio',
    value: 0.85,
    category: 'leverage',
    formula: 'Total Debt / Total Equity',
    interpretation: 'Moderate leverage, manageable debt levels',
    benchmarkPercentile: 60,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.92,
  },
  {
    id: 'ratio-4',
    statementId: 'statement-1',
    ratioName: 'enterpriseValueToEbitda',
    ratioDisplayName: 'Enterprise Value to EBITDA (EV/EBITDA)',
    value: 18.5,
    category: 'valuation',
    formula: 'Enterprise Value / EBITDA',
    interpretation: 'Reasonable valuation multiple',
    benchmarkPercentile: 55,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.88,
  },
  {
    id: 'ratio-5',
    statementId: 'statement-1',
    ratioName: 'freeCashFlow',
    ratioDisplayName: 'Free Cash Flow',
    value: 99803000000,
    category: 'cashFlow',
    formula: 'Operating Cash Flow - Capital Expenditures',
    interpretation: 'Strong cash generation ability',
    benchmarkPercentile: 80,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.97,
  },
  {
    id: 'ratio-6',
    statementId: 'statement-1',
    ratioName: 'freeCashFlowYield',
    ratioDisplayName: 'Free Cash Flow Yield',
    value: 0.052,
    category: 'cashFlow',
    formula: 'Free Cash Flow / Market Capitalization',
    interpretation: 'Attractive yield for investors',
    benchmarkPercentile: 70,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.94,
  },
  {
    id: 'ratio-7',
    statementId: 'statement-1',
    ratioName: 'revenueGrowthRate',
    ratioDisplayName: 'Revenue Growth Rate',
    value: 0.08,
    category: 'growth',
    formula: '(Current Revenue - Previous Revenue) / Previous Revenue',
    interpretation: 'Healthy revenue growth',
    benchmarkPercentile: 65,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.91,
  },
  {
    id: 'ratio-8',
    statementId: 'statement-1',
    ratioName: 'returnOnInvestedCapital',
    ratioDisplayName: 'Return on Invested Capital (ROIC)',
    value: 0.162,
    category: 'profitability',
    formula: 'Net Operating Profit After Tax / Invested Capital',
    interpretation: 'Excellent capital efficiency',
    benchmarkPercentile: 85,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.96,
  },
];

const mockRatioBenchmarks = {
  returnOnEquity: {
    ratioName: 'returnOnEquity',
    industry: 'Technology',
    percentile25: 0.08,
    percentile50: 0.12,
    percentile75: 0.18,
    percentile90: 0.25,
    sampleSize: 1250,
    lastUpdated: '2023-12-31T00:00:00Z',
  },
  currentRatio: {
    ratioName: 'currentRatio',
    industry: 'Technology',
    percentile25: 1.2,
    percentile50: 1.8,
    percentile75: 2.5,
    percentile90: 3.2,
    sampleSize: 1250,
    lastUpdated: '2023-12-31T00:00:00Z',
  },
  debtToEquity: {
    ratioName: 'debtToEquity',
    industry: 'Technology',
    percentile25: 0.3,
    percentile50: 0.6,
    percentile75: 1.0,
    percentile90: 1.5,
    sampleSize: 1250,
    lastUpdated: '2023-12-31T00:00:00Z',
  },
  enterpriseValueToEbitda: {
    ratioName: 'enterpriseValueToEbitda',
    industry: 'Technology',
    percentile25: 12.0,
    percentile50: 18.0,
    percentile75: 28.0,
    percentile90: 40.0,
    sampleSize: 1250,
    lastUpdated: '2023-12-31T00:00:00Z',
  },
};

const mockRatioExplanations = {
  returnOnEquity: {
    ratioName: 'returnOnEquity',
    displayName: 'Return on Equity (ROE)',
    formula: 'Net Income / Shareholders Equity',
    description:
      "Measures how efficiently a company uses shareholders' equity to generate profits.",
    interpretation:
      'Higher ROE indicates better profitability and efficient use of equity capital.',
    category: 'profitability',
    importance: 'high',
    warrenBuffettFavorite: true,
    analystPreferred: true,
    calculationMethod: "Annual net income divided by average shareholders' equity",
    limitations: 'Can be manipulated through share buybacks and debt financing',
    relatedRatios: ['returnOnAssets', 'returnOnInvestedCapital', 'equityMultiplier'],
  },
  currentRatio: {
    ratioName: 'currentRatio',
    displayName: 'Current Ratio',
    formula: 'Current Assets / Current Liabilities',
    description:
      "Measures a company's ability to pay short-term obligations with short-term assets.",
    interpretation: 'A ratio above 1.0 indicates the company can cover its short-term liabilities.',
    category: 'liquidity',
    importance: 'high',
    warrenBuffettFavorite: false,
    analystPreferred: true,
    calculationMethod: 'Total current assets divided by total current liabilities',
    limitations: 'Does not account for the quality and liquidity of current assets',
    relatedRatios: ['quickRatio', 'cashRatio', 'operatingCashFlowRatio'],
  },
  freeCashFlow: {
    ratioName: 'freeCashFlow',
    displayName: 'Free Cash Flow',
    formula: 'Operating Cash Flow - Capital Expenditures',
    description: 'The cash a company generates after accounting for capital expenditures.',
    interpretation:
      'Positive free cash flow indicates the company is generating more cash than it spends.',
    category: 'cashFlow',
    importance: 'very_high',
    warrenBuffettFavorite: true,
    analystPreferred: true,
    calculationMethod: 'Operating cash flow minus capital expenditures',
    limitations: 'Can be volatile and affected by one-time items',
    relatedRatios: ['freeCashFlowYield', 'freeCashFlowPerShare', 'cashFlowReturnOnInvestment'],
  },
};

export const ratioHandlers = [
  graphql.query('GetFinancialRatios', ({ variables }) => {
    const { statementId } = variables as { statementId: string };

    if (statementId === 'statement-1') {
      return HttpResponse.json({
        data: {
          financialRatios: mockFinancialRatios,
        },
      });
    }
    return HttpResponse.json({
      data: {
        financialRatios: [],
      },
      errors: [{ message: 'Statement not found' }],
    });
  }),

  graphql.query('GetRatioBenchmarks', ({ variables }) => {
    const { ratioName, industry: _industry } = variables as {
      ratioName: string;
      industry?: string;
    };

    const benchmark = mockRatioBenchmarks[ratioName as keyof typeof mockRatioBenchmarks];
    if (benchmark) {
      return HttpResponse.json({
        data: {
          ratioBenchmarks: benchmark,
        },
      });
    }
    return HttpResponse.json({
      data: {
        ratioBenchmarks: null,
      },
      errors: [{ message: 'Benchmark data not found' }],
    });
  }),

  graphql.query('GetRatioExplanation', ({ variables }) => {
    const { ratioName } = variables as { ratioName: string };

    const explanation = mockRatioExplanations[ratioName as keyof typeof mockRatioExplanations];
    if (explanation) {
      return HttpResponse.json({
        data: {
          ratioExplanation: explanation,
        },
      });
    }
    return HttpResponse.json({
      data: {
        ratioExplanation: null,
      },
      errors: [{ message: 'Explanation not found' }],
    });
  }),
];
