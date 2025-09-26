// ESLint configuration for Vite + React + TypeScript
import js from '@eslint/js';
import typescript from '@typescript-eslint/eslint-plugin';
import typescriptParser from '@typescript-eslint/parser';
import react from 'eslint-plugin-react';
import reactHooks from 'eslint-plugin-react-hooks';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import testingLibrary from 'eslint-plugin-testing-library';

export default [
  js.configs.recommended,
  {
    files: ['**/*.{ts,tsx,js,jsx}'],
    languageOptions: {
      parser: typescriptParser,
      parserOptions: {
        ecmaVersion: 2020,
        sourceType: 'module',
        ecmaFeatures: {
          jsx: true,
        },
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
      '@typescript-eslint': typescript,
      'react': react,
      'react-hooks': reactHooks,
      'jsx-a11y': jsxA11y,
      'testing-library': testingLibrary,
    },
    rules: {
      ...typescript.configs.recommended.rules,
      ...react.configs.recommended.rules,
      ...reactHooks.configs.recommended.rules,
      ...jsxA11y.configs.recommended.rules,
      ...testingLibrary.configs.react.rules,

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
    settings: {
      react: {
        version: 'detect',
      },
    },
  },
  {
    files: ['**/*.test.{ts,tsx,js,jsx}', '**/__tests__/**/*'],
    rules: {
      '@typescript-eslint/no-explicit-any': 'off',
      'no-console': 'off',
      'testing-library/no-node-access': 'warn',
    },
  },
];
