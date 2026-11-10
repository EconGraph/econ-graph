import React from 'react';
import { Link as RouterLink } from 'react-router-dom';
import { Box, Link, Typography } from '@mui/material';

/**
 * The footer every page shares, with the Privacy Policy and Terms of Use links (ECO-881). It sits
 * outside `main`, so it is the page's contentinfo landmark.
 * @returns The site footer.
 */
const SiteFooter: React.FC = () => (
  <Box
    component='footer'
    sx={{ py: 2, px: 3, borderTop: 1, borderColor: 'divider', textAlign: 'center' }}
  >
    <Typography variant='body2' sx={{ color: 'text.secondary' }}>
      <Link component={RouterLink} to='/privacy'>
        Privacy Policy
      </Link>
      {' · '}
      <Link component={RouterLink} to='/terms'>
        Terms of Use
      </Link>
    </Typography>
  </Box>
);

export default SiteFooter;
