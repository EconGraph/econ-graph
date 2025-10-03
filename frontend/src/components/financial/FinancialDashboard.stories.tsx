import type { Meta, StoryObj } from '@storybook/react';
import { FinancialDashboard } from './FinancialDashboard';

const meta: Meta<typeof FinancialDashboard> = {
  title: 'Financial/FinancialDashboard',
  component: FinancialDashboard,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    companyId: {
      control: 'text',
      description: 'The company ID to display financial data for (Apple Inc. = 0000320193)',
      defaultValue: '0000320193',
    },
    userType: {
      control: 'select',
      options: ['beginner', 'intermediate', 'advanced', 'expert'],
      description: 'The type of user viewing the dashboard',
    },
    showEducationalContent: {
      control: 'boolean',
      description: 'Whether to display educational content',
    },
    showCollaborativeFeatures: {
      control: 'boolean',
      description: 'Whether to show collaborative features',
    },
  },
};

export default meta;
type Story = StoryObj<typeof FinancialDashboard>;

export const Default: Story = {
  args: {
    companyId: '0000320193', // Apple Inc. - matches mock data
    userType: 'intermediate',
    showEducationalContent: true,
    showCollaborativeFeatures: true,
  },
};

export const BeginnerUser: Story = {
  args: {
    companyId: '0000320193', // Apple Inc. - matches mock data
    userType: 'beginner',
    showEducationalContent: true,
    showCollaborativeFeatures: false,
  },
};

export const AdvancedUser: Story = {
  args: {
    companyId: '0000320193', // Apple Inc. - matches mock data
    userType: 'advanced',
    showEducationalContent: false,
    showCollaborativeFeatures: true,
  },
};
