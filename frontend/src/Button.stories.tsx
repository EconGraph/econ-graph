import type { Meta, StoryObj } from '@storybook/react';

const Button = ({ children }: { children: string }) => <button>{children}</button>;

const meta: Meta<typeof Button> = {
  title: 'Example/Button',
  component: Button,
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Primary: Story = {
  args: {
    children: 'Button',
  },
};
