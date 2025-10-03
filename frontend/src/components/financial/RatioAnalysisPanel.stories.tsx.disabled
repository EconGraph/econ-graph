import type { Meta, StoryObj } from '@storybook/react';
import { RatioAnalysisPanel } from './RatioAnalysisPanel';

const meta: Meta<typeof RatioAnalysisPanel> = {
  title: 'Financial/RatioAnalysisPanel',
  component: RatioAnalysisPanel,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    statementId: {
      control: 'text',
      description: 'The financial statement ID for ratio analysis',
      defaultValue: 'statement-1',
    },
    userType: {
      control: 'select',
      options: ['beginner', 'intermediate', 'advanced', 'expert'],
      description: 'The type of user viewing the ratios',
    },
    showEducationalContent: {
      control: 'boolean',
      description: 'Whether to display educational content',
    },
  },
};

export default meta;
type Story = StoryObj<typeof RatioAnalysisPanel>;

export const Default: Story = {
  args: {
    statementId: 'statement-1',
    userType: 'intermediate',
    showEducationalContent: true,
  },
};

export const BeginnerUser: Story = {
  args: {
    statementId: 'statement-1',
    userType: 'beginner',
    showEducationalContent: true,
  },
};

export const AdvancedUser: Story = {
  args: {
    statementId: 'statement-1',
    userType: 'advanced',
    showEducationalContent: false,
  },
};

export const ExpertUser: Story = {
  args: {
    statementId: 'statement-1',
    userType: 'expert',
    showEducationalContent: false,
  },
};
