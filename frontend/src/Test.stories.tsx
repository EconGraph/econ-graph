import type { Meta, StoryObj } from '@storybook/react';

const Test = () => <div>Hello World</div>;

const meta: Meta<typeof Test> = {
  title: 'Test/Test',
  component: Test,
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
