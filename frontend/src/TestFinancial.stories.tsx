import type { Meta, StoryObj } from '@storybook/react';
import { Card, CardContent, CardHeader, Typography, Box } from '@mui/material';

const TestFinancial = ({ companyName }: { companyName: string }) => {
  console.log('🔧 TestFinancial rendering with:', companyName);
  return (
    <Box sx={{ p: 3 }}>
      <Card>
        <CardHeader title="Test Financial Component" />
        <CardContent>
          <Typography variant="h6">Company: {companyName}</Typography>
          <Typography variant="body1">This is a simple test component to verify Material-UI rendering.</Typography>
        </CardContent>
      </Card>
    </Box>
  );
};

const meta: Meta<typeof TestFinancial> = {
  title: 'Test/TestFinancial',
  component: TestFinancial,
  argTypes: {
    companyName: { control: 'text' },
  },
};

export default meta;
type Story = StoryObj<typeof TestFinancial>;

export const Default: Story = {
  args: {
    companyName: 'Apple Inc.',
  },
};
