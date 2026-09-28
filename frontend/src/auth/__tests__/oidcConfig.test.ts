/**
 * REQUIREMENT: Sign-in through Keycloak (OpenID Connect).
 * PURPOSE: Verify build config parsing and the UserManager settings (PKCE code flow, in-memory tokens).
 */

import { InMemoryWebStorage, WebStorageStateStore } from 'oidc-client-ts';

import { buildUserManagerSettings, readOidcConfig } from '../oidcConfig';

describe('readOidcConfig', () => {
  it('is disabled without an issuer', () => {
    expect(readOidcConfig({})).toBeNull();
    expect(readOidcConfig({ VITE_OIDC_ISSUER: '  ' })).toBeNull();
  });

  it('falls back to the docker-compose dev realm only under the dev server', () => {
    expect(readOidcConfig({ DEV: true })).toEqual({
      issuer: 'http://localhost:8081/realms/econ-graph',
      clientId: 'econ-graph-web',
    });
    expect(readOidcConfig({ DEV: false })).toBeNull();
  });

  it('defaults the client id and trims a trailing slash from the issuer', () => {
    expect(readOidcConfig({ VITE_OIDC_ISSUER: 'https://kc.test/realms/econ-graph/' })).toEqual({
      issuer: 'https://kc.test/realms/econ-graph',
      clientId: 'econ-graph-web',
    });
  });

  it('uses a configured client id', () => {
    expect(
      readOidcConfig({ VITE_OIDC_ISSUER: 'https://kc.test/realms/x', VITE_OIDC_CLIENT_ID: 'web2' })
    ).toEqual({ issuer: 'https://kc.test/realms/x', clientId: 'web2' });
  });
});

describe('buildUserManagerSettings', () => {
  const settings = buildUserManagerSettings(
    { issuer: 'https://kc.test/realms/econ-graph', clientId: 'econ-graph-web' },
    'https://econgraph.test'
  );

  it('uses the authorization code flow with callback routes on this origin', () => {
    expect(settings).toMatchObject({
      authority: 'https://kc.test/realms/econ-graph',
      client_id: 'econ-graph-web',
      response_type: 'code',
      redirect_uri: 'https://econgraph.test/auth/callback',
      silent_redirect_uri: 'https://econgraph.test/silent-callback.html',
      post_logout_redirect_uri: 'https://econgraph.test/',
      // AuthProvider owns renewal, so the library must not renew on its own as well.
      automaticSilentRenew: false,
      silentRequestTimeoutInSeconds: 5,
      requestTimeoutInSeconds: 5,
    });
  });

  it('keeps tokens in memory, not in browser storage', () => {
    expect(settings.userStore).toBeInstanceOf(WebStorageStateStore);
    expect((settings.userStore as unknown as { _store: unknown })._store).toBeInstanceOf(
      InMemoryWebStorage
    );
  });
});
