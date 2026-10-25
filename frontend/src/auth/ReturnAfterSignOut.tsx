/**
 * Returns to the page sign-out began on. Keycloak sends the browser back to the home page after
 * sign-out, so a visitor who signed out on a series page would otherwise be left on the dashboard.
 */

import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';

import { useAuth } from '../contexts/AuthContext';

/**
 * Renders nothing; on load after a sign-out redirect, replaces the home page with the page
 * sign-out began on.
 * @returns Null.
 */
const ReturnAfterSignOut = (): null => {
  const { completeSignOut } = useAuth();
  const navigate = useNavigate();

  useEffect(() => {
    let active = true;
    void completeSignOut().then(returnTo => {
      if (active && returnTo) navigate(returnTo, { replace: true });
    });
    return () => {
      active = false;
    };
  }, [completeSignOut, navigate]);

  return null;
};

export default ReturnAfterSignOut;
