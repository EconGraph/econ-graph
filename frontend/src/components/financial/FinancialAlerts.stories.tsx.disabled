import type { Meta, StoryObj } from '@storybook/react';
import { FinancialAlerts } from './FinancialAlerts';

const meta: Meta<typeof FinancialAlerts> = {
  title: 'Financial/FinancialAlerts',
  component: FinancialAlerts,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    companyId: {
      control: 'text',
      description: 'The company ID for alerts',
      defaultValue: '0000320193',
    },
    userType: {
      control: 'select',
      options: ['beginner', 'intermediate', 'advanced', 'expert'],
      description: 'The type of user viewing the alerts',
    },
  },
};

export default meta;
type Story = StoryObj<typeof FinancialAlerts>;

export const Default: Story = {
  args: {
    companyId: '0000320193',
    userType: 'intermediate',
  },
};

export const BeginnerUser: Story = {
  args: {
    companyId: '0000320193',
    userType: 'beginner',
  },
};

export const AdvancedUser: Story = {
  args: {
    companyId: '0000320193',
    userType: 'advanced',
  },
};

export const ExpertUser: Story = {
  args: {
    companyId: '0000320193',
    userType: 'expert',
  },
};
