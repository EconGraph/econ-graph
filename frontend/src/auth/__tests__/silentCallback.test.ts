/**
 * REQUIREMENT: Sign-in through Keycloak (OpenID Connect).
 * PURPOSE: Verify the silent sign-in page hands the response to the app, and does nothing when
 * sign-in is not configured.
 */

import { vi } from 'vitest';

const { signinSilentCallback, userManagerCtor, readOidcConfig } = vi.hoisted(() => {
  const signinSilentCallbackFn = vi.fn(async () => undefined);
  // A plain function, not an arrow function, so it works when the module calls `new UserManager(...)`.
  const ctor = vi.fn(function UserManagerMock(this: { signinSilentCallback: typeof signinSilentCallbackFn }) {
    this.signinSilentCallback = signinSilentCallbackFn;
  });
  return { signinSilentCallback: signinSilentCallbackFn, userManagerCtor: ctor, readOidcConfig: vi.fn() };
});

vi.mock('oidc-client-ts', () => ({ UserManager: userManagerCtor }));
vi.mock('../oidcConfig', () => ({
  readOidcConfig,
  buildUserManagerSettings: (config: { issuer: string }) => ({ authority: config.issuer }),
}));

describe('silent-callback page', () => {
  beforeEach(() => {
    vi.resetModules();
    signinSilentCallback.mockClear();
    userManagerCtor.mockClear();
  });

  it('passes the response to the app window when an issuer is configured', async () => {
    readOidcConfig.mockReturnValue({ issuer: 'https://kc.test/realms/x', clientId: 'web' });

    await import('../silentCallback');

    expect(userManagerCtor).toHaveBeenCalledWith({ authority: 'https://kc.test/realms/x' });
    expect(signinSilentCallback).toHaveBeenCalledTimes(1);
  });

  it('does nothing without an issuer', async () => {
    readOidcConfig.mockReturnValue(null);

    await import('../silentCallback');

    expect(userManagerCtor).not.toHaveBeenCalled();
  });
});
