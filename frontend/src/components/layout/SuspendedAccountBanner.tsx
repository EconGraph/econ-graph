import React from 'react';
import { Alert, AlertTitle, Button } from '@mui/material';

import { useAuth } from '../../contexts/AuthContext';

/**
 * Tells a signed-in user whose account is suspended why signed-in features stopped working.
 * Public pages keep rendering underneath: their data is read anonymously while suspended.
 * @returns The banner, or null when the account is not suspended.
 */
const SuspendedAccountBanner: React.FC = () => {
  const { isSuspended, signOut } = useAuth();
  if (!isSuspended) {
    return null;
  }
  return (
    <Alert
      severity='warning'
      sx={{ mb: 3 }}
      action={
        <Button color='inherit' size='small' onClick={() => void signOut()}>
          Sign out
        </Button>
      }
    >
      <AlertTitle>Your account is suspended</AlertTitle>
      You can still browse public data, but signed-in features are unavailable. Contact an
      administrator if you think this is a mistake.
    </Alert>
  );
};

export default SuspendedAccountBanner;
