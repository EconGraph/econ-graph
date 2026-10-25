/// <reference types="vite/client" />
/// <reference types="vitest/globals" />

/** The app version, package.json's `version`, defined at build time (appVersion.config.ts). */
declare const __APP_VERSION__: string;

interface ImportMetaEnv {
  readonly VITE_API_URL: string;
  readonly VITE_GRAPHQL_URL: string;
  /** Where data operations go, with no token (e.g. `/data/graphql`). Unset: the GraphQL URL. */
  readonly VITE_DATA_GRAPHQL_URL?: string;
  readonly VITE_WS_URL: string;
  /** Keycloak realm issuer URL (`<keycloak>/realms/econ-graph`), or a path (`/idp/realms/econ-graph`) resolved against the page origin at runtime. Unset: the dev server uses the docker-compose dev realm; a build disables sign-in. */
  readonly VITE_OIDC_ISSUER?: string;
  /** Public OIDC client id. Defaults to econ-graph-web. */
  readonly VITE_OIDC_CLIENT_ID?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
