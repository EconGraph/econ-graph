import type { Meta, StoryObj } from '@storybook/react';
import React from 'react';

const SimpleTest = () => {
  return (
    <div style={{ padding: '20px', border: '1px solid #ccc', margin: '20px' }}>
      <h1>Simple Test Component</h1>
      <p>This is a test component to verify Storybook is working.</p>
    </div>
  );
};

const meta: Meta<typeof SimpleTest> = {
  title: 'Test/SimpleTest',
  component: SimpleTest,
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
