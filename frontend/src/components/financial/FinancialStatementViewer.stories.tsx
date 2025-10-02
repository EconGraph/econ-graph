import type { Meta, StoryObj } from '@storybook/react';
import { FinancialStatementViewer } from './FinancialStatementViewer';

const meta: Meta<typeof FinancialStatementViewer> = {
  title: 'Financial/FinancialStatementViewer',
  component: FinancialStatementViewer,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    statementId: {
      control: 'text',
      description: 'The financial statement ID to display',
      defaultValue: 'statement-1',
    },
    companyId: {
      control: 'text',
      description: 'The company ID for the statement (Apple Inc.)',
      defaultValue: '0000320193',
    },
    userType: {
      control: 'select',
      options: ['beginner', 'intermediate', 'advanced', 'expert'],
      description: 'The type of user viewing the statement',
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
type Story = StoryObj<typeof FinancialStatementViewer>;

export const Default: Story = {
  args: {
    statementId: 'statement-1',
    companyId: '0000320193',
    userType: 'intermediate',
    showEducationalContent: true,
    showCollaborativeFeatures: true,
  },
};

export const BeginnerUser: Story = {
  args: {
    statementId: 'statement-1',
    companyId: '0000320193',
    userType: 'beginner',
    showEducationalContent: true,
    showCollaborativeFeatures: false,
  },
};

export const AdvancedUser: Story = {
  args: {
    statementId: 'statement-1',
    companyId: '0000320193',
    userType: 'advanced',
    showEducationalContent: false,
    showCollaborativeFeatures: true,
  },
};

export const ExpertUser: Story = {
  args: {
    statementId: 'statement-1',
    companyId: '0000320193',
    userType: 'expert',
    showEducationalContent: false,
    showCollaborativeFeatures: true,
  },
};
