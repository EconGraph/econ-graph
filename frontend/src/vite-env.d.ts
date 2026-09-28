/// <reference types="vite/client" />
/// <reference types="vitest/globals" />

interface ImportMetaEnv {
  readonly VITE_API_URL: string;
  readonly VITE_GRAPHQL_URL: string;
  readonly VITE_WS_URL: string;
  /** Keycloak realm issuer URL (`<keycloak>/realms/econ-graph`). Unset: the dev server uses the docker-compose dev realm; a build disables sign-in. */
  readonly VITE_OIDC_ISSUER?: string;
  /** Public OIDC client id. Defaults to econ-graph-web. */
  readonly VITE_OIDC_CLIENT_ID?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
