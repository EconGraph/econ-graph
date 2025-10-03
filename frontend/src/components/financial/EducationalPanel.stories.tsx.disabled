import type { Meta, StoryObj } from '@storybook/react';
import { EducationalPanel } from './EducationalPanel';

const meta: Meta<typeof EducationalPanel> = {
  title: 'Financial/EducationalPanel',
  component: EducationalPanel,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    title: {
      control: 'text',
      description: 'The title of the educational panel',
    },
    content: {
      control: 'text',
      description: 'The educational content to display',
    },
    level: {
      control: 'select',
      options: ['beginner', 'intermediate', 'advanced'],
      description: 'The educational level of the content',
    },
    showExamples: {
      control: 'boolean',
      description: 'Whether to show examples',
    },
  },
};

export default meta;
type Story = StoryObj<typeof EducationalPanel>;

export const Default: Story = {
  args: {
    title: 'Understanding Financial Ratios',
    content: 'Financial ratios are mathematical comparisons of financial statement accounts or categories. They help investors and analysts understand a company\'s financial health and performance.',
    level: 'beginner',
    showExamples: true,
  },
};

export const Intermediate: Story = {
  args: {
    title: 'Advanced Ratio Analysis',
    content: 'Advanced ratio analysis involves comparing ratios across time periods, against industry benchmarks, and in combination with other financial metrics to identify trends and potential issues.',
    level: 'intermediate',
    showExamples: true,
  },
};

export const Advanced: Story = {
  args: {
    title: 'Professional Financial Analysis',
    content: 'Professional financial analysis requires understanding the interrelationships between different ratios, industry-specific considerations, and the impact of accounting policies on ratio calculations.',
    level: 'advanced',
    showExamples: false,
  },
};

export const ProfitabilityRatios: Story = {
  args: {
    title: 'Profitability Ratios Explained',
    content: 'Profitability ratios measure a company\'s ability to generate earnings relative to its revenue, assets, and equity. Key ratios include Return on Equity (ROE), Net Profit Margin, and Return on Assets (ROA).',
    level: 'intermediate',
    showExamples: true,
  },
};

export const LiquidityRatios: Story = {
  args: {
    title: 'Understanding Liquidity Ratios',
    content: 'Liquidity ratios measure a company\'s ability to meet short-term obligations. The Current Ratio and Quick Ratio are the most commonly used liquidity measures.',
    level: 'beginner',
    showExamples: true,
  },
};
