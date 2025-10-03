import type { Meta, StoryObj } from '@storybook/react';

console.log('🔧 Simple.stories.tsx: File loaded successfully');

const Simple = ({ text }: { text: string }) => {
  console.log('🔧 Simple component rendering with text:', text);
  return (
    <div style={{ padding: '20px', border: '1px solid #ccc' }}>
      <h2>Simple Component</h2>
      <p>{text}</p>
    </div>
  );
};

console.log('🔧 Simple.stories.tsx: Creating meta object');

const meta: Meta<typeof Simple> = {
  title: 'Test/Simple',
  component: Simple,
  argTypes: {
    text: { control: 'text' },
  },
};

console.log('🔧 Simple.stories.tsx: Meta object created:', meta);

export default meta;
type Story = StoryObj<typeof meta>;

console.log('🔧 Simple.stories.tsx: Creating Default story');

export const Default: Story = {
  args: {
    text: 'This is a simple component that should work.',
  },
};

console.log('🔧 Simple.stories.tsx: Default story created');
