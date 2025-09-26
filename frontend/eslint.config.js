import globals from "globals";
import pluginJs from "@eslint/js";
import tseslint from "typescript-eslint";
import pluginReact from "eslint-plugin-react";
import pluginReactHooks from "eslint-plugin-react-hooks";
import pluginJsxA11y from "eslint-plugin-jsx-a11y";

export default [
  {
    files: ["**/*.{js,mjs,cjs,ts,jsx,tsx}"],
    settings: {
      react: {
        version: "detect",
      },
    },
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        ecmaFeatures: {
          jsx: true,
        },
        ecmaVersion: 2020,
        sourceType: "module",
      },
      globals: {
        // Browser globals
        window: 'readonly',
        document: 'readonly',
        console: 'readonly',
        fetch: 'readonly',
        localStorage: 'readonly',
        sessionStorage: 'readonly',
        Storage: 'readonly',
        performance: 'readonly',
        setTimeout: 'readonly',
        clearTimeout: 'readonly',
        setInterval: 'readonly',
        clearInterval: 'readonly',
        atob: 'readonly',
        btoa: 'readonly',
        URL: 'readonly',
        URLSearchParams: 'readonly',
        Blob: 'readonly',

        // DOM types
        HTMLElement: 'readonly',
        HTMLDivElement: 'readonly',
        HTMLButtonElement: 'readonly',
        HTMLInputElement: 'readonly',
        HTMLHeadingElement: 'readonly',
        SVGSVGElement: 'readonly',
        SVGElement: 'readonly',
        Element: 'readonly',
        MouseEvent: 'readonly',
        TouchEvent: 'readonly',
        KeyboardEvent: 'readonly',
        Event: 'readonly',
        MessageEvent: 'readonly',
        React: 'readonly',

        // Node.js globals
        process: 'readonly',
        global: 'readonly',
        module: 'readonly',
        require: 'readonly',

        // Vite globals
        import: 'readonly',
        importMeta: 'readonly',

        // Jest globals
        jest: 'readonly',
        describe: 'readonly',
        it: 'readonly',
        test: 'readonly',
        expect: 'readonly',
        beforeEach: 'readonly',
        afterEach: 'readonly',
        beforeAll: 'readonly',
        afterAll: 'readonly',
      },
    },
    plugins: {
      '@typescript-eslint': tseslint.plugin,
      'react': pluginReact,
      'react-hooks': pluginReactHooks,
      'jsx-a11y': pluginJsxA11y,
    },
    rules: {
      ...pluginJs.configs.recommended.rules,
      ...tseslint.configs.recommended.rules,
      ...pluginReact.configs.recommended.rules,
      ...pluginReactHooks.configs.recommended.rules,
      ...pluginJsxA11y.configs.recommended.rules,

      // Custom rules
      'react/react-in-jsx-scope': 'off', // Not needed with React 17+
      'react/prop-types': 'off', // Using TypeScript for prop validation
      '@typescript-eslint/no-unused-vars': 'warn',
      '@typescript-eslint/no-explicit-any': 'warn',
      'no-console': 'warn',
      '@typescript-eslint/no-require-imports': 'off', // Allow require() in test files
      'no-useless-escape': 'off', // Allow escape characters in regex
      'react/no-unescaped-entities': 'off', // Allow quotes and apostrophes in JSX
      'jsx-a11y/click-events-have-key-events': 'warn', // Warn instead of error
      'jsx-a11y/no-static-element-interactions': 'warn', // Warn instead of error
      'jsx-a11y/label-has-associated-control': 'warn', // Warn instead of error
      'no-case-declarations': 'off', // Allow variable declarations in case blocks
      'no-constant-binary-expression': 'warn', // Warn instead of error
      '@typescript-eslint/no-unused-expressions': 'warn', // Warn instead of error
      'react/display-name': 'warn', // Warn instead of error
    },
  },
];
