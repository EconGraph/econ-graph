import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 3000,
    proxy: {
      "/api": "http://localhost:8081",
      "/graphql": "http://localhost:8081",
    },
  },
  build: {
    outDir: "build",
    sourcemap: true,
  },
  base: "/admin/",
});
