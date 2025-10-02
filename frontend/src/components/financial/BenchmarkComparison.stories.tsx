import type { Meta, StoryObj } from '@storybook/react';
import { BenchmarkComparison } from './BenchmarkComparison';

const meta: Meta<typeof BenchmarkComparison> = {
  title: 'Financial/BenchmarkComparison',
  component: BenchmarkComparison,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    ratios: {
      control: 'object',
      description: 'Array of financial ratios for comparison',
    },
    company: {
      control: 'object',
      description: 'Company information for comparison',
    },
    userType: {
      control: 'select',
      options: ['beginner', 'intermediate', 'advanced', 'expert'],
      description: 'The type of user viewing the comparison',
    },
  },
};

export default meta;
type Story = StoryObj<typeof BenchmarkComparison>;

// Mock data for benchmark comparison
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
  },
  {
    id: 'ratio-3',
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
    dataQualityScore: 0.98
  }
];

const mockCompany = {
  id: '0000320193',
  name: 'Apple Inc.',
  ticker: 'AAPL',
  industry: 'Technology Hardware & Equipment',
  sector: 'Technology'
};

export const Default: Story = {
  args: {
    ratios: mockRatios,
    company: mockCompany,
    userType: 'intermediate',
  },
};

export const BeginnerUser: Story = {
  args: {
    ratios: mockRatios,
    company: mockCompany,
    userType: 'beginner',
  },
};

export const AdvancedUser: Story = {
  args: {
    ratios: mockRatios,
    company: mockCompany,
    userType: 'advanced',
  },
};

export const ExpertUser: Story = {
  args: {
    ratios: mockRatios,
    company: mockCompany,
    userType: 'expert',
  },
};
