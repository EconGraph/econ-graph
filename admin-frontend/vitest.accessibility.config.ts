// Keep the project's React transform, aliases, flags and jsdom environment.
// Replace broad unit-test UI/router mocks with the accessibility setup.
import { defineConfig, mergeConfig } from "vitest/config";
import base from "./vitest.config.ts";

export default mergeConfig(
  {
    ...base,
    test: {
      ...base.test,
      setupFiles: [],
      exclude: base.test?.exclude?.filter(
        (pattern) => pattern !== "**/__tests__/accessibility/**",
      ),
    },
  },
  defineConfig({
    test: {
      setupFiles: ["./src/__tests__/accessibility/setup.ts"],
      include: ["src/__tests__/accessibility/**/*.accessibility.test.tsx"],
      passWithNoTests: false,
      testTimeout: 30000,
      reporters: ["default", "json", "junit"],
      outputFile: {
        json: "accessibility-results/runtime.json",
        junit: "accessibility-results/runtime.xml",
      },
    },
  }),
);
