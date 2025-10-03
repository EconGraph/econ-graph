import type { Meta, StoryObj } from '@storybook/react';
import { BenchmarkComparison } from './BenchmarkComparison';

const meta: Meta<typeof BenchmarkComparison> = {
  title: 'Financial/BenchmarkComparison',
  component: BenchmarkComparison,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    ratioName: {
      control: 'text',
      description: 'Name of the financial ratio being compared',
    },
    companyValue: {
      control: 'number',
      description: 'Company value for the ratio',
    },
    benchmarkData: {
      control: 'object',
      description: 'Benchmark data for comparison',
    },
  },
};

export default meta;
type Story = StoryObj<typeof BenchmarkComparison>;

// Mock data comes from GraphQL response files via MSW
// The component will receive data from the MSW handlers

export const Default: Story = {
  args: {
    ratioName: 'Return on Equity',
    companyValue: 0.147,
    benchmarkData: {
      percentile: 75,
      industryMedian: 0.12,
      industryP25: 0.08,
      industryP75: 0.16,
      industryP90: 0.2,
      industryP10: 0.05,
    },
  },
};

export const HighPerformance: Story = {
  args: {
    ratioName: 'Net Profit Margin',
    companyValue: 0.253,
    benchmarkData: {
      percentile: 85,
      industryMedian: 0.15,
      industryP25: 0.1,
      industryP75: 0.2,
      industryP90: 0.25,
      industryP10: 0.08,
    },
  },
};

export const LowPerformance: Story = {
  args: {
    ratioName: 'Current Ratio',
    companyValue: 1.04,
    benchmarkData: {
      percentile: 30,
      industryMedian: 1.5,
      industryP25: 1.2,
      industryP75: 1.8,
      industryP90: 2.2,
      industryP10: 1.0,
    },
  },
};
