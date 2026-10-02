/// <reference types="vitest" />
import { defineConfig, configDefaults } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    // Run with the dedicated real-UI accessibility setup.
    exclude: [...configDefaults.exclude, "**/__tests__/accessibility/**"],
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/setupTests.ts"],
    css: true,
    testTimeout: 6000,
  },
});
