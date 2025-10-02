import type { Meta, StoryObj } from '@storybook/react';
import { FinancialExport } from './FinancialExport';

const meta: Meta<typeof FinancialExport> = {
  title: 'Financial/FinancialExport',
  component: FinancialExport,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    companyId: {
      control: 'text',
      description: 'The company ID for export',
      defaultValue: '0000320193',
    },
    format: {
      control: 'select',
      options: ['pdf', 'excel', 'csv', 'json'],
      description: 'The export format',
    },
    includeCharts: {
      control: 'boolean',
      description: 'Whether to include charts in the export',
    },
    includeRatios: {
      control: 'boolean',
      description: 'Whether to include financial ratios',
    },
    includeStatements: {
      control: 'boolean',
      description: 'Whether to include financial statements',
    },
    userType: {
      control: 'select',
      options: ['beginner', 'intermediate', 'advanced', 'expert'],
      description: 'The type of user using the export',
    },
  },
};

export default meta;
type Story = StoryObj<typeof FinancialExport>;

export const Default: Story = {
  args: {
    companyId: '0000320193',
    format: 'pdf',
    includeCharts: true,
    includeRatios: true,
    includeStatements: true,
    userType: 'intermediate',
  },
};

export const ExcelExport: Story = {
  args: {
    companyId: '0000320193',
    format: 'excel',
    includeCharts: false,
    includeRatios: true,
    includeStatements: true,
    userType: 'advanced',
  },
};

export const CSVExport: Story = {
  args: {
    companyId: '0000320193',
    format: 'csv',
    includeCharts: false,
    includeRatios: true,
    includeStatements: false,
    userType: 'expert',
  },
};

export const JSONExport: Story = {
  args: {
    companyId: '0000320193',
    format: 'json',
    includeCharts: false,
    includeRatios: true,
    includeStatements: true,
    userType: 'advanced',
  },
};

export const BeginnerUser: Story = {
  args: {
    companyId: '0000320193',
    format: 'pdf',
    includeCharts: true,
    includeRatios: true,
    includeStatements: true,
    userType: 'beginner',
  },
};
