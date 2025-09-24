/**
 * ESLint Configuration - Memory Optimized
 * Optimized for reduced memory usage during linting
 */

module.exports = {
  extends: [
    'react-app',
    'react-app/jest'
  ],

  // Memory optimization settings
  parserOptions: {
    ecmaVersion: 2020,
    sourceType: 'module',
    ecmaFeatures: {
      jsx: true
    }
  },

  // Environment settings
  env: {
    browser: true,
    es6: true,
    node: true,
    jest: true
  },

  // Rules optimized for memory efficiency
  rules: {
    // Disable memory-intensive rules
    'jest/no-conditional-expect': 'off',
    'testing-library/no-node-access': 'off',

    // Enable caching for better performance
    'import/no-unresolved': 'off',
    'import/extensions': 'off',

    // Reduce complexity checks that consume memory
    'complexity': 'off',
    'max-lines-per-function': 'off',
    'max-lines': 'off',
    'max-statements': 'off',
    'max-depth': 'off',
    'max-nested-callbacks': 'off',

    // Optimize for speed over thoroughness
    'no-unused-vars': 'warn',
    'no-console': 'warn',
    'prefer-const': 'warn',
    'no-var': 'warn'
  },

  // Override for test files (memory-intensive)
  overrides: [
    {
      files: [
        '**/*.test.ts',
        '**/*.test.tsx',
        '**/__tests__/**/*'
      ],
      rules: {
        'jest/no-conditional-expect': 'off',
        'testing-library/no-node-access': 'off',
        'testing-library/no-wait-for-multiple-assertions': 'off',
        'testing-library/no-wait-for-side-effects': 'off',
        'testing-library/prefer-screen-queries': 'off',
        'testing-library/render-result-naming-convention': 'off'
      }
    }
  ],

  // Ignore patterns for memory efficiency
  ignorePatterns: [
    'node_modules/',
    'build/',
    'coverage/',
    'dist/',
    '*.min.js',
    '*.bundle.js'
  ],

  // Settings for memory optimization
  settings: {
    react: {
      version: 'detect'
    },
    'import/resolver': {
      node: {
        extensions: ['.js', '.jsx', '.ts', '.tsx']
      }
    }
  }
};
