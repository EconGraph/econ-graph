import type { Meta, StoryObj } from '@storybook/react';
import { FinancialMobile } from './FinancialMobile';

const meta: Meta<typeof FinancialMobile> = {
  title: 'Financial/FinancialMobile',
  component: FinancialMobile,
  parameters: {
    layout: 'fullscreen',
    viewport: {
      defaultViewport: 'mobile1',
    },
  },
  argTypes: {
    companyId: {
      control: 'text',
      description: 'The company ID for mobile view',
      defaultValue: '0000320193',
    },
    userType: {
      control: 'select',
      options: ['beginner', 'intermediate', 'advanced', 'expert'],
      description: 'The type of user viewing on mobile',
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
type Story = StoryObj<typeof FinancialMobile>;

export const Default: Story = {
  args: {
    companyId: '0000320193',
    userType: 'intermediate',
    showEducationalContent: true,
    showCollaborativeFeatures: true,
  },
};

export const BeginnerUser: Story = {
  args: {
    companyId: '0000320193',
    userType: 'beginner',
    showEducationalContent: true,
    showCollaborativeFeatures: false,
  },
};

export const AdvancedUser: Story = {
  args: {
    companyId: '0000320193',
    userType: 'advanced',
    showEducationalContent: false,
    showCollaborativeFeatures: true,
  },
};

export const ExpertUser: Story = {
  args: {
    companyId: '0000320193',
    userType: 'expert',
    showEducationalContent: false,
    showCollaborativeFeatures: true,
  },
};

export const TabletView: Story = {
  args: {
    companyId: '0000320193',
    userType: 'intermediate',
    showEducationalContent: true,
    showCollaborativeFeatures: true,
  },
  parameters: {
    viewport: {
      defaultViewport: 'tablet',
    },
  },
};
