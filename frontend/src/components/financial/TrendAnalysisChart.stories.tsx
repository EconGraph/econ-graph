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

// Mock data comes from GraphQL response files via MSW
// The component will receive data from the MSW handlers

export const Default: Story = {
  args: {
    timeRange: '3Y',
  },
};

export const OneYear: Story = {
  args: {
    timeRange: '1Y',
  },
};

export const FiveYear: Story = {
  args: {
    timeRange: '5Y',
  },
};

export const TenYear: Story = {
  args: {
    timeRange: '10Y',
  },
};
