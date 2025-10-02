import type { Meta, StoryObj } from '@storybook/react';
import { PeerComparisonChart } from './PeerComparisonChart';

const meta: Meta<typeof PeerComparisonChart> = {
  title: 'Financial/PeerComparisonChart',
  component: PeerComparisonChart,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    companyId: {
      control: 'text',
      description: 'The company ID for comparison',
      defaultValue: '0000320193',
    },
    peerIds: {
      control: 'object',
      description: 'Array of peer company IDs for comparison',
    },
    metrics: {
      control: 'object',
      description: 'Array of metrics to compare',
    },
    userType: {
      control: 'select',
      options: ['beginner', 'intermediate', 'advanced', 'expert'],
      description: 'The type of user viewing the comparison',
    },
  },
};

export default meta;
type Story = StoryObj<typeof PeerComparisonChart>;

// Mock data for peer comparison - using actual peer companies from mock data
const mockPeerIds = ['0000789019', '0001018724', '0001652044']; // Microsoft, Amazon, Alphabet
const mockMetrics = [
  'returnOnEquity',
  'netProfitMargin',
  'currentRatio',
  'debtToEquity'
];

export const Default: Story = {
  args: {
    companyId: '0000320193',
    peerIds: mockPeerIds,
    metrics: mockMetrics,
    userType: 'intermediate',
  },
};

export const BeginnerUser: Story = {
  args: {
    companyId: '0000320193',
    peerIds: mockPeerIds,
    metrics: mockMetrics,
    userType: 'beginner',
  },
};

export const AdvancedUser: Story = {
  args: {
    companyId: '0000320193',
    peerIds: mockPeerIds,
    metrics: mockMetrics,
    userType: 'advanced',
  },
};

export const ExpertUser: Story = {
  args: {
    companyId: '0000320193',
    peerIds: mockPeerIds,
    metrics: mockMetrics,
    userType: 'expert',
  },
};

export const LimitedMetrics: Story = {
  args: {
    companyId: '0000320193',
    peerIds: mockPeerIds,
    metrics: ['returnOnEquity', 'netProfitMargin'],
    userType: 'intermediate',
  },
};
