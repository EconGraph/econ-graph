import type { Meta, StoryObj } from '@storybook/react';
import { TrendAnalysisChart } from './TrendAnalysisChart';

const meta: Meta<typeof TrendAnalysisChart> = {
  title: 'Financial/TrendAnalysisChart',
  component: TrendAnalysisChart,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    ratios: {
      control: 'object',
      description: 'Array of financial ratios for trend analysis',
    },
    statements: {
      control: 'object',
      description: 'Array of financial statements',
    },
    timeRange: {
      control: 'select',
      options: ['1Y', '3Y', '5Y', '10Y'],
      description: 'Time range for trend analysis',
    },
    onTimeRangeChange: {
      action: 'timeRangeChanged',
      description: 'Callback when time range changes',
    },
  },
};

export default meta;
type Story = StoryObj<typeof TrendAnalysisChart>;

// Mock data for trend analysis
const mockRatios = [
  {
    id: 'ratio-1',
    statementId: 'statement-1',
    ratioName: 'returnOnEquity',
    ratioDisplayName: 'Return on Equity',
    value: 0.147,
    category: 'profitability',
    formula: 'Net Income / Shareholders Equity',
    interpretation: 'Strong profitability, above industry average',
    benchmarkPercentile: 75,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.95
  },
  {
    id: 'ratio-2',
    statementId: 'statement-1',
    ratioName: 'netProfitMargin',
    ratioDisplayName: 'Net Profit Margin',
    value: 0.253,
    category: 'profitability',
    formula: 'Net Income / Revenue',
    interpretation: 'Strong profitability, above industry average',
    benchmarkPercentile: 80,
    periodEndDate: '2023-12-31',
    fiscalYear: 2023,
    fiscalQuarter: 4,
    calculatedAt: '2023-12-31T00:00:00Z',
    dataQualityScore: 0.92
  }
];

const mockStatements = [
  {
    id: 'statement-1',
    type: 'balanceSheet',
    period: '2023-12-31',
    data: {
      companyId: '0000320193',
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
      updatedAt: '2023-12-31T00:00:00Z'
    }
  }
];

export const Default: Story = {
  args: {
    ratios: mockRatios,
    statements: mockStatements,
    timeRange: '3Y',
  },
};

export const OneYear: Story = {
  args: {
    ratios: mockRatios,
    statements: mockStatements,
    timeRange: '1Y',
  },
};

export const FiveYear: Story = {
  args: {
    ratios: mockRatios,
    statements: mockStatements,
    timeRange: '5Y',
  },
};

export const TenYear: Story = {
  args: {
    ratios: mockRatios,
    statements: mockStatements,
    timeRange: '10Y',
  },
};
