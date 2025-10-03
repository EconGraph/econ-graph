import React from 'react';
import { Card, CardContent, CardHeader, Typography, Box } from '@mui/material';

interface FinancialDashboardSimpleProps {
  companyId: string;
  userType?: 'beginner' | 'intermediate' | 'advanced' | 'expert';
  showEducationalContent?: boolean;
  showCollaborativeFeatures?: boolean;
}

export const FinancialDashboardSimple: React.FC<FinancialDashboardSimpleProps> = ({
  companyId,
  userType = 'intermediate',
  showEducationalContent = true,
  showCollaborativeFeatures = true,
}) => {
  return (
    <Box sx={{ p: 3 }}>
      <Card>
        <CardHeader title='Financial Dashboard' />
        <CardContent>
          <Typography variant='h6' gutterBottom>
            Company ID: {companyId}
          </Typography>
          <Typography variant='body1' gutterBottom>
            User Type: {userType}
          </Typography>
          <Typography variant='body1' gutterBottom>
            Educational Content: {showEducationalContent ? 'Enabled' : 'Disabled'}
          </Typography>
          <Typography variant='body1'>
            Collaborative Features: {showCollaborativeFeatures ? 'Enabled' : 'Disabled'}
          </Typography>
        </CardContent>
      </Card>
    </Box>
  );
};
