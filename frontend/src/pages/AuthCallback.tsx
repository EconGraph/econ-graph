/**
 * REQUIREMENT: Sign-in through Keycloak (OpenID Connect).
 * PURPOSE: Finishes the authorization code redirect, then returns to the page sign-in began on.
 */

import React, { useEffect, useState } from 'react';
import { Alert, Box, Button, CircularProgress, Typography } from '@mui/material';
import { useNavigate } from 'react-router-dom';

import { useAuth } from '../contexts/AuthContext';

/**
 * Callback route for the identity provider's redirect.
 * @returns A progress indicator, or the error with a way home.
 */
const AuthCallback: React.FC = () => {
  const { completeSignIn } = useAuth();
  const navigate = useNavigate();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    completeSignIn()
      .then(returnTo => {
        if (active) navigate(returnTo, { replace: true });
      })
      .catch(err => {
        if (active) setError(err instanceof Error && err.message ? err.message : 'Sign-in failed');
      });
    return () => {
      active = false;
    };
  }, [completeSignIn, navigate]);

  if (error) {
    return (
      <Box sx={{ maxWidth: 480, mx: 'auto', mt: 6 }}>
        <Alert severity='error' sx={{ mb: 2 }}>
          Sign-in did not complete: {error}
        </Alert>
        <Button variant='contained' onClick={() => navigate('/', { replace: true })}>
          Back to the dashboard
        </Button>
      </Box>
    );
  }

  return (
    <Box sx={{ display: 'flex', alignItems: 'center', gap: 2, justifyContent: 'center', mt: 6 }}>
      <CircularProgress size={24} />
      <Typography>Signing you in…</Typography>
    </Box>
  );
};

export default AuthCallback;
