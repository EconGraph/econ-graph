import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';

/**
 * Keeps the canonical URL in step with client-side navigation. The gateway redirects
 * alternate public names before serving the app, so the page origin is the configured
 * main origin, including after an operator rolls back to the apex host.
 * @returns Null.
 */
const CanonicalLink = (): null => {
  const { pathname } = useLocation();

  useEffect(() => {
    const link = document.createElement('link');
    link.rel = 'canonical';
    // Search parameters include filters and OIDC callback state, neither of which
    // belongs in the canonical URL. Hash fragments are also omitted.
    link.href = `${window.location.origin}${pathname}`;
    document.head.appendChild(link);
    return () => link.remove();
  }, [pathname]);

  return null;
};

export default CanonicalLink;
