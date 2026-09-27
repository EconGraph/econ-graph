/**
 * Entry for `silent-callback.html`, the page Keycloak redirects the silent sign-in iframe to.
 * It only passes the response to the parent window, which finishes sign-in.
 */

import { UserManager } from 'oidc-client-ts';

import { buildUserManagerSettings, readOidcConfig } from './oidcConfig';

const config = readOidcConfig();
if (config) {
  new UserManager(buildUserManagerSettings(config, window.location.origin))
    .signinSilentCallback()
    .catch(error => {
      console.error('Silent sign-in callback failed:', error);
    });
}
