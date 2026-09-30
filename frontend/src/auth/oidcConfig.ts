/**
 * OpenID Connect settings for signing in through Keycloak.
 *
 * The issuer and client id are build-time config (`VITE_OIDC_ISSUER`, `VITE_OIDC_CLIENT_ID`).
 * The dev server falls back to the Keycloak dev realm from docker-compose. A production build
 * without an issuer disables sign-in rather than pointing at a guessed default.
 */

import {
  InMemoryWebStorage,
  UserManager,
  WebStorageStateStore,
  type UserManagerSettings,
} from 'oidc-client-ts';

/** Route that finishes the authorization code redirect. */
export const CALLBACK_PATH = '/auth/callback';

/**
 * Page loaded in the hidden iframe by a silent sign-in (session restore after a reload). It is a
 * separate, tiny Vite entry (`silent-callback.html`) so the iframe does not boot the whole app.
 */
export const SILENT_CALLBACK_PATH = '/silent-callback.html';

const DEFAULT_CLIENT_ID = 'econ-graph-web';

/** The dev realm served by `docker compose up keycloak`. */
export const DEV_ISSUER = 'http://localhost:8081/realms/econ-graph';

/** The build config variables sign-in reads. */
export interface OidcEnv {
  VITE_OIDC_ISSUER?: string;
  VITE_OIDC_CLIENT_ID?: string;
  /** True under the Vite dev server. */
  DEV?: boolean;
}

export interface OidcConfig {
  issuer: string;
  clientId: string;
}

/**
 * Reads the OIDC issuer and client id from build config.
 * @param env - The Vite env to read, `import.meta.env` by default.
 * @returns The config, or null when no issuer is configured.
 */
export function readOidcConfig(env: OidcEnv = import.meta.env): OidcConfig | null {
  const issuer = (env.VITE_OIDC_ISSUER?.trim() || (env.DEV ? DEV_ISSUER : '')).replace(/\/+$/, '');
  if (!issuer) {
    return null;
  }
  return { issuer, clientId: env.VITE_OIDC_CLIENT_ID?.trim() || DEFAULT_CLIENT_ID };
}

/**
 * Builds `UserManager` settings: authorization code with PKCE and tokens kept in memory only.
 * Renewal is driven by `AuthProvider` (refresh token grant while one is held, else a silent
 * iframe), not by the library's `automaticSilentRenew`, so exactly one renewal runs at a time.
 * @param config - Issuer and client id.
 * @param origin - The app's origin, used for redirect URIs.
 * @returns Settings for `new UserManager(...)`.
 */
export function buildUserManagerSettings(config: OidcConfig, origin: string): UserManagerSettings {
  return {
    authority: config.issuer,
    client_id: config.clientId,
    redirect_uri: `${origin}${CALLBACK_PATH}`,
    silent_redirect_uri: `${origin}${SILENT_CALLBACK_PATH}`,
    post_logout_redirect_uri: `${origin}/`,
    response_type: 'code',
    scope: 'openid profile email',
    automaticSilentRenew: false,
    // Fail fast when Keycloak is unreachable or hangs: the silent iframe (default 10 s) and every
    // fetch to Keycloak (discovery, JWKS, token; no timeout by default).
    silentRequestTimeoutInSeconds: 5,
    requestTimeoutInSeconds: 5,
    // Tokens never touch localStorage or sessionStorage. Only the short-lived PKCE verifier and
    // state for an in-flight redirect use sessionStorage (the library default for stateStore).
    userStore: new WebStorageStateStore({ store: new InMemoryWebStorage() }),
  };
}

let sharedUserManager: UserManager | null | undefined;

/**
 * Returns the app-wide `UserManager`, created on first use.
 * @returns The manager, or null when sign-in is not configured.
 */
export function getUserManager(): UserManager | null {
  if (sharedUserManager === undefined) {
    const config = readOidcConfig();
    sharedUserManager = config
      ? new UserManager(buildUserManagerSettings(config, window.location.origin))
      : null;
    // Drop state left in sessionStorage by sign-ins that never came back (timeouts, closed tabs).
    void sharedUserManager?.clearStaleState().catch(() => undefined);
  }
  return sharedUserManager;
}
