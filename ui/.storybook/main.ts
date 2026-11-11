import type { StorybookConfig } from '@storybook/web-components-vite';

const config: StorybookConfig = {
  framework: '@storybook/web-components-vite',
  addons: ['@storybook/addon-a11y', '@storybook/addon-vitest'],
  stories: ['../src/**/*.stories.ts'],
  core: {
    // The sandbox's run phase has no network: nothing may try to reach one.
    disableTelemetry: true,
    disableWhatsNewNotifications: true,
  },
};

export default config;
