/**
 * Clears cached query results when the signed-in user changes (sign-in, sign-out, a failed
 * renewal), so data fetched with one user's token is never shown to another or to anonymous.
 */

import { useEffect, useRef } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { useAuth } from '../contexts/AuthContext';

/**
 * Renders nothing; resets the query cache after the first settled user changes.
 * @returns Null.
 */
const ResetQueriesOnUserChange = (): null => {
  const { user, isLoading } = useAuth();
  const queryClient = useQueryClient();
  const userId = isLoading ? undefined : (user?.id ?? null);
  const previous = useRef<string | null | undefined>(undefined);

  useEffect(() => {
    if (userId === undefined) {
      return;
    }
    if (previous.current !== undefined && previous.current !== userId) {
      void queryClient.resetQueries();
    }
    previous.current = userId;
  }, [userId, queryClient]);

  return null;
};

export default ResetQueriesOnUserChange;
