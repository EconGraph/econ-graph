/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_GRAPHQL_ENDPOINT?: string;
  readonly VITE_MSW_DEBUG?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
