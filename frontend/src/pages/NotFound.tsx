import React from 'react';
import { Box, Button, Typography } from '@mui/material';
import { Link as RouterLink } from 'react-router-dom';

/**
 * REQUIREMENT: Unknown URLs show a clear message instead of an empty page.
 * PURPOSE: Catch-all route for paths the app does not serve, such as the removed /analysis page.
 * @returns JSX element representing the not-found page.
 */
const NotFound: React.FC = () => {
  return (
    <Box sx={{ textAlign: 'center', py: 8 }}>
      <Typography variant='h4' component='h1' gutterBottom>
        Page not found
      </Typography>
      <Typography color='text.secondary' sx={{ mb: 3 }}>
        The page you are looking for does not exist.
      </Typography>
      <Button component={RouterLink} to='/' variant='contained'>
        Go to the dashboard
      </Button>
    </Box>
  );
};

export default NotFound;
