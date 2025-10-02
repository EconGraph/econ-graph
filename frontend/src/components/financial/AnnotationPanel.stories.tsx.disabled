import type { Meta, StoryObj } from '@storybook/react';
import { AnnotationPanel } from './AnnotationPanel';

const meta: Meta<typeof AnnotationPanel> = {
  title: 'Financial/AnnotationPanel',
  component: AnnotationPanel,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    entityId: {
      control: 'text',
      description: 'The ID of the entity being annotated',
      defaultValue: '0000320193',
    },
    entityType: {
      control: 'select',
      options: ['company-analysis', 'financial-statement', 'ratio-analysis'],
      description: 'The type of entity being annotated',
    },
    userType: {
      control: 'select',
      options: ['beginner', 'intermediate', 'advanced', 'expert'],
      description: 'The type of user creating annotations',
    },
  },
};

export default meta;
type Story = StoryObj<typeof AnnotationPanel>;

export const Default: Story = {
  args: {
    entityId: '0000320193',
    entityType: 'company-analysis',
    userType: 'intermediate',
  },
};

export const FinancialStatement: Story = {
  args: {
    entityId: 'statement-1',
    entityType: 'financial-statement',
    userType: 'advanced',
  },
};

export const RatioAnalysis: Story = {
  args: {
    entityId: 'ratio-1',
    entityType: 'ratio-analysis',
    userType: 'expert',
  },
};

export const BeginnerUser: Story = {
  args: {
    entityId: '0000320193',
    entityType: 'company-analysis',
    userType: 'beginner',
  },
};

export const ExpertUser: Story = {
  args: {
    entityId: '0000320193',
    entityType: 'company-analysis',
    userType: 'expert',
  },
};
