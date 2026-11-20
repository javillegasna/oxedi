import type { Meta, StoryObj } from '@storybook/web-components-vite';
import { html } from 'lit';
import { version } from 'oxedi-engine';
import { expect } from 'storybook/test';
import { OxediViewer } from './oxedi-viewer.ts';

const meta: Meta = {
  title: 'oxedi-viewer',
  component: 'oxedi-viewer',
};

export default meta;

/** The element loads the engine and shows the version the engine itself reports. */
export const EngineVersion: StoryObj = {
  render: () => html`<oxedi-viewer></oxedi-viewer>`,
  play: async ({ canvasElement }) => {
    const viewer = canvasElement.querySelector('oxedi-viewer');
    if (!(viewer instanceof OxediViewer)) {
      throw new Error('the story rendered no <oxedi-viewer> element in its canvas');
    }
    await viewer.updateComplete;
    const footer = viewer.shadowRoot?.querySelector('footer');
    await expect(footer?.textContent).toBe(`oxedi v${version()}`);
    await expect(version()).toMatch(/^\d+\.\d+\.\d+/);
  },
};
