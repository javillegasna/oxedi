import { fileURLToPath } from 'node:url';
import { storybookTest } from '@storybook/addon-vitest/vitest-plugin';
import { playwright } from '@vitest/browser-playwright';
import { defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config.ts';

// Chromium's own sandbox needs user namespaces, which the sandbox's container does not grant;
// the viewer image sets this variable, and nothing else turns the browser's sandbox off.
const chromiumArgs = process.env.OXEDI_CHROMIUM_NO_SANDBOX === '1' ? ['--no-sandbox'] : [];

// The vite config brings the alias to the generated engine module.
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      projects: [
        {
          extends: true,
          plugins: [
            storybookTest({ configDir: fileURLToPath(new URL('.storybook', import.meta.url)) }),
          ],
          test: {
            name: 'storybook',
            browser: {
              enabled: true,
              headless: true,
              // The run must leave nothing outside dist/ and node_modules/.
              screenshotFailures: false,
              provider: playwright({ launchOptions: { args: chromiumArgs } }),
              instances: [{ browser: 'chromium' }],
            },
          },
        },
      ],
    },
  }),
);
