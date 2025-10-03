import type { Meta, StoryObj } from '@storybook/react';
import { PeerComparisonChart } from './PeerComparisonChart';

const meta: Meta<typeof PeerComparisonChart> = {
  title: 'Financial/PeerComparisonChart',
  component: PeerComparisonChart,
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
    selectedRatios: {
      control: 'object',
      description: 'Array of selected ratios to display',
    },
    onRatioSelectionChange: {
      action: 'ratioSelectionChanged',
      description: 'Callback when ratio selection changes',
    },
  },
};

export default meta;
type Story = StoryObj<typeof PeerComparisonChart>;

// Mock data comes from GraphQL response files via MSW
// The component will receive data from the MSW handlers

export const Default: Story = {
  args: {
    userType: 'intermediate',
  },
};

export const BeginnerUser: Story = {
  args: {
    userType: 'beginner',
  },
};

export const AdvancedUser: Story = {
  args: {
    userType: 'advanced',
  },
};

export const ExpertUser: Story = {
  args: {
    userType: 'expert',
  },
};
