import type { StorybookConfig } from '@storybook/web-components-vite';
import { mergeConfig } from 'vite';

const config: StorybookConfig = {
  framework: '@storybook/web-components-vite',
  addons: ['@storybook/addon-a11y', '@storybook/addon-vitest'],
  stories: ['../src/**/*.stories.ts'],
  core: {
    // The sandbox's run phase has no network: nothing may try to reach one.
    disableTelemetry: true,
    disableWhatsNewNotifications: true,
  },
  // Lit's development build announces itself once per page; the stories and their tests run it
  // on purpose, for its extra checks, so the notice is silenced by its code before Lit loads.
  previewHead: (head) =>
    `${head}<script>(globalThis.litIssuedWarnings ??= new Set()).add('dev-mode');</script>`,
  // The renderer imports `lit/directive-helpers.js` besides `lit`; pre-bundling both together
  // keeps one copy of lit-html in the page.
  viteFinal: (viteConfig) =>
    mergeConfig(viteConfig, { optimizeDeps: { include: ['lit', 'lit/directive-helpers.js'] } }),
};

export default config;
