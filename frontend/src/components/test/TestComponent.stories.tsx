import type { Meta, StoryObj } from '@storybook/react';
import React from 'react';

const TestComponent = () => {
  return <div>Hello from Test Component</div>;
};

const meta: Meta<typeof TestComponent> = {
  title: 'Test/TestComponent',
  component: TestComponent,
};

export default meta;
type Story = StoryObj<typeof TestComponent>;

export const Default: Story = {};
