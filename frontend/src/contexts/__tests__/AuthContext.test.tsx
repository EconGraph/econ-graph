/**
 * REQUIREMENT: Sign-in through Keycloak (OpenID Connect).
 * PURPOSE: Verify the auth context against a mocked oidc-client-ts UserManager: signed-out state,
 * the redirect callback, bearer tokens on GraphQL requests and renewal of an expired token.
 */

import React from 'react';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query';
import {
  ErrorResponse,
  ErrorTimeout,
  User as OidcUser,
  type UserManager,
  type UserProfile,
} from 'oidc-client-ts';
import { vi } from 'vitest';

import { AuthProvider, safeReturnTo, useAuth } from '../AuthContext';
import Header from '../../components/layout/Header';
import { executeGraphQL } from '../../utils/graphql';
import { getAccessToken } from '../../auth/accessToken';
import ResetQueriesOnUserChange from '../../auth/ResetQueriesOnUserChange';

// setupTests.vitest.ts replaces the auth context with a stub for every other test.
vi.unmock('../AuthContext');

const ISSUER = 'https://keycloak.test/realms/econ-graph';

const makeUser = ({
  accessToken = 'access-1',
  expiresIn = 300,
  userState,
}: { accessToken?: string; expiresIn?: number; userState?: unknown } = {}) =>
  new OidcUser({
    access_token: accessToken,
    token_type: 'Bearer',
    session_state: null,
    profile: {
      sub: 'f3a1c2d4-0000-4000-8000-000000000001',
      iss: ISSUER,
      aud: 'econ-graph-web',
      exp: 0,
      iat: 0,
      name: 'Alice Analyst',
      email: 'alice@example.com',
    } as UserProfile,
    expires_at: Math.floor(Date.now() / 1000) + expiresIn,
    userState,
  });

type Listener = (...args: unknown[]) => void;

/**
 * A stand-in for oidc-client-ts's UserManager with an in-memory user and manual event firing.
 * @param initialUser - The user the manager already holds.
 * @returns The mock and helpers to fire its events.
 */
function mockUserManager(initialUser: OidcUser | null = null) {
  let stored = initialUser;
  const listeners = {
    loaded: [] as Listener[],
    unloaded: [] as Listener[],
    expiring: [] as Listener[],
    expired: [] as Listener[],
  };
  const add = (list: Listener[]) => vi.fn((cb: Listener) => list.push(cb));
  const remove = (list: Listener[]) =>
    vi.fn((cb: Listener) => {
      const i = list.indexOf(cb);
      if (i >= 0) list.splice(i, 1);
    });

  const manager = {
    settings: { authority: ISSUER, client_id: 'econ-graph-web' },
    events: {
      addUserLoaded: add(listeners.loaded),
      removeUserLoaded: remove(listeners.loaded),
      addUserUnloaded: add(listeners.unloaded),
      removeUserUnloaded: remove(listeners.unloaded),
      addAccessTokenExpiring: add(listeners.expiring),
      removeAccessTokenExpiring: remove(listeners.expiring),
      addAccessTokenExpired: add(listeners.expired),
      removeAccessTokenExpired: remove(listeners.expired),
    },
    getUser: vi.fn(async () => stored),
    // No identity provider session by default, so session restore fails as Keycloak would.
    signinSilent: vi.fn(async (): Promise<OidcUser | null> => {
      throw new Error('login_required');
    }),
    signinRedirect: vi.fn(async () => undefined),
    signinRedirectCallback: vi.fn(async (): Promise<OidcUser> => {
      throw new Error('no callback response');
    }),
    signoutRedirect: vi.fn(async () => undefined),
    removeUser: vi.fn(async () => {
      stored = null;
      listeners.unloaded.forEach(cb => cb());
    }),
  };

  const loadUser = (user: OidcUser) => {
    stored = user;
    listeners.loaded.forEach(cb => cb(user));
  };

  return {
    manager,
    userManager: manager as unknown as UserManager,
    loadUser,
    setStored: (user: OidcUser | null) => {
      stored = user;
    },
    fireExpiring: () => listeners.expiring.forEach(cb => cb()),
    fireExpired: () => listeners.expired.forEach(cb => cb()),
  };
}

// The shared test setup stubs localStorage to always be empty; these tests need a working one.
const storage = new Map<string, string>();
const workingLocalStorage = {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
};
const markPreviouslySignedIn = () => storage.set('econgraph.auth.signedIn', '1');

/**
 * A promise with its resolver exposed, to hold a mocked call open.
 * @returns The promise and its resolve function.
 */
function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>(r => {
    resolve = r;
  });
  return { promise, resolve };
}

const AuthProbe: React.FC = () => {
  const { user, isAuthenticated, isLoading, error } = useAuth();
  return (
    <div>
      <span data-testid='status'>
        {isLoading ? 'loading' : isAuthenticated ? `signed-in:${user?.name}` : 'signed-out'}
      </span>
      <span data-testid='user-id'>{user?.id ?? ''}</span>
      <span data-testid='error'>{error ?? ''}</span>
    </div>
  );
};

const CallbackProbe: React.FC<{ onReturn: (path: string) => void }> = ({ onReturn }) => {
  const { completeSignIn } = useAuth();
  React.useEffect(() => {
    completeSignIn().then(onReturn, () => undefined);
  }, [completeSignIn, onReturn]);
  return null;
};

describe('AuthContext with Keycloak (oidc-client-ts)', () => {
  const fetchMock = vi.fn();

  beforeEach(() => {
    fetchMock.mockReset();
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({ data: { ok: true } }),
    });
    vi.stubGlobal('fetch', fetchMock);
    storage.clear();
    vi.stubGlobal('localStorage', workingLocalStorage);
    window.history.replaceState(null, '', '/series/gdp?range=10y');
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    window.history.replaceState(null, '', '/');
  });

  it('shows Sign in when signed out and redirects to Keycloak with the current page', async () => {
    const { manager, userManager } = mockUserManager();
    render(
      <React.StrictMode>
        <AuthProvider userManager={userManager}>
          <Header onMenuClick={() => undefined} />
        </AuthProvider>
      </React.StrictMode>
    );

    const button = await screen.findByRole('button', { name: /sign in/i });
    // A browser that was never signed in makes no silent sign-in round trip.
    expect(manager.signinSilent).not.toHaveBeenCalled();

    await userEvent.click(button);
    expect(manager.signinRedirect).toHaveBeenCalledWith({
      state: { returnTo: '/series/gdp?range=10y' },
    });
  });

  it('tries one silent restore for a browser that was signed in, even under StrictMode', async () => {
    markPreviouslySignedIn();
    const { manager, userManager } = mockUserManager();
    manager.signinSilent.mockRejectedValue(new ErrorResponse({ error: 'login_required' }));
    render(
      <React.StrictMode>
        <AuthProvider userManager={userManager}>
          <Header onMenuClick={() => undefined} />
        </AuthProvider>
      </React.StrictMode>
    );

    expect(await screen.findByRole('button', { name: /sign in/i })).toBeInTheDocument();
    expect(manager.signinSilent).toHaveBeenCalledTimes(1);
    // Keycloak has no session any more, so the next page load skips the round trip.
    expect(storage.has('econgraph.auth.signedIn')).toBe(false);
  });

  it('shows an error when Keycloak cannot be reached on Sign in', async () => {
    const { manager, userManager } = mockUserManager();
    manager.signinRedirect.mockRejectedValue(new TypeError('Failed to fetch'));
    render(
      <AuthProvider userManager={userManager}>
        <Header onMenuClick={() => undefined} />
      </AuthProvider>
    );

    await userEvent.click(await screen.findByRole('button', { name: /sign in/i }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Could not reach the sign-in service: Failed to fetch'
    );
  });

  it('shows no Sign in button when the build has no issuer configured', () => {
    render(
      <AuthProvider userManager={null}>
        <Header onMenuClick={() => undefined} />
      </AuthProvider>
    );
    expect(screen.queryByRole('button', { name: /sign in/i })).not.toBeInTheDocument();
  });

  it('stores the user from the callback and returns to the saved page', async () => {
    window.history.replaceState(null, '', '/auth/callback?code=abc&state=xyz');
    const { manager, userManager, loadUser } = mockUserManager();
    manager.signinRedirectCallback.mockImplementation(async () => {
      // Like the real library, a successful exchange stores the user and fires userLoaded.
      const user = makeUser({ userState: { returnTo: '/series/gdp' } });
      loadUser(user);
      return user;
    });
    const onReturn = vi.fn();

    render(
      <React.StrictMode>
        <AuthProvider userManager={userManager}>
          <AuthProbe />
          <CallbackProbe onReturn={onReturn} />
        </AuthProvider>
      </React.StrictMode>
    );

    await waitFor(() => expect(onReturn).toHaveBeenCalledWith('/series/gdp'));
    expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst');
    // Later page loads know to restore the session.
    expect(storage.get('econgraph.auth.signedIn')).toBe('1');
    expect(screen.getByTestId('user-id')).toHaveTextContent('f3a1c2d4-0000-4000-8000-000000000001');
    // The code can be exchanged only once, even though StrictMode runs the effect twice.
    expect(manager.signinRedirectCallback).toHaveBeenCalledTimes(1);
    // The callback route does not also try a silent sign-in.
    expect(manager.signinSilent).not.toHaveBeenCalled();
  });

  it('never returns to another origin from the callback state', async () => {
    window.history.replaceState(null, '', '/auth/callback?code=abc&state=xyz');
    const { manager, userManager } = mockUserManager();
    manager.signinRedirectCallback.mockResolvedValue(
      makeUser({ userState: { returnTo: '//evil.example/phish' } })
    );
    const onReturn = vi.fn();

    render(
      <AuthProvider userManager={userManager}>
        <CallbackProbe onReturn={onReturn} />
      </AuthProvider>
    );

    await waitFor(() => expect(onReturn).toHaveBeenCalledWith('/'));
  });

  it.each([
    ['https://evil.example', '/'],
    ['//evil.example', '/'],
    ['/\\evil.example', '/'],
    ['/\t/evil.example', '/'],
    ['/\n/evil.example', '/'],
    ['/auth/callback?error=access_denied', '/'],
    [undefined, '/'],
    ['/series/1?x=2#y', '/series/1?x=2#y'],
  ])('safeReturnTo(%j) is %j', (value, expected) => {
    expect(safeReturnTo(value)).toBe(expected);
  });

  it('sends the access token as a bearer header on GraphQL requests', async () => {
    const { userManager } = mockUserManager(makeUser({ accessToken: 'token-alice' }));
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );

    await executeGraphQL({ query: '{ ok }' });

    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers.Authorization).toBe('Bearer token-alice');
  });

  it('sends no Authorization header when signed out', async () => {
    const { userManager } = mockUserManager();
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() => expect(screen.getByTestId('status')).toHaveTextContent('signed-out'));

    await executeGraphQL({ query: '{ ok }' });

    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers.Authorization).toBeUndefined();
  });

  it('renews an expired token before a request and uses the new one', async () => {
    const { manager, userManager, loadUser, setStored } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );

    setStored(makeUser({ accessToken: 'stale', expiresIn: -10 }));
    manager.signinSilent.mockImplementation(async () => {
      const renewed = makeUser({ accessToken: 'renewed' });
      loadUser(renewed);
      return renewed;
    });

    await executeGraphQL({ query: '{ ok }' });

    expect(manager.signinSilent).toHaveBeenCalledTimes(1);
    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers.Authorization).toBe('Bearer renewed');
  });

  it('renews shortly before expiry, keeping the user when renewal succeeds', async () => {
    const { manager, userManager, loadUser, fireExpiring } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    manager.signinSilent.mockImplementation(async () => {
      const renewed = makeUser({ accessToken: 'renewed' });
      loadUser(renewed);
      return renewed;
    });

    await act(async () => {
      fireExpiring();
    });

    expect(manager.signinSilent).toHaveBeenCalledTimes(1);
    expect(manager.removeUser).not.toHaveBeenCalled();
    expect(await getAccessToken()).toBe('renewed');
  });

  it('signs out when Keycloak refuses the renewal', async () => {
    const { manager, userManager, fireExpired } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    manager.signinSilent.mockRejectedValue(new ErrorResponse({ error: 'invalid_grant' }));

    await act(async () => {
      fireExpired();
    });

    expect(manager.removeUser).toHaveBeenCalled();
    await waitFor(() => expect(screen.getByTestId('status')).toHaveTextContent('signed-out'));
    expect(await getAccessToken()).toBeNull();
  });

  it('stays signed in when renewal fails for a transient reason', async () => {
    const { manager, userManager, fireExpiring } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    manager.signinSilent.mockRejectedValue(new ErrorTimeout('IFrame timed out without a response'));

    await act(async () => {
      fireExpiring();
    });

    expect(manager.signinSilent).toHaveBeenCalledTimes(1);
    expect(manager.removeUser).not.toHaveBeenCalled();
    expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst');
  });

  it('does not sign out on a stale iframe response ("State does not match")', async () => {
    const { manager, userManager, fireExpiring } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    // Thrown by oidc-client-ts for an unrelated, transient cause: a stale iframe response whose
    // OAuth state no longer correlates to a pending request. Not a sub mismatch.
    manager.signinSilent.mockRejectedValue(new Error('State does not match'));

    await act(async () => {
      fireExpiring();
    });

    expect(manager.removeUser).not.toHaveBeenCalled();
    expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst');
  });

  it('signs out when Keycloak now has a different user', async () => {
    const { manager, userManager, fireExpiring } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    // oidc-client-ts raises a plain Error when the renewed sub differs from the current one.
    manager.signinSilent.mockRejectedValue(new Error('sub in id_token does not match current sub'));

    await act(async () => {
      fireExpiring();
    });

    expect(manager.removeUser).toHaveBeenCalled();
    await waitFor(() => expect(screen.getByTestId('status')).toHaveTextContent('signed-out'));
  });

  it('signs out when a refreshed token drops azp', async () => {
    const { manager, userManager, fireExpiring } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    manager.signinSilent.mockRejectedValue(
      new Error('azp not in id_token, but present in original id_token')
    );

    await act(async () => {
      fireExpiring();
    });

    expect(manager.removeUser).toHaveBeenCalled();
    await waitFor(() => expect(screen.getByTestId('status')).toHaveTextContent('signed-out'));
  });

  it('keeps the user when Keycloak reports a temporary problem as an ErrorResponse', async () => {
    const { manager, userManager, fireExpiring } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    // A 5xx from Keycloak's own token endpoint (down for maintenance, a transient failure) is an
    // ErrorResponse with one of these codes, not a refusal of this session.
    manager.signinSilent.mockRejectedValue(new ErrorResponse({ error: 'temporarily_unavailable' }));

    await act(async () => {
      fireExpiring();
    });

    expect(manager.removeUser).not.toHaveBeenCalled();
    expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst');
  });

  it('keeps the user when Keycloak answers a renewal with a 5xx page', async () => {
    const { manager, userManager, fireExpiring } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    // oidc-client-ts reports an ingress error page this way while Keycloak restarts.
    manager.signinSilent.mockRejectedValue(
      new Error('Invalid response Content-Type: text/html, from URL: https://keycloak.test/token')
    );

    await act(async () => {
      fireExpiring();
    });

    expect(manager.removeUser).not.toHaveBeenCalled();
    expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst');
  });

  it('keeps using a still-valid token when renewal fails transiently', async () => {
    const { manager, userManager, setStored } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    setStored(makeUser({ accessToken: 'nearly-expired', expiresIn: 10 }));
    manager.signinSilent.mockRejectedValue(new TypeError('Failed to fetch'));

    await executeGraphQL({ query: '{ ok }' });

    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers.Authorization).toBe('Bearer nearly-expired');
  });

  it('fails the request instead of sending it anonymously when an expired token cannot renew', async () => {
    const { manager, userManager, setStored } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    setStored(makeUser({ accessToken: 'expired', expiresIn: -10 }));
    manager.signinSilent.mockRejectedValue(new TypeError('Failed to fetch'));

    await expect(executeGraphQL({ query: '{ ok }' })).rejects.toThrow(/could not be renewed/);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('shares one renewal between concurrent requests', async () => {
    const { manager, userManager, setStored } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    setStored(makeUser({ expiresIn: -10 }));
    const pending = deferred<OidcUser>();
    manager.signinSilent.mockReturnValue(pending.promise);

    const first = getAccessToken();
    const second = getAccessToken();
    await waitFor(() => expect(manager.signinSilent).toHaveBeenCalled());
    pending.resolve(makeUser({ accessToken: 'renewed' }));

    expect(await Promise.all([first, second])).toEqual(['renewed', 'renewed']);
    expect(manager.signinSilent).toHaveBeenCalledTimes(1);
  });

  it('stops trying to restore when the silent sign-in keeps timing out', async () => {
    markPreviouslySignedIn();
    const { manager, userManager } = mockUserManager();
    manager.signinSilent.mockRejectedValue(new ErrorTimeout('IFrame timed out without a response'));
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );

    await waitFor(() => expect(screen.getByTestId('status')).toHaveTextContent('signed-out'));
    expect(manager.signinSilent).toHaveBeenCalledTimes(1);
    expect(storage.has('econgraph.auth.signedIn')).toBe(false);
  });

  it('restores a session after a reload through silent sign-in', async () => {
    markPreviouslySignedIn();
    const { manager, userManager } = mockUserManager();
    manager.signinSilent.mockResolvedValue(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
  });

  it('holds requests made during session restore until it finishes', async () => {
    markPreviouslySignedIn();
    const { manager, userManager } = mockUserManager();
    let finishRestore: (user: OidcUser) => void = () => undefined;
    manager.signinSilent.mockImplementation(
      () =>
        new Promise<OidcUser>(resolve => {
          finishRestore = resolve;
        })
    );
    manager.getUser.mockResolvedValueOnce(null);
    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
      </AuthProvider>
    );

    const request = executeGraphQL({ query: '{ ok }' });
    await waitFor(() => expect(manager.signinSilent).toHaveBeenCalled());
    expect(fetchMock).not.toHaveBeenCalled();

    const restored = makeUser({ accessToken: 'restored' });
    manager.getUser.mockResolvedValue(restored);
    await act(async () => {
      finishRestore(restored);
      await request;
    });

    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers.Authorization).toBe('Bearer restored');
  });

  it('sends the restored token on queries that fetch on first render', async () => {
    markPreviouslySignedIn();
    const { manager, userManager, loadUser } = mockUserManager();
    // Like the library, a successful silent sign-in stores the user it returns.
    manager.signinSilent.mockImplementation(async () => {
      const restored = makeUser({ accessToken: 'restored' });
      loadUser(restored);
      return restored;
    });
    const QueryOnMount: React.FC = () => {
      useQuery(['on-mount'], () => executeGraphQL({ query: '{ ok }' }));
      return null;
    };

    render(
      <React.StrictMode>
        <QueryClientProvider client={new QueryClient()}>
          <AuthProvider userManager={userManager}>
            <QueryOnMount />
          </AuthProvider>
        </QueryClientProvider>
      </React.StrictMode>
    );

    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    for (const [, init] of fetchMock.mock.calls) {
      expect(init.headers.Authorization).toBe('Bearer restored');
    }
  });

  it('falls back to the Keycloak session when the callback URL is stale', async () => {
    markPreviouslySignedIn();
    window.history.replaceState(null, '', '/auth/callback?code=old&state=gone');
    const { manager, userManager } = mockUserManager();
    manager.signinRedirectCallback.mockRejectedValue(new Error('No matching state found'));
    manager.signinSilent.mockResolvedValue(makeUser());
    const onReturn = vi.fn();

    render(
      <AuthProvider userManager={userManager}>
        <AuthProbe />
        <CallbackProbe onReturn={onReturn} />
      </AuthProvider>
    );

    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    // The callback page goes home instead of reporting a failure for a signed-in user.
    await waitFor(() => expect(onReturn).toHaveBeenCalledWith('/'));
    expect(manager.signinRedirectCallback).toHaveBeenCalledTimes(1);
  });

  it('clears cached queries when the signed-in user changes', async () => {
    const { userManager } = mockUserManager(makeUser());
    const queryClient = new QueryClient();
    const resetQueries = vi.spyOn(queryClient, 'resetQueries');
    render(
      <QueryClientProvider client={queryClient}>
        <AuthProvider userManager={userManager}>
          <ResetQueriesOnUserChange />
          <AuthProbe />
        </AuthProvider>
      </QueryClientProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId('status')).toHaveTextContent('signed-in:Alice Analyst')
    );
    expect(resetQueries).not.toHaveBeenCalled();

    await act(async () => {
      await userManager.removeUser();
    });

    await waitFor(() => expect(resetQueries).toHaveBeenCalledTimes(1));
  });

  it('signs out through Keycloak from the user menu', async () => {
    const { manager, userManager } = mockUserManager(makeUser());
    render(
      <AuthProvider userManager={userManager}>
        <Header onMenuClick={() => undefined} />
      </AuthProvider>
    );

    await userEvent.click(await screen.findByRole('button', { name: /user menu/i }));
    expect(screen.getByRole('menuitem', { name: /manage account/i })).toHaveAttribute(
      'href',
      `${ISSUER}/account?referrer=econ-graph-web`
    );
    await userEvent.click(screen.getByRole('menuitem', { name: /sign out/i }));

    expect(manager.signoutRedirect).toHaveBeenCalledTimes(1);
  });

  it('forgets local tokens when Keycloak cannot be reached on sign out', async () => {
    const { manager, userManager } = mockUserManager(makeUser());
    manager.signoutRedirect.mockRejectedValue(new Error('network'));
    render(
      <AuthProvider userManager={userManager}>
        <Header onMenuClick={() => undefined} />
      </AuthProvider>
    );

    await userEvent.click(await screen.findByRole('button', { name: /user menu/i }));
    await userEvent.click(screen.getByRole('menuitem', { name: /sign out/i }));

    expect(manager.removeUser).toHaveBeenCalled();
    expect(await screen.findByRole('button', { name: /sign in/i })).toBeInTheDocument();
    expect(storage.has('econgraph.auth.signedIn')).toBe(false);
    expect(await screen.findByRole('alert')).toHaveTextContent(/could not be reached/);
  });
});
