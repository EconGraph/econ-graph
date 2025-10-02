import type { StorybookConfig } from '@storybook/react-vite';

const config: StorybookConfig = {
  stories: ['../src/**/*.stories.@(js|jsx|mjs|ts|tsx)'],
  addons: [
    '@storybook/addon-onboarding',
    '@storybook/addon-links',
    '@storybook/addon-essentials',
    '@storybook/addon-interactions',
  ],
  framework: {
    name: '@storybook/react-vite',
    options: {},
  },
  docs: {
    autodocs: 'tag',
  },
  viteFinal: async (config) => {
    // Ensure compatibility with existing Vite config
    if (config.resolve) {
      config.resolve.alias = {
        ...config.resolve.alias,
        '@': '/src',
      };
    }
    
    // Handle Node.js modules for browser compatibility
    config.define = {
      ...config.define,
      global: 'globalThis',
    };
    
    // Configure external modules for browser compatibility
    config.external = config.external || [];
    config.external.push('fs', 'path');
    
    return config;
  },
};

export default config;
