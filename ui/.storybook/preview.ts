import type { Preview } from '@storybook/web-components-vite';
import '../src/tokens.css';

const preview: Preview = {
  parameters: {
    // An accessibility violation fails the story's test instead of only being listed.
    a11y: { test: 'error' },
  },
};

export default preview;
