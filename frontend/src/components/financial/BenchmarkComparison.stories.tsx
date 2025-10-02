import type { Meta, StoryObj } from '@storybook/react';
import { BenchmarkComparison } from './BenchmarkComparison';

const meta: Meta<typeof BenchmarkComparison> = {
  title: 'Financial/BenchmarkComparison',
  component: BenchmarkComparison,
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
  },
};

export default meta;
type Story = StoryObj<typeof BenchmarkComparison>;

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
