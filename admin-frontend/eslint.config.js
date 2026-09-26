import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist"] },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // eslint-plugin-react-hooks 7 adds the React Compiler rules to `recommended` as errors.
      // Keep the classic hook rules as errors; report the compiler rules as warnings until the
      // flagged components are refactored.
      ...Object.fromEntries(
        Object.keys(reactHooks.configs.recommended.rules)
          .filter(
            (rule) =>
              ![
                "react-hooks/rules-of-hooks",
                "react-hooks/exhaustive-deps",
              ].includes(rule),
          )
          .map((rule) => [rule, "warn"]),
      ),
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_" },
      ],
      "@typescript-eslint/no-explicit-any": "off", // Disable to reduce warnings
    },
  },
);
