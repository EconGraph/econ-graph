/**
 * REQUIREMENT: Sign-in through Keycloak (OpenID Connect).
 * PURPOSE: Hold the signed-in user and expose sign-in, sign-out and access tokens to the app.
 * Authorization code with PKCE via oidc-client-ts. Tokens stay in memory only; after a reload the
 * session of a user who was signed in is restored with a silent sign-in against the identity
 * provider's own session.
 */

import React, {
  createContext,
  useCallback,
  useContext,
  useLayoutEffect,
  useMemo,
  useState,
} from 'react';
import { ErrorResponse, type User as OidcUser, type UserManager } from 'oidc-client-ts';

import { setAccessTokenProvider } from '../auth/accessToken';
import { CALLBACK_PATH, getUserManager } from '../auth/oidcConfig';

/** The signed-in user, taken from the ID token claims. */
export interface User {
  /** The identity provider's subject (`sub`), which the backend uses as the user id. */
  id: string;
  name: string;
  email?: string;
  avatar?: string;
}

export interface AuthState {
  user: User | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
}

interface AuthContextType extends AuthState {
  /** False when the build has no OIDC issuer configured; sign-in is then unavailable. */
  isConfigured: boolean;
  /** Link to Keycloak's account console (`<issuer>/account`), when sign-in is configured. */
  accountUrl: string | null;
  /** Redirects to the identity provider, returning to `returnTo` (default: the current page). */
  signIn: (returnTo?: string) => Promise<void>;
  signOut: () => Promise<void>;
  /** Finishes the redirect on the callback route and returns the path to go back to. */
  completeSignIn: () => Promise<string>;
  clearError: () => void;
}

interface SignInState {
  returnTo?: string;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

const toUser = (oidcUser: OidcUser): User => {
  const { profile } = oidcUser;
  return {
    id: profile.sub,
    name: profile.name || profile.preferred_username || profile.email || 'Signed in',
    email: profile.email,
    avatar: profile.picture,
  };
};

/**
 * Accepts only same-origin paths, so a crafted `state` cannot redirect off site, and never the
 * callback route itself, which would leave the user on "Signing you in" forever.
 * @param value - The requested return path.
 * @returns The path, or `/` when it is not a safe same-origin path.
 */
export const safeReturnTo = (value: unknown): string => {
  if (typeof value !== 'string' || !value.startsWith('/') || value.startsWith('//')) {
    return '/';
  }
  // Browsers drop tabs and newlines and treat backslashes as slashes, so `/\t/evil` is `//evil`.
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f\\]/.test(value) || value.startsWith(CALLBACK_PATH)) {
    return '/';
  }
  return value;
};

const currentPath = () =>
  `${window.location.pathname}${window.location.search}${window.location.hash}`;

const errorMessage = (error: unknown, fallback: string) =>
  error instanceof Error && error.message ? error.message : fallback;

/** Renew a little early, so a token does not expire while a request is in flight. */
const RENEW_BEFORE_EXPIRY_SECONDS = 30;

const isExpiring = (oidcUser: OidcUser) =>
  oidcUser.expires_in !== undefined && oidcUser.expires_in < RENEW_BEFORE_EXPIRY_SECONDS;

/**
 * oidc-client-ts's own wording for "the renewed token belongs to someone else": `_validateIdTokenAttributes`
 * (dist/esm/oidc-client-ts.js in the pinned version below) compares the freshly renewed claim to
 * the one already held and throws a plain `Error` with one of these messages, not an
 * `ErrorResponse`. Matched by exact message, not by the substring "does not match" or "not in
 * id_token", which the library also throws for unrelated, transient causes (e.g. "State does not
 * match" when a stale iframe response arrives late).
 *
 * The oidc-client-ts dependency is pinned to an exact version (see package.json) so this list, and
 * `oidcConfig.test.ts`'s check that it still matches the library's source, cannot silently go
 * stale on an upgrade.
 */
export const KEYCLOAK_SESSION_CHANGED = [
  'sub in id_token does not match current sub',
  'nonce in id_token does not match nonce in client storage',
  'auth_time in id_token does not match original auth_time',
  'azp in id_token does not match original azp',
  'azp not in id_token, but present in original id_token',
];

/** Error codes Keycloak itself uses for a temporary problem, not a refusal of this session. */
const TRANSIENT_ERROR_CODES = ['server_error', 'temporarily_unavailable', 'unknown_error'];

/**
 * A renewal Keycloak refused: the session ended, the refresh token was revoked, or the identity
 * provider's session now belongs to someone else. Anything else, such as a timeout, a network
 * failure, a 5xx from a restarting Keycloak (as a JSON error body, an `ErrorResponse`, or an HTML
 * error page, a plain `Error`), or a stale iframe response, is worth retrying later.
 * @param error - The error from `signinSilent`.
 * @returns True when the local session should end.
 */
const isRefusal = (error: unknown) => {
  if (error instanceof ErrorResponse) {
    return !TRANSIENT_ERROR_CODES.includes(error.error ?? '');
  }
  return error instanceof Error && KEYCLOAK_SESSION_CHANGED.includes(error.message);
};

// A non-secret hint that this browser had a signed-in user, so anonymous visitors (and a dev server
// without Keycloak) skip the silent sign-in round trip on every page load.
const SIGNED_IN_HINT_KEY = 'econgraph.auth.signedIn';

const readSignedInHint = () => {
  try {
    return window.localStorage.getItem(SIGNED_IN_HINT_KEY) === '1';
  } catch {
    return false;
  }
};

const writeSignedInHint = (signedIn: boolean) => {
  try {
    if (signedIn) {
      window.localStorage.setItem(SIGNED_IN_HINT_KEY, '1');
    } else {
      window.localStorage.removeItem(SIGNED_IN_HINT_KEY);
    }
  } catch {
    // Storage unavailable or partitioned (private mode, some in-app browsers): the hint can never
    // be written, so every reload takes the "never signed in" path below and skips the silent
    // restore round trip. A signed-in user then sees Sign in again after every reload, not just
    // once per browser as elsewhere; there is no reload-surviving state available to do better.
  }
};

/**
 * Keycloak's account console, with a link back to this app.
 * @param manager - The user manager.
 * @returns The account console URL.
 */
const accountUrlFor = (manager: UserManager) =>
  `${manager.settings.authority.replace(/\/+$/, '')}/account?referrer=${encodeURIComponent(
    manager.settings.client_id
  )}`;

/** In-flight work shared by every caller, one set per user manager. */
interface Session {
  renewal: Promise<OidcUser | null> | null;
  restore: Promise<OidcUser | null> | null;
  codeExchange: Promise<OidcUser> | null;
}

/** Key for the session of a provider without a manager (sign-in not configured). */
const NO_MANAGER = {} as UserManager;

interface AuthProviderProps {
  children: React.ReactNode;
  /** Injected in tests. Defaults to the app-wide manager built from build config. */
  userManager?: UserManager | null;
}

export const AuthProvider: React.FC<AuthProviderProps> = ({ children, userManager }) => {
  const manager = useMemo(
    () => (userManager === undefined ? getUserManager() : userManager),
    [userManager]
  );
  // A new manager starts a new session. State, not memo, so React never discards it.
  const [sessions] = useState(() => new WeakMap<UserManager, Session>());
  const session = useMemo<Session>(() => {
    const key = manager ?? NO_MANAGER;
    let existing = sessions.get(key);
    if (!existing) {
      existing = { renewal: null, restore: null, codeExchange: null };
      sessions.set(key, existing);
    }
    return existing;
  }, [manager, sessions]);
  const [authState, setAuthState] = useState<AuthState>({
    user: null,
    isAuthenticated: false,
    isLoading: manager !== null,
    error: null,
  });

  const setUser = useCallback((oidcUser: OidcUser | null) => {
    setAuthState(prev => ({
      ...prev,
      user: oidcUser ? toUser(oidcUser) : null,
      isAuthenticated: oidcUser !== null,
      isLoading: false,
    }));
  }, []);

  // The redirect response can be consumed only once, so every caller shares one exchange.
  const exchangeCode = useCallback((): Promise<OidcUser> => {
    if (!manager) {
      return Promise.reject(new Error('Sign-in is not configured for this build'));
    }
    if (!session.codeExchange) {
      session.codeExchange = manager.signinRedirectCallback();
    }
    return session.codeExchange;
  }, [manager, session]);

  // The app is the only renewer (the library's automaticSilentRenew is off), so one refresh token
  // is never spent twice. Concurrent callers share the in-flight renewal.
  const renew = useCallback((): Promise<OidcUser | null> => {
    if (!manager) {
      return Promise.resolve(null);
    }
    if (!session.renewal) {
      session.renewal = manager
        .signinSilent()
        .catch(async error => {
          if (isRefusal(error)) {
            await manager.removeUser().catch(() => undefined);
          }
          // A timeout, network error or 5xx keeps the user; the next request or expiry retries.
          return null;
        })
        .finally(() => {
          session.renewal = null;
        });
    }
    return session.renewal;
  }, [manager, session]);

  // Tokens live in memory, so a page load starts empty: finish a redirect on the callback route,
  // otherwise restore the session of a browser that was signed in, through the same renewal path.
  const restoreSession = useCallback(async (): Promise<OidcUser | null> => {
    if (!manager) {
      return null;
    }
    if (window.location.pathname === CALLBACK_PATH) {
      try {
        return await exchangeCode();
      } catch {
        // A stale or reloaded callback URL: fall through to the identity provider's session.
      }
    }
    const existing = await manager.getUser();
    if (existing && !isExpiring(existing)) {
      return existing;
    }
    if (!existing && !readSignedInHint()) {
      return null;
    }
    const restored = (await renew()) ?? (await manager.getUser());
    if (!restored) {
      // Nothing to restore, whatever the reason: stop paying for the round trip on every load.
      // Keycloak's single sign-on makes the next Sign in one click.
      writeSignedInHint(false);
    }
    return restored;
  }, [manager, exchangeCode, renew]);

  // A layout effect, so the token provider is registered before any child's passive effect
  // (such as a query fetching on mount) asks for a token.
  useLayoutEffect(() => {
    if (!manager) {
      return undefined;
    }
    let active = true;

    const onUserLoaded = (oidcUser: OidcUser) => {
      writeSignedInHint(true);
      if (active) setUser(oidcUser);
    };
    const onUserUnloaded = () => {
      writeSignedInHint(false);
      if (active) setUser(null);
    };
    const onTokenExpiring = () => {
      void renew();
    };
    manager.events.addUserLoaded(onUserLoaded);
    manager.events.addUserUnloaded(onUserUnloaded);
    manager.events.addAccessTokenExpiring(onTokenExpiring);
    manager.events.addAccessTokenExpired(onTokenExpiring);

    // Restore once per session, even when StrictMode runs this effect twice.
    if (!session.restore) {
      session.restore = restoreSession().catch(() => null);
    }
    session.restore.then(restored => {
      if (active) setUser(restored);
    });

    const unregister = setAccessTokenProvider(async () => {
      // Requests made while the session is restored wait for it instead of going anonymous.
      await session.restore;
      const current = await manager.getUser();
      if (!current || !isExpiring(current)) {
        return current?.access_token ?? null;
      }
      const renewed = await renew();
      if (renewed) {
        return renewed.access_token;
      }
      // Renewal failed. Signed out if Keycloak refused; otherwise use the old token while valid,
      // and fail the request rather than silently sending it without the user's token.
      const fallback = await manager.getUser();
      if (!fallback) {
        return null;
      }
      if (!fallback.expired) {
        return fallback.access_token;
      }
      throw new Error('Your sign-in session could not be renewed');
    });

    return () => {
      active = false;
      manager.events.removeUserLoaded(onUserLoaded);
      manager.events.removeUserUnloaded(onUserUnloaded);
      manager.events.removeAccessTokenExpiring(onTokenExpiring);
      manager.events.removeAccessTokenExpired(onTokenExpiring);
      unregister();
    };
  }, [manager, session, renew, restoreSession, setUser]);

  const signIn = useCallback(
    async (returnTo?: string) => {
      if (!manager) {
        setAuthState(prev => ({ ...prev, error: 'Sign-in is not configured for this build' }));
        return;
      }
      try {
        setAuthState(prev => ({ ...prev, error: null }));
        const state: SignInState = { returnTo: safeReturnTo(returnTo ?? currentPath()) };
        await manager.signinRedirect({ state });
      } catch (error) {
        setAuthState(prev => ({
          ...prev,
          error: `Could not reach the sign-in service: ${errorMessage(error, 'unknown error')}`,
        }));
      }
    },
    [manager]
  );

  const signOut = useCallback(async () => {
    if (!manager) {
      return;
    }
    try {
      // Forgets the tokens and ends the identity provider session, then returns to the home page.
      await manager.signoutRedirect();
    } catch {
      // The provider is unreachable: at least forget the tokens held here (raises userUnloaded),
      // and say that the Keycloak session may still be alive.
      await manager.removeUser().catch(() => undefined);
      setAuthState(prev => ({
        ...prev,
        error:
          'Signed out of this page, but the sign-in service could not be reached. Close the browser to end your session.',
      }));
    }
  }, [manager]);

  const completeSignIn = useCallback(async (): Promise<string> => {
    try {
      const oidcUser = await exchangeCode();
      return safeReturnTo((oidcUser.state as SignInState | undefined)?.returnTo);
    } catch (error) {
      // A stale callback URL can still end signed in through the identity provider's session.
      if (await session.restore) {
        return '/';
      }
      throw error;
    }
  }, [exchangeCode, session]);

  const clearError = useCallback(() => {
    setAuthState(prev => ({ ...prev, error: null }));
  }, []);

  const contextValue = useMemo<AuthContextType>(
    () => ({
      ...authState,
      isConfigured: manager !== null,
      accountUrl: manager ? accountUrlFor(manager) : null,
      signIn,
      signOut,
      completeSignIn,
      clearError,
    }),
    [authState, manager, signIn, signOut, completeSignIn, clearError]
  );

  return <AuthContext.Provider value={contextValue}>{children}</AuthContext.Provider>;
};

export const useAuth = (): AuthContextType => {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
};

export default AuthContext;
