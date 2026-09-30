// Reuse the project's parser, plugins, settings and file matching. This audit
// runs only accessibility rules, with the complete recommended ruleset enabled.
import base from './eslint.config.js';
import jsxA11y from 'eslint-plugin-jsx-a11y';

export default [
  ...base.map(config => ({
    ...config,
    rules: Object.fromEntries(Object.keys(config.rules ?? {}).map(rule => [rule, 'off'])),
  })),
  {
    files: ['src/**/*.{js,jsx,ts,tsx}'],
    plugins: { 'jsx-a11y': jsxA11y },
    rules: jsxA11y.flatConfigs.recommended.rules,
  },
];
