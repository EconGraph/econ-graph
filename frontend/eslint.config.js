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

        // Vitest globals
        vi: 'readonly',
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

      // Custom rules
      // Disable base rule in favor of @typescript-eslint/no-unused-vars
      'no-unused-vars': 'off',
      'react/react-in-jsx-scope': 'off', // Not needed with React 17+
      'react/prop-types': 'off', // Using TypeScript for prop validation
      '@typescript-eslint/no-unused-vars': ['warn', {
        argsIgnorePattern: '^_',
        varsIgnorePattern: '^_',
        // Do not warn on unused catch variables; often needed for control flow
        caughtErrors: 'none',
        ignoreRestSiblings: true,
      }],
      // Temporarily relax any usage while migrating types
      '@typescript-eslint/no-explicit-any': 'off',
      'no-console': 'warn',
      '@typescript-eslint/no-require-imports': 'off', // Allow require() in test files
      'no-useless-escape': 'off', // Allow escape characters in regex
      'react/no-unescaped-entities': 'off', // Allow quotes and apostrophes in JSX
      'jsx-a11y/click-events-have-key-events': 'off', // Too strict for development
      'jsx-a11y/no-static-element-interactions': 'off', // Too strict for development
      'jsx-a11y/label-has-associated-control': 'off', // Too strict for development
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
    plugins: {
      'testing-library': testingLibrary,
    },
    rules: {
      ...testingLibrary.configs.react.rules,
      '@typescript-eslint/no-explicit-any': 'off',
      'no-console': 'off',
      'testing-library/no-node-access': 'off', // Too strict for development
      'testing-library/no-manual-cleanup': 'off', // Vitest handles cleanup automatically
    },
  },
  {
    files: ['**/*.test.cjs', '**/*.cjs'],
    languageOptions: {
      globals: {
        process: 'readonly',
        console: 'readonly',
        describe: 'readonly',
        it: 'readonly',
        test: 'readonly',
        expect: 'readonly',
        beforeEach: 'readonly',
        afterEach: 'readonly',
        beforeAll: 'readonly',
        afterAll: 'readonly',
        vi: 'readonly',
      },
    },
    rules: {
      'no-console': 'off',
    },
  },
  // Test and mock files: relax noisy rules
  {
    files: [
      '**/*.test.ts',
      '**/*.test.tsx',
      'src/__tests__/**',
      'src/test-utils/**',
      'src/setupTests.ts',
    ],
    rules: {
      'no-console': 'off',
      '@typescript-eslint/no-explicit-any': 'off',
      'react/display-name': 'off',
      'jsx-a11y/click-events-have-key-events': 'off',
      'jsx-a11y/no-static-element-interactions': 'off',
      'jsx-a11y/label-has-associated-control': 'off',
    },
  },
  // Local dev server files can log freely
  {
    files: ['src/server/**', 'src/**/*.server.{js,ts}'],
    rules: {
      'no-console': 'off',
    },
  },
  // Types and polyfills: allow any
  {
    files: [
      'src/types/**',
      'src/test-utils/**',
      'src/setupTests.ts',
      'src/**/*.d.ts',
      'src/test-utils/polyfills.js',
    ],
    rules: {
      '@typescript-eslint/no-explicit-any': 'off',
      '@typescript-eslint/no-unused-vars': ['warn', { caughtErrors: 'none' }],
    },
  },
  // Allow console logs in server, utils, contexts, and hooks (diagnostics)
  {
    files: [
      'src/server/**',
      'src/utils/**',
      'src/contexts/**',
      'src/hooks/**',
    ],
    rules: {
      'no-console': 'off',
    },
  },
];
